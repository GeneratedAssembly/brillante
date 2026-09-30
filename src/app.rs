use std::sync::Arc;

use crate::config::AppConfig;
use crate::content::ContentStore;
use crate::diag::log_diagnostics;
use crate::widgets::WidgetRegistry;
use crate::web::{build_router, AppState};

pub fn initial_state(app: &AppConfig) -> AppState {
    let store = ContentStore::load(&app.courses);
    log_diagnostics(&store.diagnostics);
    AppState {
        store: Arc::new(tokio::sync::RwLock::new(Arc::new(store))),
        registry: Arc::new(WidgetRegistry::with_builtins()),
    }
}

pub fn router(app: &AppConfig) -> axum::Router {
    build_router(initial_state(app))
}
