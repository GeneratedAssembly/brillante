use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::web::AppState;

const SITE_CSS: &str = include_str!("site.css");
const SITE_JS: &str = include_str!("site.js");
const FAVICON: &str = include_str!("favicon.svg");
const SCRATCHPAD_CSS: &str = include_str!("scratchpad.css");
const SCRATCHPAD_JS: &str = include_str!("scratchpad.js");
const NOTES_CSS: &str = include_str!("notes.css");
const NOTES_JS: &str = include_str!("notes.js");
const TUTOR_CSS: &str = include_str!("tutor.css");
const TUTOR_JS: &str = include_str!("tutor.js");

pub async fn site_asset(Path(file): Path<String>) -> impl IntoResponse {
    match file.as_str() {
        "site.css" => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            SITE_CSS,
        ),
        "site.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            SITE_JS,
        ),
        "scratchpad.css" => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            SCRATCHPAD_CSS,
        ),
        "scratchpad.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            SCRATCHPAD_JS,
        ),
        "notes.css" => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            NOTES_CSS,
        ),
        "notes.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            NOTES_JS,
        ),
        "tutor.css" => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            TUTOR_CSS,
        ),
        "tutor.js" => (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            TUTOR_JS,
        ),
        "favicon.svg" => (
            [(header::CONTENT_TYPE, "image/svg+xml")],
            FAVICON,
        ),
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    }
    .into_response()
}

pub async fn widget_asset(
    State(state): State<AppState>,
    Path((kind, asset)): Path<(String, String)>,
) -> Response {
    let Some(def) = state.registry.get(&kind) else {
        return (StatusCode::NOT_FOUND, "unknown widget type").into_response();
    };
    let (body, content_type) = match asset.as_str() {
        "script.js" => (def.script, "text/javascript; charset=utf-8"),
        "style.css" => (def.styles, "text/css; charset=utf-8"),
        _ => return (StatusCode::NOT_FOUND, "unknown asset").into_response(),
    };
    (
        [(header::CONTENT_TYPE, content_type)],
        [(header::CACHE_CONTROL, "public, max-age=60")],
        body,
    )
        .into_response()
}
