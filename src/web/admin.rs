use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use leptos::prelude::*;

use crate::render::render_document;
use crate::web::access::{self, clean_role, parse_allow_roles, Visibility, ADMIN_ROLE};
use crate::web::pages::{not_found_page, urlencode, Shell};
use crate::web::AppState;

const USERS_PER_PAGE: u32 = 50;
const MAX_ROLES_PER_USER: usize = 16;

/// Everything on this module renders only for admins; everyone else gets the
/// plain 404 page so the admin UI's existence is not advertised.
async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<access::Viewer, Box<Response>> {
    let viewer = access::viewer(state, headers).await;
    if !viewer.is_admin() || viewer.session.is_none() {
        return Err(Box::new(not_found_page()));
    }
    Ok(viewer)
}

/// `/admin` lands on the users tab for admins; others get the plain 404.
pub async fn admin_root(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    match require_admin(&state, &headers).await {
        Ok(_) => Redirect::to("/admin/users").into_response(),
        Err(resp) => *resp,
    }
}

fn admin_response(title: String, content: Vec<AnyView>) -> Response {
    Html(render_document(move || {
        view! {
            <Shell title={title} auth_enabled=true signed_in=true admin=true content />
        }
    }))
    .into_response()
}

fn redirect_with(path: &str, flag: Option<&str>) -> Response {
    match flag {
        Some(f) => Redirect::to(&format!("{path}?{f}")).into_response(),
        None => Redirect::to(path).into_response(),
    }
}

fn feedback(query_error: Option<&String>, query_saved: Option<&str>) -> AnyView {
    match (query_error.map(|e| e.as_str()), query_saved) {
        (Some("confirm"), _) => view! {
            <p class="auth-error">"Tick the confirm box next to Delete to remove an account."</p>
        }
        .into_any(),
        (Some("self-delete"), _) => view! {
            <p class="auth-error">"You cannot delete your own account."</p>
        }
        .into_any(),
        (Some("last-admin"), _) => view! {
            <p class="auth-error">"The last remaining admin account cannot be deleted."</p>
        }
        .into_any(),
        (Some("delete"), _) => view! {
            <p class="auth-error">"That account could not be deleted. Please try again."</p>
        }
        .into_any(),
        (Some(_), _) => view! {
            <p class="auth-error">"That change could not be applied. Check the value and try again."</p>
        }
        .into_any(),
        (None, Some("deleted")) => {
            view! { <p class="admin-saved">"Account deleted."</p> }.into_any()
        }
        (None, Some(_)) => view! { <p class="admin-saved">"Saved."</p> }.into_any(),
        (None, None) => view! { <span hidden></span> }.into_any(),
    }
}

fn tab_link(active: &str, name: &str, label: &'static str, href: &'static str) -> AnyView {
    let class = if active == name {
        "admin-tab active"
    } else {
        "admin-tab"
    };
    view! { <a class={class} href={href}>{label}</a> }.into_any()
}

fn admin_tabs(active: &str) -> AnyView {
    view! {
        <nav class="admin-tabs">
            {tab_link(active, "courses", "Courses", "/admin/courses")}
            {tab_link(active, "users", "Users", "/admin/users")}
            {tab_link(active, "tutor", "Tutor", "/admin/tutor")}
        </nav>
    }
    .into_any()
}

// ---------------------------------------------------------------------------
// Users: listing, search, role management
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Deserialize)]
pub struct UsersQuery {
    q: Option<String>,
    page: Option<u32>,
    error: Option<String>,
    saved: Option<String>,
}

