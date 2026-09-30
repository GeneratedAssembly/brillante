pub mod assets;
pub mod content_files;
pub mod pages;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::content::ContentStore;
use crate::widgets::WidgetRegistry;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<tokio::sync::RwLock<Arc<ContentStore>>>,
    pub registry: Arc<WidgetRegistry>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(pages::home))
        .route("/course/{slug}", get(pages::course_page))
        .route("/course/{slug}/{lesson}", get(pages::lesson_page))
        .route("/assets/{file}", get(assets::site_asset))
        .route("/widgets/{kind}/{asset}", get(assets::widget_asset))
        .route("/content/{course}/{*path}", get(content_files::serve))
        .route("/healthz", get(healthz))
        .route("/admin/reload", post(reload))
        .route("/admin/diagnostics", get(diagnostics))
        .fallback(pages::not_found)
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn reload(axum::extract::State(state): axum::extract::State<AppState>) -> String {
    let fresh = {
        let store = state.store.read().await;
        store.reload()
    };
    let summary = format!(
        "reloaded {} course(s), {} diagnostic(s)",
        fresh.courses.len(),
        fresh.diagnostics.len()
    );
    *state.store.write().await = Arc::new(fresh);
    summary
}

async fn diagnostics(axum::extract::State(state): axum::extract::State<AppState>) -> String {
    let store = state.store.read().await;
    if store.diagnostics.is_empty() {
        return "no diagnostics\n".to_string();
    }
    let mut out = String::new();
    for d in &store.diagnostics {
        out.push_str(&format!("{d}\n"));
    }
    out
}
