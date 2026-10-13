use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::pb::PbError;
use crate::web::session::Authed;
use crate::web::AppState;

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 160
        && s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

pub(crate) fn slug_ok(s: &str) -> bool {
    valid_slug(s)
}

#[derive(Debug, serde::Deserialize)]
pub struct ProgressUpdate {
    course: String,
    lesson: String,
    done: bool,
}

pub async fn list(
    State(state): State<AppState>,
    Authed(session, _user): Authed,
) -> Result<Response, PbError> {
    let pb = state.pb.expect("auth enabled implies pb");
    let rows = pb.list_progress(&session.token).await?;
    let mut courses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        courses.entry(row.course).or_default().push(row.lesson);
    }
    Ok(Json(serde_json::json!({ "courses": courses })).into_response())
}

pub async fn update(
    State(state): State<AppState>,
    Authed(session, user): Authed,
    Json(update): Json<ProgressUpdate>,
) -> Result<Response, PbError> {
    if !valid_slug(&update.course) || !valid_slug(&update.lesson) {
        return Err(PbError::Rejected(
            "course and lesson must be lowercase slug values".into(),
        ));
    }
    let pb = state.pb.expect("auth enabled implies pb");
    if update.done {
        pb.complete_lesson(&session.token, &user.id, &update.course, &update.lesson)
            .await?;
    } else {
        pb.uncomplete_lesson(&session.token, &update.course, &update.lesson)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_rules() {
        assert!(valid_slug("getting-started"));
        assert!(valid_slug("loops2"));
        assert!(!valid_slug(""));
        assert!(!valid_slug("-x"));
        assert!(!valid_slug("x-"));
        assert!(!valid_slug("a--b"));
        assert!(!valid_slug("A-b"));
        assert!(!valid_slug("a b"));
        assert!(!valid_slug("a/b"));
        assert!(!valid_slug(&"x".repeat(161)));
    }
}