pub async fn users_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<UsersQuery>,
) -> Response {
    let viewer = match require_admin(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let sess = viewer.session.as_ref().expect("admin session");
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    let page = params.page.unwrap_or(1).max(1);
    let term = params
        .q
        .as_ref()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let filter = term.as_ref().map(|t| {
        let quoted = crate::pb::filter_quote(t);
        format!("(email ~ {quoted} || name ~ {quoted})")
    });
    let (users, total) = match pb
        .list_users_page(&sess.token, filter.as_deref(), page, USERS_PER_PAGE)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("admin users list failed: {e}");
            return error_panel(
                "User list unavailable",
                "PocketBase could not be reached. Please try again.",
            );
        }
    };

    let rows: Vec<AnyView> = users
        .iter()
        .map(|u| user_row(u, u.id == sess.user_id))
        .collect();
    let total_pages = total.div_ceil(USERS_PER_PAGE).max(1);
    let q_param = term
        .as_ref()
        .map(|t| format!("&q={}", urlencode(t)))
        .unwrap_or_default();
    let pager: AnyView = if total_pages > 1 {
        let prev: AnyView = if page > 1 {
            view! {
                <a class="admin-pager-link" href={format!("/admin/users?page={}&q={}{}", page - 1, term.clone().unwrap_or_default(), q_param)}>
                    "Previous"
                </a>
            }
            .into_any()
        } else {
            view! { <span hidden></span> }.into_any()
        };
        let next: AnyView = if page < total_pages {
            view! {
                <a class="admin-pager-link" href={format!("/admin/users?page={}&q={}{}", page + 1, term.clone().unwrap_or_default(), q_param)}>
                    "Next"
                </a>
            }
            .into_any()
        } else {
            view! { <span hidden></span> }.into_any()
        };
        view! { <nav class="admin-pager">{prev}<span class="admin-pager-info">{format!("page {page} of {total_pages}")}</span>{next}</nav> }.into_any()
    } else {
        view! { <span hidden></span> }.into_any()
    };

    let content = vec![
        admin_tabs("users"),
        feedback(params.error.as_ref(), params.saved.as_deref()),
        view! {
            <section class="admin-panel">
                <h1>"Users"</h1>
                <p class="admin-lede">
                    "Roles gate private courses. The special role \""{ADMIN_ROLE}"\" grants platform administration."
                </p>
                <form method="get" action="/admin/users" class="admin-search">
                    <input type="search" name="q" placeholder="search by email or name" value={term.clone().unwrap_or_default()} />
                    <button type="submit" class="button button-outline button-small">"Search"</button>
                </form>
                <div class="admin-rows">{rows}</div>
                {pager}
            </section>
        }
        .into_any(),
    ];
    admin_response("Users — Brillante".to_string(), content)
}

fn user_row(user: &crate::pb::AuthUser, is_self: bool) -> AnyView {
    let email = user.email.clone();
    let name = if user.name.is_empty() {
        "—".to_string()
    } else {
        user.name.clone()
    };
    let uid = user.id.clone();
    let chips: Vec<AnyView> = user
        .roles
        .iter()
        .map(|role| {
            let role_value = role.clone();
            let label = role.clone();
            view! {
                <span class="chip chip-role">
                    {label}
                    <form method="post" action="/admin/users/roles" class="chip-form">
                        <input type="hidden" name="user_id" value={uid.clone()} />
                        <input type="hidden" name="action" value="remove" />
                        <input type="hidden" name="role" value={role_value} />
                        <button type="submit" class="chip-remove" title="Remove role">"×"</button>
                    </form>
                </span>
            }
            .into_any()
        })
        .collect();
    let add_form: AnyView = if user.roles.len() >= MAX_ROLES_PER_USER {
        view! { <span class="admin-muted">"role limit reached"</span> }.into_any()
    } else {
        view! {
            <form method="post" action="/admin/users/roles" class="role-add-form">
                <input type="hidden" name="user_id" value={uid.clone()} />
                <input type="hidden" name="action" value="add" />
                <input type="text" name="role" placeholder="add role" maxlength={MAX_ROLE_LEN_STR} />
                <button type="submit" class="button button-outline button-small">"Add"</button>
            </form>
        }
        .into_any()
    };
    let delete_form: AnyView = if is_self {
        view! { <span class="admin-muted">"you"</span> }.into_any()
    } else {
        view! {
            <form method="post" action="/admin/users/delete" class="user-delete-form">
                <input type="hidden" name="user_id" value={uid} />
                <label class="admin-confirm-label">
                    <input type="checkbox" name="confirm" value="on" />
                    "confirm"
                </label>
                <button type="submit" class="button button-outline button-small button-danger">
                    "Delete"
                </button>
            </form>
        }
        .into_any()
    };
    view! {
        <div class="admin-row">
            <span class="admin-user-name">{name}</span>
            <span class="admin-user-email">{email}</span>
            <span class="admin-roles">{chips}</span>
            {add_form}
            {delete_form}
        </div>
    }
    .into_any()
}

