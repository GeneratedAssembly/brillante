pub mod access;
pub mod admin;
pub mod assets;
pub mod assignments;
pub mod auth;
pub mod content_files;
pub mod grader;
pub mod notes;
pub mod oauth;
pub mod pages;
pub mod progress;
pub mod search;
pub mod session;
pub mod subscriptions;
pub mod teach;
pub mod tutor;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

use crate::config::PocketBaseConfig;
use crate::content::ContentStore;
use crate::pb::PocketBase;
use crate::widgets::WidgetRegistry;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<tokio::sync::RwLock<Arc<ContentStore>>>,
    pub registry: Arc<WidgetRegistry>,
    pub pb: Option<PocketBase>,
    pub pb_config: Option<PocketBaseConfig>,
    pub access: access::AccessState,
    pub tutor: tutor::TutorState,
    pub grader: grader::GraderState,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(pages::home))
        .route("/browse", get(pages::browse))
        .route("/account", get(auth::account_page))
        .route("/auth/login", post(auth::login))
        .route("/auth/signup", post(auth::signup))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/oauth/{provider}", get(oauth::start))
        .route("/auth/oauth/{provider}/callback", get(oauth::callback))
        .route("/api/progress", get(progress::list).post(progress::update))
        .route("/my/subscriptions", post(subscriptions::update))
        .route("/admin", get(admin::admin_root))
        .route("/admin/courses", get(admin::courses_page))
        .route("/admin/courses/{slug}", post(admin::update_course))
        .route("/admin/courses/{slug}/grants", post(admin::update_grants))
        .route("/admin/users", get(admin::users_page))
        .route("/admin/users/roles", post(admin::update_roles))
        .route("/admin/users/delete", post(admin::delete_user))
        .route("/admin/tutor", get(admin::tutor_page))
        .route("/admin/tutor", post(admin::tutor_save))
        .route("/api/tutor/chat", post(tutor::chat))
        .route("/admin/grader", get(admin::grader_page))
        .route("/admin/grader", post(admin::grader_save))
        .route("/api/assignment/preview", post(assignments::preview))
        .route("/api/assignment/grade", post(assignments::grade))
        .route("/api/assignment/submission", get(assignments::submission))
        .route("/api/teach/preview", post(teach::preview))
        .route("/api/teach/grade", post(teach::grade))
        .route("/api/teach/submission", get(teach::submission))
        .route("/course/{slug}", get(pages::course_page))
        .route("/course/{slug}/{lesson}", get(pages::lesson_page))
        .route("/notes/{course}", get(notes::index))
        .route("/notes/{course}/{lesson}", post(notes::save))
        .route("/notes/{course}/{lesson}/{entry}", post(notes::update))
        .route(
            "/notes/{course}/{lesson}/{entry}/delete",
            post(notes::delete_entry),
        )
        .route("/api/notes/preview", post(notes::preview))
        .route("/assets/{file}", get(assets::site_asset))
        .route("/widgets/{kind}/{asset}", get(assets::widget_asset))
        .route("/content/{course}/{*path}", get(content_files::serve))
        .route("/healthz", get(healthz))
        .route("/admin/reload", post(admin::reload))
        .route("/admin/diagnostics", get(admin::diagnostics))
        .fallback(pages::not_found)
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}
