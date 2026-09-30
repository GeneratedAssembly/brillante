use std::path::PathBuf;

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::web::AppState;

const BLOCKED_EXTENSIONS: [&str; 1] = ["toml"];

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "txt" | "md" => "text/plain; charset=utf-8",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

pub async fn serve(
    State(state): State<AppState>,
    Path((course_slug, rel)): Path<(String, String)>,
) -> Response {
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&course_slug) else {
        return (StatusCode::NOT_FOUND, "unknown course").into_response();
    };

    let rel_path = PathBuf::from(&rel);
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.contains('\0')
        || rel_path.components().any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    }
    let name = rel_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.starts_with('.') {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    if rel_path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|e| BLOCKED_EXTENSIONS.contains(&e.as_str()))
    {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    let full = course.dir.join(&rel_path);
    let Ok(full) = full.canonicalize() else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let Ok(course_root) = course.dir.canonicalize() else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    if !full.starts_with(&course_root) || !full.is_file() {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    match tokio::fs::read(&full).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type(&rel)),
                (header::CACHE_CONTROL, "public, max-age=300"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