const MAX_ROLE_LEN_STR: &str = "64";

#[derive(Debug, serde::Deserialize)]
pub struct RoleForm {
    user_id: String,
    action: String,
    role: String,
    q: Option<String>,
    page: Option<u32>,
}

pub async fn update_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<RoleForm>,
) -> Response {
    let viewer = match require_admin(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let sess = viewer.session.as_ref().expect("admin session");
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    let back = users_back_url(&form.q, form.page, None);

    let Some(role) = clean_role(&form.role) else {
        return redirect_with(&back, Some("error=role"));
    };
    let user = match pb.fetch_user(&sess.token, &form.user_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("admin role update: target user fetch failed: {e}");
            return redirect_with(&back, Some("error=save"));
        }
    };
    let mut roles: Vec<String> = user.roles.clone();
    match form.action.as_str() {
        "add" => {
            if !roles.contains(&role) {
                if roles.len() >= MAX_ROLES_PER_USER {
                    return redirect_with(&back, Some("error=role-limit"));
                }
                roles.push(role);
            }
        }
        "remove" => roles.retain(|r| r != &role),
        _ => return redirect_with(&back, Some("error=role")),
    }
    match pb.update_user_roles(&sess.token, &form.user_id, &roles).await {
        Ok(()) => redirect_with(&back, Some("saved=1")),
        Err(e) => {
            tracing::warn!("admin role update failed: {e}");
            redirect_with(&back, Some("error=save"))
        }
    }
}

fn users_back_url(q: &Option<String>, page: Option<u32>, flag: Option<&str>) -> String {
    let mut url = String::from("/admin/users?");
    if let Some(term) = q.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        url.push_str(&format!("q={}&", urlencode(term)));
    }
    if let Some(p) = page.filter(|p| *p > 1) {
        url.push_str(&format!("page={p}&"));
    }
    if let Some(f) = flag {
        url.push_str(f);
    } else {
        url.pop(); // trailing '&' or '?'
    }
    url
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteUserForm {
    user_id: String,
    #[serde(default)]
    confirm: Option<String>,
    q: Option<String>,
    page: Option<u32>,
}

/// Refusals that keep the platform manageable: an admin cannot delete their
/// own signed-in account, and the last admin cannot be removed (which would
/// lock everyone out of the admin UI).
fn delete_guard(target: &crate::pb::AuthUser, self_id: &str) -> Result<(), &'static str> {
    if target.id == self_id {
        return Err("self-delete");
    }
    Ok(())
}

pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<DeleteUserForm>,
) -> Response {
    let viewer = match require_admin(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let sess = viewer.session.as_ref().expect("admin session");
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    let back = users_back_url(&form.q, form.page, None);

    if form.confirm.as_deref() != Some("on") {
        return redirect_with(&back, Some("error=confirm"));
    }
    let target = match pb.fetch_user(&sess.token, &form.user_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("admin delete: target fetch failed: {e}");
            return redirect_with(&back, Some("error=delete"));
        }
    };
    if let Err(flag) = delete_guard(&target, &sess.user_id) {
        return redirect_with(&back, Some(flag));
    }
    if target.is_admin() {
        match pb.list_users_page(&sess.token, Some("is_admin = true"), 1, 1).await {
            Ok((_, total)) if total > 1 => {}
            Ok((_, _)) => return redirect_with(&back, Some("error=last-admin")),
            Err(e) => {
                tracing::warn!("admin delete: admin count failed: {e}");
                return redirect_with(&back, Some("error=delete"));
            }
        }
    }
    match pb.delete_user(&sess.token, &target.id).await {
        Ok(()) => redirect_with(&back, Some("saved=deleted")),
        Err(e) => {
            tracing::warn!("admin delete failed: {e}");
            redirect_with(&back, Some("error=delete"))
        }
    }
}

// Courses: visibility, roles, individual grants
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Deserialize)]
pub struct CoursesQuery {
    error: Option<String>,
    saved: Option<String>,
}

