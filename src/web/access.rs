use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

use crate::pb::{CourseAccessRow, PbError, PocketBase};
use crate::web::session::{self, Session};
use crate::web::AppState;

pub const ADMIN_ROLE: &str = "admin";
const CACHE_TTL: Duration = Duration::from_secs(30);
const MAX_ROLE_LEN: usize = 64;
const MAX_ROLES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

/// Per-course privacy configuration. A course with no entry in the map is
/// public (backwards compatible with servers before privacy existed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseAccess {
    pub visibility: Visibility,
    pub allow_roles: Vec<String>,
}

pub type AccessMap = HashMap<String, CourseAccess>;

pub fn normalize_role(raw: &str) -> String {
    raw.trim().to_lowercase()
}

pub fn parse_allow_roles(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(normalize_role)
        .filter(|r| !r.is_empty() && r.len() <= MAX_ROLE_LEN)
        .take(MAX_ROLES)
        .collect()
}

/// Validate a single role name typed by an admin (normalized, charset-bound).
pub fn clean_role(raw: &str) -> Option<String> {
    let role = normalize_role(raw);
    if role.is_empty() || role.len() > MAX_ROLE_LEN {
        return None;
    }
    if !role
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return None;
    }
    Some(role)
}

/// Build the access map from PB rows. Unknown visibility strings degrade to
/// public (deny-privacy would brick a server on a typo'd row).
pub fn access_map_from_rows(rows: Vec<CourseAccessRow>) -> AccessMap {
    rows.into_iter()
        .filter(|row| !row.course.is_empty())
        .map(|row| {
            let visibility = match row.visibility.as_str() {
                "private" => Visibility::Private,
                _ => Visibility::Public,
            };
            let allow_roles = row
                .allow_roles
                .iter()
                .map(|r| normalize_role(r))
                .filter(|r| !r.is_empty())
                .collect();
            let access = CourseAccess {
                visibility,
                allow_roles,
            };
            (row.course, access)
        })
        .collect()
}

#[derive(Default)]
struct CacheInner {
    map: Option<Arc<AccessMap>>,
    fetched_at: Option<Instant>,
}

/// Short-TTL cache over the PB `course_access` collection. Visibility changes
/// propagate within the TTL; per-user roles and grants are never cached.
#[derive(Clone, Default)]
pub struct AccessState {
    inner: Arc<tokio::sync::Mutex<CacheInner>>,
}

impl AccessState {
    /// Cached map, refreshed from PB at most every CACHE_TTL. The lock is
    /// never held across the network call: two concurrent misses just fetch
    /// twice and the last write wins.
    pub async fn get(&self, pb: Option<&PocketBase>) -> Arc<AccessMap> {
        let Some(pb) = pb else {
            return Arc::new(AccessMap::new());
        };
        {
            let inner = self.inner.lock().await;
            if inner.fetched_at.is_some_and(|t| t.elapsed() < CACHE_TTL)
                && let Some(map) = &inner.map
            {
                return map.clone();
            }
        }
        let map = self.fetch_map(pb).await;
        let mut inner = self.inner.lock().await;
        inner.map = Some(map.clone());
        inner.fetched_at = Some(Instant::now());
        map
    }

    /// Always-fresh map for admin pages: after saving a change the rendered
    /// state must not lag the TTL.
    pub async fn refresh(&self, pb: Option<&PocketBase>) -> Arc<AccessMap> {
        let Some(pb) = pb else {
            return Arc::new(AccessMap::new());
        };
        let map = self.fetch_map(pb).await;
        let mut inner = self.inner.lock().await;
        inner.map = Some(map.clone());
        inner.fetched_at = Some(Instant::now());
        map
    }

    async fn fetch_map(&self, pb: &PocketBase) -> Arc<AccessMap> {
        let previous = self.inner.lock().await.map.clone();
        match pb.list_course_access().await {
            Ok(rows) => Arc::new(access_map_from_rows(rows)),
            Err(e) => {
                tracing::debug!("course access refresh failed: {e}");
                previous.unwrap_or_default()
            }
        }
    }
}

/// Who is asking: resolved from the session cookie. `session` is the PB token
/// holder; `user` carries the verified record (incl. roles); `grants` decide
/// private-course access; `subscriptions` is the personal "my courses" list.
#[derive(Debug, Clone, Default)]
pub struct Viewer {
    pub session: Option<Session>,
    pub user: Option<crate::pb::AuthUser>,
    pub grants: HashSet<String>,
    pub subscriptions: HashSet<String>,
}

impl Viewer {
    pub fn anonymous() -> Viewer {
        Viewer::default()
    }

    pub fn signed_in(&self) -> bool {
        self.session.is_some()
    }

    pub fn roles(&self) -> &[String] {
        match &self.user {
            Some(u) => &u.roles,
            None => &[],
        }
    }

    pub fn is_admin(&self) -> bool {
        self.roles().iter().any(|r| r == ADMIN_ROLE)
    }
}

