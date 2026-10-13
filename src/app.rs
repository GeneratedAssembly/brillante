use std::sync::Arc;

use crate::config::AppConfig;
use crate::content::ContentStore;
use crate::diag::log_diagnostics;
use crate::pb::PocketBase;
use crate::widgets::WidgetRegistry;
use crate::web::{build_router, AppState};

pub fn initial_state(app: &AppConfig) -> AppState {
    let store = ContentStore::load(&app.courses);
    log_diagnostics(&store.diagnostics);
    let pb = app
        .pocketbase
        .as_ref()
        .map(|cfg| PocketBase::new(&cfg.url));
    if pb.is_some() {
        tracing::info!(
            "pocketbase auth enabled: {}",
            app.pocketbase.as_ref().map(|c| c.url.as_str()).unwrap_or("")
        );
    }
    AppState {
        store: Arc::new(tokio::sync::RwLock::new(Arc::new(store))),
        registry: Arc::new(WidgetRegistry::with_builtins()),
        pb,
        pb_config: app.pocketbase.clone(),
        access: crate::web::access::AccessState::default(),
        tutor: crate::web::tutor::TutorState::new(),
    }
}

pub fn router(app: &AppConfig) -> axum::Router {
    build_router(initial_state(app))
}

const BOOTSTRAP_MAX_ATTEMPTS: u32 = 60;
const BOOTSTRAP_RETRY_SECS: u64 = 30;

/// Read `BRILLANTE_PB_SUPERUSER_*` from `pocketbase/.env` (a dev convenience;
/// real deployments should export them in the environment, which always wins).
/// Must run before the async runtime starts threads: `set_var` is only safe
/// while the process is single-threaded.
pub fn load_bootstrap_env() {
    let Ok(text) = std::fs::read_to_string("pocketbase/.env") else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.starts_with("BRILLANTE_PB_SUPERUSER_") && std::env::var_os(key).is_none() {
            let value = value.trim().trim_matches('"');
            // Safety: called from main before the tokio runtime exists.
            unsafe { std::env::set_var(key, value) };
        }
    }
}

/// Ensure every `bootstrap_admins` email holds the `admin` role. Runs in the
/// background with retries because PocketBase may not be up yet when the
/// server starts. Superuser credentials come from the environment and are
/// never used for request handling.
pub fn spawn_admin_bootstrap(config: &AppConfig) {
    let Some(cfg) = &config.pocketbase else {
        return;
    };
    if cfg.bootstrap_admins.is_empty() {
        return;
    }
    let superuser_email = match std::env::var("BRILLANTE_PB_SUPERUSER_EMAIL") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            tracing::warn!(
                "[pocketbase] bootstrap_admins is set but BRILLANTE_PB_SUPERUSER_EMAIL is not; \
                 admin role bootstrap skipped"
            );
            return;
        }
    };
    let superuser_password = match std::env::var("BRILLANTE_PB_SUPERUSER_PASSWORD") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            tracing::warn!(
                "[pocketbase] bootstrap_admins is set but BRILLANTE_PB_SUPERUSER_PASSWORD is not; \
                 admin role bootstrap skipped"
            );
            return;
        }
    };
    let pb = PocketBase::new(&cfg.url);
    let emails = cfg.bootstrap_admins.clone();
    tokio::spawn(async move {
        for attempt in 1..=BOOTSTRAP_MAX_ATTEMPTS {
            match pb.superuser_auth(&superuser_email, &superuser_password).await {
                Ok(token) => match pb.bootstrap_admins(&token, &emails).await {
                    Ok(report) => {
                        if !report.promoted.is_empty() {
                            tracing::info!("promoted to admin: {}", report.promoted.join(", "));
                        }
                        if report.missing.is_empty() {
                            tracing::debug!(
                                "admin bootstrap settled: {} already admin",
                                report.already_admin.len()
                            );
                            return;
                        }
                        // Fresh deployments sign up *after* first boot: keep
                        // retrying within the window so a bootstrap_admins
                        // account is promoted as soon as it exists.
                        if attempt == 1 {
                            tracing::warn!(
                                "bootstrap_admins emails with no account yet (retrying for \
                                 ~{} min): {}",
                                BOOTSTRAP_MAX_ATTEMPTS as u64 * BOOTSTRAP_RETRY_SECS / 60,
                                report.missing.join(", ")
                            );
                        }
                    }
                    Err(e) => {
                        if attempt == BOOTSTRAP_MAX_ATTEMPTS {
                            tracing::error!("admin role bootstrap failed: {e}");
                        } else {
                            tracing::warn!("admin role bootstrap failed ({e}); retrying");
                        }
                    }
                },
                Err(e) => {
                    if attempt == BOOTSTRAP_MAX_ATTEMPTS {
                        tracing::error!("admin role bootstrap could not authenticate: {e}");
                    } else {
                        tracing::debug!("superuser auth failed ({e}); retrying");
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(BOOTSTRAP_RETRY_SECS)).await;
        }
        tracing::warn!("admin role bootstrap gave up after its retry window; restart the                        server or promote manually from Admin > Users");
    });
}