pub async fn courses_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CoursesQuery>,
) -> Response {
    let viewer = match require_admin(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let sess = viewer.session.as_ref().expect("admin session");
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    let store = state.store.read().await.clone();
    let map = state.access.refresh(state.pb.as_ref()).await;
    let grants = match pb.list_grants_all(&sess.token).await {
        Ok(rows) => {
            let mut grouped: HashMap<String, Vec<crate::pb::GrantRow>> = HashMap::new();
            for row in rows {
                grouped.entry(row.course.clone()).or_default().push(row);
            }
            grouped
        }
        Err(e) => {
            tracing::warn!("admin grants list failed: {e}");
            return error_panel(
                "Course settings unavailable",
                "PocketBase could not be reached. Please try again.",
            );
        }
    };

    let panels: Vec<AnyView> = store
        .courses
        .iter()
        .map(|course| {
            let slug = course.slug.clone();
            let title = course.title.clone();
            let href = format!("/course/{slug}");
            let access_row = map.get(&slug);
            let visibility = access_row
                .map(|a| a.visibility)
                .unwrap_or(Visibility::Public);
            let allow_roles = access_row
                .map(|a| a.allow_roles.join(", "))
                .unwrap_or_default();
            let course_grants = grants.get(&slug).cloned().unwrap_or_default();
            course_panel(&slug, &title, &href, &allow_roles, visibility, &course_grants)
        })
        .collect();

    let content = vec![
        admin_tabs("courses"),
        feedback(params.error.as_ref(), params.saved.as_deref()),
        view! {
            <section class="admin-panel">
                <h1>"Course access"</h1>
                <p class="admin-lede">
                    "Courses are public unless configured otherwise. A private course is open to admins, to users holding one of its roles, and to individually granted users."
                </p>
                <div class="admin-panels">{panels}</div>
            </section>
        }
        .into_any(),
    ];
    admin_response("Course access — Brillante".to_string(), content)
}

fn course_panel(
    slug: &str,
    title: &str,
    href: &str,
    allow_roles: &str,
    visibility: Visibility,
    course_grants: &[crate::pb::GrantRow],
) -> AnyView {
    let private_selected = visibility == Visibility::Private;
    let grants_list: AnyView = if course_grants.is_empty() {
        view! { <p class="admin-muted">"No individual grants."</p> }.into_any()
    } else {
        let items: Vec<AnyView> = course_grants
            .iter()
            .map(|g| {
                let who = g
                    .expand
                    .as_ref()
                    .and_then(|e| e.user.as_ref())
                    .map(|u| {
                        if u.name.is_empty() {
                            u.email.clone()
                        } else {
                            format!("{} ({})", u.name, u.email)
                        }
                    })
                    .unwrap_or_else(|| g.id.clone());
                let gid = g.id.clone();
                view! {
                    <li class="grant-item">
                        <span>{who}</span>
                        <form method="post" action={format!("/admin/courses/{slug}/grants")} class="grant-revoke-form">
                            <input type="hidden" name="action" value="revoke" />
                            <input type="hidden" name="grant_id" value={gid} />
                            <button type="submit" class="button button-outline button-small">"Revoke"</button>
                        </form>
                    </li>
                }
                .into_any()
            })
            .collect();
        view! { <ul class="grant-list">{items}</ul> }.into_any()
    };
    view! {
        <section class="admin-course-panel">
            <h2 class="admin-course-title">
                <a href={href.to_string()}>{title.to_string()}</a>
                <span class="admin-course-slug">{slug.to_string()}</span>
            </h2>
            <form method="post" action={format!("/admin/courses/{slug}")} class="course-settings-form">
                <label>
                    "Visibility"
                    <select name="visibility">
                        <option value="public" selected={!private_selected}>"public"</option>
                        <option value="private" selected={private_selected}>"private"</option>
                    </select>
                </label>
                <label>
                    "Open to roles"
                    <input type="text" name="allow_roles" value={allow_roles.to_string()} placeholder="comma-separated, e.g. instructor, student-fall26" />
                </label>
                <button type="submit" class="button button-primary button-small">"Save"</button>
            </form>
            <div class="grant-block">
                <h3>"Individual grants"</h3>
                {grants_list}
                <form method="post" action={format!("/admin/courses/{slug}/grants")} class="grant-add-form">
                    <input type="hidden" name="action" value="grant" />
                    <input type="email" name="email" placeholder="user email" required />
                    <button type="submit" class="button button-outline button-small">"Grant access"</button>
                </form>
            </div>
        </section>
    }
    .into_any()
}

#[derive(Debug, serde::Deserialize)]
pub struct CourseSettingsForm {
    visibility: String,
    allow_roles: Option<String>,
}

pub async fn update_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    axum::Form(form): axum::Form<CourseSettingsForm>,
) -> Response {
    let viewer = match require_admin(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let sess = viewer.session.as_ref().expect("admin session");
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    {
        let store = state.store.read().await;
        if store.course(&slug).is_none() {
            return not_found_page();
        }
    }
    let visibility = match form.visibility.as_str() {
        "public" => "public",
        "private" => "private",
        _ => return redirect_with("/admin/courses", Some("error=visibility")),
    };
    let roles = form
        .allow_roles
        .as_deref()
        .map(parse_allow_roles)
        .unwrap_or_default();
    match pb
        .upsert_course_access(&sess.token, &slug, visibility, &roles)
        .await
    {
        Ok(()) => redirect_with("/admin/courses", Some("saved=1")),
        Err(e) => {
            tracing::warn!("admin course access update failed: {e}");
            redirect_with("/admin/courses", Some("error=save"))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct GrantForm {
    action: String,
    email: Option<String>,
    grant_id: Option<String>,
}

pub async fn update_grants(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    axum::Form(form): axum::Form<GrantForm>,
) -> Response {
    let viewer = match require_admin(&state, &headers).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let sess = viewer.session.as_ref().expect("admin session");
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    {
        let store = state.store.read().await;
        if store.course(&slug).is_none() {
            return not_found_page();
        }
    }
    let result = match form.action.as_str() {
        "grant" => {
            let Some(email) = form.email.as_deref().map(str::trim).filter(|e| !e.is_empty()) else {
                return redirect_with("/admin/courses", Some("error=email"));
            };
            match pb.find_user_by_email(&sess.token, email).await {
                Ok(Some(user)) => pb.create_grant(&sess.token, &user.id, &slug).await,
                Ok(None) => return redirect_with("/admin/courses", Some("error=unknown-user")),
                Err(e) => Err(e),
            }
        }
        "revoke" => match form.grant_id.as_deref().filter(|g| !g.is_empty()) {
            Some(gid) => pb.delete_grant(&sess.token, gid).await,
            None => return redirect_with("/admin/courses", Some("error=email")),
        },
        _ => return redirect_with("/admin/courses", Some("error=email")),
    };
    match result {
        Ok(()) => redirect_with("/admin/courses", Some("saved=1")),
        Err(e) => {
            tracing::warn!("admin grant update failed: {e}");
            redirect_with("/admin/courses", Some("error=save"))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct TutorForm {
    #[serde(default)]
    enabled: Option<String>,
    base_url: String,
    model: String,
    #[serde(default)]
    api_key: Option<String>,
}

fn valid_endpoint(url: &str) -> bool {
    url.len() <= 300
        && (url.starts_with("https://") || url.starts_with("http://"))
        && !url.contains(char::is_whitespace)
}

pub async fn tutor_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<UsersQuery>,
) -> Response {
    match require_admin(&state, &headers).await {
        Ok(_) => {}
        Err(resp) => return *resp,
    }
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    let settings = state.tutor.settings(Some(pb)).await;
    let key_hint = if settings.key_tail.is_empty() {
        "not set".to_string()
    } else {
        format!("stored (ends …{})", settings.key_tail)
    };
    let content = vec![
        admin_tabs("tutor"),
        feedback(params.error.as_ref(), params.saved.as_deref()),
        view! {
            <section class="admin-panel">
                <h1>"Virtual tutor"</h1>
                <p class="admin-lede">
                    "When enabled, signed-in learners get an Ask-the-tutor panel on lesson \
                     pages. The tutor sees the current lesson and course outline as context. \
                     Any OpenAI-compatible endpoint works (OpenAI, OpenRouter, Groq, …)."
                </p>
                <form method="post" action="/admin/tutor" class="tutor-config-form">
                    <label class="admin-field admin-field-check">
                        <input
                            type="checkbox"
                            name="enabled"
                            value="on"
                            checked=settings.enabled
                        />
                        "Enable the tutor"
                    </label>
                    <label class="admin-field">
                        <span>"Endpoint (base URL)"</span>
                        <input
                            type="text"
                            name="base_url"
                            value={settings.base_url}
                            placeholder="https://api.openai.com/v1"
                            maxlength="300"
                        />
                    </label>
                    <label class="admin-field">
                        <span>"API key"</span>
                        <input
                            type="password"
                            name="api_key"
                            value=""
                            autocomplete="off"
                            placeholder={key_hint}
                            maxlength="400"
                        />
                        <small class="admin-muted">"Leave blank to keep the stored key."</small>
                    </label>
                    <label class="admin-field">
                        <span>"Model"</span>
                        <input
                            type="text"
                            name="model"
                            value={settings.model}
                            placeholder="gpt-4o-mini"
                            maxlength="120"
                        />
                        <small class="admin-muted">
                            "Exact model name for your endpoint; not validated here."
                        </small>
                    </label>
                    <button type="submit" class="button button-primary">"Save"</button>
                </form>
            </section>
        }
        .into_any(),
    ];
    admin_response("Tutor — Brillante".to_string(), content)
}

pub async fn tutor_save(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<TutorForm>,
) -> Response {
    match require_admin(&state, &headers).await {
        Ok(_) => {}
        Err(resp) => return *resp,
    }
    let pb = state.pb.as_ref().expect("pb enabled for admins");
    let base_url = form.base_url.trim().trim_end_matches('/');
    let model = form.model.trim();
    if !valid_endpoint(base_url) || model.is_empty() || model.len() > 120 {
        return redirect_with("/admin/tutor", Some("error=config"));
    }
    let enabled = form.enabled.as_deref() == Some("on");
    // PB 0.40 refuses hidden-field (api_key) writes from admin tokens, so
    // the server persists with its superuser credentials; the admin gate
    // above is what authorizes the change.
    let su = match crate::web::tutor::superuser_token(pb).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("tutor save needs superuser credentials: {e}");
            return redirect_with("/admin/tutor", Some("error=credentials"));
        }
    };
    match pb
        .save_tutor_settings(&su, enabled, base_url, model, form.api_key.as_deref())
        .await
    {
        Ok(()) => {
            state.tutor.invalidate().await;
            redirect_with("/admin/tutor", Some("saved=1"))
        }
        Err(e) => {
            tracing::warn!("tutor settings save failed: {e}");
            redirect_with("/admin/tutor", Some("error=save"))
        }
    }
}

fn error_panel(title: &str, message: &str) -> Response {
    let title = format!("{title} — Brillante");
    let content = vec![view! {
        <section class="admin-panel">
            <h1>{title.clone()}</h1>
            <p class="auth-error">{message.to_string()}</p>
            <p><a href="/admin/courses">"Back to course access"</a></p>
        </section>
    }
    .into_any()];
    admin_response(title, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, admin: bool) -> crate::pb::AuthUser {
        crate::pb::AuthUser {
            id: id.into(),
            email: format!("{id}@x.y"),
            name: String::new(),
            roles: if admin { vec!["admin".into()] } else { vec![] },
            created: String::new(),
        }
    }

    #[test]
    fn delete_guard_blocks_self_delete() {
        assert_eq!(delete_guard(&user("u1", true), "u1"), Err("self-delete"));
        assert_eq!(delete_guard(&user("u2", false), "u1"), Ok(()));
    }

    #[test]
    fn feedback_maps_delete_flags() {
        let html = |view: AnyView| crate::render::render_html(move || view);
        assert!(html(feedback(None, Some("deleted"))).contains("Account deleted."));
        assert!(html(feedback(Some(&"confirm".to_string()), None)).contains("confirm box"));
        assert!(html(feedback(Some(&"last-admin".to_string()), None)).contains("last remaining admin"));
        assert!(html(feedback(Some(&"self-delete".to_string()), None)).contains("own account"));
        assert!(html(feedback(None, None)).contains("hidden"));
    }
}