/// Resolve the viewer with the user's own token so PocketBase verifies the
/// session on every call. Roles, grants, and subscriptions are fetched
/// concurrently; any failure degrades to anonymous (deny private access),
/// the same policy as progress rendering.
pub async fn viewer(state: &AppState, headers: &HeaderMap) -> Viewer {
    let Some(pb) = &state.pb else {
        return Viewer::anonymous();
    };
    let Some(raw) = session::cookie_from_headers(headers) else {
        return Viewer::anonymous();
    };
    let Some((user_id, exp)) = session::decode_claims(&raw) else {
        return Viewer::anonymous();
    };
    if exp <= session::now_unix() {
        return Viewer::anonymous();
    }
    let sess = Session {
        token: raw,
        user_id: user_id.clone(),
        exp,
    };
    let (user, grants, subs) = tokio::join!(
        pb.fetch_user(&sess.token, &user_id),
        pb.list_grants(&sess.token, &user_id),
        pb.list_subscriptions(&sess.token, &user_id),
    );
    let (user, grants, subs) = match (user, grants, subs) {
        (Ok(u), Ok(g), Ok(s)) => (u, g, s),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            tracing::debug!("viewer resolve degraded to anonymous: {e}");
            return Viewer::anonymous();
        }
    };
    Viewer {
        session: Some(sess),
        user: Some(user),
        grants: grants.into_iter().collect(),
        subscriptions: subs.into_iter().collect(),
    }
}

/// Whether `viewer` may open the course. Admins bypass; private courses open
/// to an exact role match or an individual grant.
pub fn can_view(slug: &str, viewer: &Viewer, map: &AccessMap) -> bool {
    let roles = viewer.roles();
    match map.get(slug) {
        None => true,
        Some(access) => match access.visibility {
            Visibility::Public => true,
            Visibility::Private => {
                roles.iter().any(|r| r == ADMIN_ROLE)
                    || roles.iter().any(|r| access.allow_roles.contains(r))
                    || viewer.grants.contains(slug)
            }
        },
    }
}

/// Roles a viewer holds that a private course accepts (for admin UI hints).
pub fn matching_roles(viewer: &Viewer, access: &CourseAccess) -> Vec<String> {
    viewer
        .roles()
        .iter()
        .filter(|r| access.allow_roles.contains(*r))
        .cloned()
        .collect()
}

pub fn pb_unreachable_error(e: &PbError) -> bool {
    matches!(e, PbError::Unreachable(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(roles: &[&str], grants: &[&str], subs: &[&str]) -> Viewer {
        Viewer {
            session: Some(Session {
                token: "t".into(),
                user_id: "u".into(),
                exp: 1,
            }),
            user: Some(crate::pb::AuthUser {
                id: "u".into(),
                email: "u@x.y".into(),
                name: "U".into(),
                roles: roles.iter().map(|s| s.to_string()).collect(),
                created: String::new(),
            }),
            grants: grants.iter().map(|s| s.to_string()).collect(),
            subscriptions: subs.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn map() -> AccessMap {
        access_map_from_rows(vec![
            CourseAccessRow {
                course: "open".into(),
                visibility: "public".into(),
                allow_roles: vec![],
            },
            CourseAccessRow {
                course: "members".into(),
                visibility: "private".into(),
                allow_roles: vec!["Student".into(), " instructor".into()],
            },
            CourseAccessRow {
                course: "weird".into(),
                visibility: "secret-sauce".into(),
                allow_roles: vec![],
            },
        ])
    }

    #[test]
    fn rows_parse_with_normalization() {
        let m = map();
        let members = &m["members"];
        assert_eq!(members.visibility, Visibility::Private);
        assert_eq!(members.allow_roles, vec!["student", "instructor"]);
        // Unknown visibility strings degrade to public, never to hidden.
        assert_eq!(m["weird"].visibility, Visibility::Public);
    }

    #[test]
    fn missing_row_means_public() {
        let v = Viewer::anonymous();
        assert!(can_view("unlisted-course", &v, &map()));
    }

    #[test]
    fn private_course_denies_anonymous_and_strangers() {
        let m = map();
        assert!(!can_view("members", &Viewer::anonymous(), &m));
        assert!(!can_view("members", &viewer(&[], &[], &[]), &m));
        assert!(!can_view("members", &viewer(&["moderator"], &[], &[]), &m));
        assert!(!can_view("members", &viewer(&[], &["other"], &[]), &m));
    }

    #[test]
    fn private_course_allows_role_grant_and_admin() {
        let m = map();
        assert!(can_view("members", &viewer(&["student"], &[], &[]), &m));
        assert!(can_view("members", &viewer(&["instructor"], &[], &[]), &m));
        assert!(can_view("members", &viewer(&[], &["members"], &[]), &m));
        assert!(can_view("members", &viewer(&["admin"], &[], &[]), &m));
    }

    #[test]
    fn public_and_missing_stay_open_for_everyone() {
        let m = map();
        assert!(can_view("open", &Viewer::anonymous(), &m));
        assert!(can_view("open", &viewer(&[], &[], &[]), &m));
    }

    #[test]
    fn role_matching_helpers() {
        assert_eq!(normalize_role("  Admin "), "admin");
        assert_eq!(parse_allow_roles(" A, b ,,C"), vec!["a", "b", "c"]);
        assert!(parse_allow_roles(" , ").is_empty());
        assert_eq!(clean_role("Student-2"), Some("student-2".to_string()));
        assert_eq!(clean_role(""), None);
        assert_eq!(clean_role("has space"), None);
        assert_eq!(clean_role(&"x".repeat(65)), None);
        let v = viewer(&["instructor"], &[], &[]);
        let access = &map()["members"];
        assert_eq!(matching_roles(&v, access), vec!["instructor".to_string()]);
    }
}
