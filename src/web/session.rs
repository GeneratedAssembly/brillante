use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use base64::Engine as _;
use cookie::{Cookie, CookieBuilder, SameSite};
use cookie::time::Duration;

use crate::pb::{AuthUser, PbError, PocketBase};

pub const SESSION_COOKIE: &str = "brillante.session";
const FIVE_DAYS: i64 = 5 * 24 * 60 * 60;
const REFRESH_MARGIN: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub exp: i64,
}

impl Session {
    pub fn needs_refresh(&self, now: i64) -> bool {
        self.exp - now < REFRESH_MARGIN
    }
}

/// PocketBase JWTs are HS512-signed by PB itself; Brillante never accepts a
/// token it did not receive from PB over a trusted channel, so the signature
/// is verified by PB on every API call. Claims are decoded, not trusted.
pub fn decode_claims(token: &str) -> Option<(String, i64)> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let user_id = claims
        .get("id")
        .or_else(|| claims.get("rid"))
        .and_then(|v| v.as_str())?
        .to_string();
    let exp = claims.get("exp")?.as_i64()?;
    Some((user_id, exp))
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn session_cookie(token: &str, secure: bool) -> Cookie<'static> {
    CookieBuilder::new(SESSION_COOKIE, token.to_string())
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::seconds(FIVE_DAYS))
        .secure(secure)
        .build()
}

pub fn cleared_cookie() -> Cookie<'static> {
    CookieBuilder::new(SESSION_COOKIE, "")
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::seconds(0))
        .build()
}

/// Resolves the session from the cookie, refreshing the PB token when it is
/// close to expiry (or when PB rejects it). Returns the session plus a
/// replacement cookie when the token was rotated or cleared.
pub async fn resolve(
    headers: &axum::http::HeaderMap,
    pb: &PocketBase,
    secure: bool,
) -> (Option<Session>, Option<Cookie<'static>>) {
    let Some(raw) = cookie_from_headers(headers) else {
        return (None, None);
    };
    let Some((user_id, exp)) = decode_claims(&raw) else {
        return (None, Some(cleared_cookie()));
    };
    let now = now_unix();
    if exp <= now {
        return (None, Some(cleared_cookie()));
    }
    let mut session = Session {
        token: raw.clone(),
        user_id,
        exp,
    };
    if !session.needs_refresh(now) {
        return (Some(session), None);
    }
    match pb.auth_refresh(&raw).await {
        Ok((fresh_token, _)) => {
            match decode_claims(&fresh_token) {
                Some((uid, exp)) => {
                    session = Session {
                        token: fresh_token.clone(),
                        user_id: uid,
                        exp,
                    };
                    (Some(session), Some(session_cookie(&fresh_token, secure)))
                }
                None => (None, Some(cleared_cookie())),
            }
        }
        Err(PbError::Unauthorized) => (None, Some(cleared_cookie())),
        Err(_) => (Some(session), None),
    }
}

pub fn cookie_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    named_cookie(headers, SESSION_COOKIE)
}

/// First value of the named cookie from the `Cookie` header, if present and
/// non-empty.
pub fn named_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    cookie::Cookie::split_parse(raw)
        .flatten()
        .find(|c| c.name() == name)
        .map(|c| c.value().to_string())
        .filter(|v| !v.is_empty())
}

/// Extractor for handlers that require an authenticated user.
pub struct Authed(pub Session, pub AuthUser);

#[derive(Debug)]
pub enum AuthError {
    Unauthorized,
    Upstream(String),
}

impl axum::response::IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AuthError::Unauthorized => (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": "authentication required" })),
            )
                .into_response(),
            AuthError::Upstream(msg) => {
                tracing::warn!("pocketbase error: {msg}");
                (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(serde_json::json!({ "error": "account service unavailable" })),
                )
                    .into_response()
            }
        }
    }
}

impl From<PbError> for AuthError {
    fn from(e: PbError) -> Self {
        match e {
            PbError::Unauthorized => AuthError::Unauthorized,
            other => AuthError::Upstream(other.to_string()),
        }
    }
}

impl axum::response::IntoResponse for PbError {
    fn into_response(self) -> axum::response::Response {
        match self {
            PbError::Rejected(m) => (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": m })),
            )
                .into_response(),
            PbError::Unauthorized => (
                axum::http::StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": "authentication required" })),
            )
                .into_response(),
            PbError::Unreachable(m) => {
                tracing::warn!("pocketbase unavailable: {m}");
                (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(serde_json::json!({ "error": "account service unavailable" })),
                )
                    .into_response()
            }
        }
    }
}

impl FromRequestParts<crate::web::AppState> for Authed {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::web::AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(pb) = &state.pb else {
            return Err(AuthError::Unauthorized);
        };
        let Some(raw) = cookie_from_headers(&parts.headers) else {
            return Err(AuthError::Unauthorized);
        };
        let Some((user_id, exp)) = decode_claims(&raw) else {
            return Err(AuthError::Unauthorized);
        };
        if exp <= now_unix() {
            return Err(AuthError::Unauthorized);
        }
        let session = Session {
            token: raw,
            user_id,
            exp,
        };
        let (_, user) = pb.auth_refresh(&session.token).await?;
        Ok(Authed(session, user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jwt(claims: serde_json::Value) -> String {
        let b64 = |v: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v);
        let header = b64(b"{\"alg\":\"HS512\"}");
        let payload = b64(serde_json::to_vec(&claims).unwrap().as_slice());
        format!("{header}.{payload}.sig")
    }

    #[test]
    fn decodes_rid_and_exp() {
        let jwt = make_jwt(serde_json::json!({"rid": "u123", "exp": 1000}));
        assert_eq!(decode_claims(&jwt), Some(("u123".into(), 1000)));
    }

    #[test]
    fn decodes_v040_id_claim() {
        let jwt = make_jwt(
            serde_json::json!({"id": "u9", "collectionId": "_pb_users_auth_", "exp": 1000}),
        );
        assert_eq!(decode_claims(&jwt), Some(("u9".into(), 1000)));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(decode_claims("not.a.jwt"), None);
        assert_eq!(decode_claims("a.b"), None);
    }

    #[test]
    fn needs_refresh_within_margin() {
        let s = Session {
            token: "t".into(),
            user_id: "u".into(),
            exp: 100_000,
        };
        assert!(s.needs_refresh(99_999));
        assert!(!s.needs_refresh(0));
    }

    #[test]
    fn session_cookie_is_httponly_lax() {
        let c = session_cookie("tok", false);
        assert!(c.http_only().unwrap());
        assert_eq!(c.same_site(), Some(SameSite::Lax));
        assert_eq!(c.path(), Some("/"));
        assert!(!c.secure().unwrap_or(false));
        let c = session_cookie("tok", true);
        assert_eq!(c.secure(), Some(true));
    }

    #[test]
    fn parses_named_cookie_from_header() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "other=1; brillante.session=abc; third=2".parse().unwrap(),
        );
        assert_eq!(cookie_from_headers(&headers).as_deref(), Some("abc"));
        let headers = axum::http::HeaderMap::new();
        assert_eq!(cookie_from_headers(&headers), None);
    }
}
