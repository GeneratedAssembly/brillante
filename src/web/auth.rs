use std::collections::HashSet;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use leptos::prelude::*;

use crate::content::course::Course;
use crate::render::render_document;
use crate::web::pages::{self, Shell};
use crate::web::session::{self, cleared_cookie, session_cookie};
use crate::web::AppState;

#[derive(Debug, Default, serde::Deserialize)]
pub struct NextParam {
    pub(crate) next: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct AccountQuery {
    next: Option<String>,
    error: Option<String>,
}

fn error_text(code: Option<&String>) -> Option<&'static str> {
    match code.map(|c| c.as_str()) {
        Some("invalid") => Some("Invalid email or password."),
        Some("mismatch") => Some("Passwords do not match."),
        Some("signup") => {
            Some("Could not create the account: the email may already be in use, or the password is too short.")
        }
        Some("unavailable") => Some("The account service is unavailable. Please try again."),
        Some("disabled") => Some("Accounts are not configured on this server."),
        Some("oauth") => {
            Some("Sign-in with the external provider failed or was cancelled. Please try again.")
        }
        Some("oauth_state") => Some("The sign-in attempt has expired. Please start again."),
        _ => None,
    }
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
    next: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct SignupForm {
    name: String,
    email: String,
    password: String,
    #[serde(rename = "passwordConfirm")]
    password_confirm: String,
    next: Option<String>,
}

pub(crate) fn safe_next(next: Option<&String>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") => n.clone(),
        _ => "/".to_string(),
    }
}

fn disabled_response() -> Response {
    let content = vec![view! {
        <section class="auth-panel">
            <h1>"Accounts"</h1>
            <p>"Accounts are not configured on this server. Add a [pocketbase] section to brillante.toml to enable sign-in and synced course progress."</p>
        </section>
    }
    .into_any()];
    Html(render_document(move || {
        view! {
            <Shell title="Accounts — Brillante".to_string() content />
        }
    }))
    .into_response()
}

pub async fn account_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AccountQuery>,
) -> Response {
    let Some(pb_cfg) = &state.pb_config else {
        return disabled_response();
    };
    let pb = state.pb.clone().unwrap();
    let (sess, rotate) = session::resolve(&headers, &pb, pb_cfg.secure_cookies).await;
    let next = safe_next(params.next.as_ref());
    let error_msg = error_text(params.error.as_ref());
    let viewer = crate::web::access::viewer(&state, &headers).await;
    let roles: Vec<String> = viewer.roles().to_vec();
    let user = viewer.user.clone();
    let is_admin = roles.iter().any(|r| r == "admin");
    let (subs, done) = if viewer.signed_in() {
        (
            viewer.subscriptions.clone(),
            crate::web::pages::server_progress(&state, &headers).await,
        )
    } else {
        (HashSet::new(), HashSet::new())
    };
    let oauth_providers = if sess.is_none() {
        match pb.oauth_providers().await {
            Ok(list) => {
                if list.is_empty() {
                    tracing::warn!(
                        "no usable oauth2 providers on /account \
                         (pocketbase reports none or entries lack an authURL)"
                    );
                }
                list
            }
            Err(e) => {
                tracing::warn!("oauth providers unavailable on /account: {e}");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let oauth_buttons: Vec<AnyView> = oauth_providers
        .iter()
        .map(|p| {
            let href = crate::web::oauth::start_href(&p.name, &next);
            let label = format!("Continue with {}", p.display_name);
            view! { <a class="button button-outline oauth-button" href={href}>{label}</a> }
                .into_any()
        })
        .collect();
    let oauth_block: AnyView = if oauth_buttons.is_empty() {
        view! { <span hidden></span> }.into_any()
    } else {
        view! {
            <div class="oauth-intro">
                <div class="oauth-buttons">{oauth_buttons}</div>
                <p class="auth-divider"><span>"or continue with email"</span></p>
            </div>
        }
        .into_any()
    };
    let error_view = move || -> AnyView {
        match error_msg {
            Some(msg) => view! { <p class="auth-error">{msg}</p> }.into_any(),
            None => view! { <span hidden></span> }.into_any(),
        }
    };
    let role_chips: AnyView = if roles.is_empty() {
        view! { <p class="auth-roles">"No roles assigned."</p> }.into_any()
    } else {
        let chips: Vec<AnyView> = roles
            .iter()
            .map(|r| view! { <span class="chip chip-role">{r.clone()}</span> }.into_any())
            .collect();
        view! { <p class="auth-roles">"Roles: " {chips}</p> }.into_any()
    };
    let store = if viewer.signed_in() {
        Some(state.store.read().await.clone())
    } else {
        None
    };
    let in_progress: Vec<(&Course, usize)> = match store.as_deref() {
        Some(store) => {
            let map = state.access.get(state.pb.as_ref()).await;
            store
                .courses
                .iter()
                .filter(|c| {
                    subs.contains(&c.slug) && crate::web::access::can_view(&c.slug, &viewer, &map)
                })
                .filter_map(|c| {
                    let completed = c
                        .lessons
                        .iter()
                        .filter(|l| done.contains(&(c.slug.clone(), l.slug.clone())))
                        .count();
                    (completed > 0 && completed < c.lessons.len()).then_some((c, completed))
                })
                .collect()
        }
        None => Vec::new(),
    };
    let keep_going: AnyView = if in_progress.is_empty() {
        view! { <span hidden></span> }.into_any()
    } else {
        let items: Vec<AnyView> = in_progress
            .iter()
            .map(|(c, completed)| {
                let (label, href) = pages::continue_target(c, &done);
                let course_href = format!("/course/{}", c.slug);
                let title = c.title.clone();
                let meta = format!("{completed} of {} lessons", c.lessons.len());
                view! {
                    <li>
                        <a href={course_href}>{title}</a>
                        <span class="progress-text">{meta}</span>
                        <a class="button button-small" href={href}>{label}</a>
                    </li>
                }
                .into_any()
            })
            .collect();
        view! {
            <section class="profile-inprogress">
                <h2 class="section-title">"Keep going"</h2>
                <ul>{items}</ul>
            </section>
        }
        .into_any()
    };
    let content = if sess.is_some() {
        let name = user
            .as_ref()
            .map(|u| u.display())
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "Signed in".to_string());
        let email = user.as_ref().map(|u| u.email.clone()).unwrap_or_default();
        let since = user
            .as_ref()
            .and_then(|u| member_since(&u.created))
            .unwrap_or_default()
            .to_string();
        let email_line: AnyView = if email.is_empty() {
            view! { <span hidden></span> }.into_any()
        } else {
            view! { <p class="profile-email">{email}</p> }.into_any()
        };
        let since_line: AnyView = if since.is_empty() {
            view! { <span hidden></span> }.into_any()
        } else {
            view! { <p class="profile-member">{format!("Member since {since}")}</p> }.into_any()
        };
        let subs_text = subs.len().to_string();
        let done_text = done.len().to_string();
        let inprog_text = in_progress.len().to_string();
        vec![view! {
            <section class="auth-panel profile-panel">
                <h1>{name}</h1>
                {email_line}
                {since_line}
                {role_chips}
                <div class="profile-stats">
                    <span class="profile-stat">
                        <b>{subs_text}</b><span>"courses subscribed"</span>
                    </span>
                    <span class="profile-stat">
                        <b>{done_text}</b><span>"lessons completed"</span>
                    </span>
                    <span class="profile-stat">
                        <b>{inprog_text}</b><span>"in progress"</span>
                    </span>
                </div>
                {keep_going}
                <p>"Course progress is saved to your account."</p>
                <form method="post" action="/auth/logout" class="auth-form">
                    <input type="hidden" name="next" value={next.clone()} />
                    <button type="submit" class="button button-outline">"Sign out"</button>
                </form>
            </section>
        }
        .into_any()]
    } else {
        vec![
            view! {
                <section class="auth-panel">
                    <h1>"Sign in"</h1>
                    {error_view()}
                    <p class="auth-lede">
                        "Sign in to sync course progress across devices. Without an account, progress is kept in this browser only."
                    </p>
                    {oauth_block}
                    <form method="post" action="/auth/login" class="auth-form">
                        <input type="hidden" name="next" value={next.clone()} />
                        <label>
                            "Email"
                            <input type="email" name="email" required autocomplete="email" />
                        </label>
                        <label>
                            "Password"
                            <input
                                type="password"
                                name="password"
                                required
                                autocomplete="current-password"
                            />
                        </label>
                        <button type="submit" class="button button-primary">"Sign in"</button>
                    </form>
                </section>
            }
            .into_any(),
            
        ]
    };
    let title = "Account — Brillante".to_string();
    let signed_in = sess.is_some();
    let mut html = Html(render_document(move || {
        view! {
            <Shell title={title} auth_enabled={true} signed_in admin={is_admin} content />
        }
    }))
    .into_response();
    if let Some(cookie) = rotate {
        html.headers_mut()
            .append(header::SET_COOKIE, header_value(cookie.to_string()));
    }
    html
}

pub async fn login(
    State(state): State<AppState>,
    axum::Form(form): axum::Form<LoginForm>,
) -> Response {
    let Some(pb) = &state.pb else {
        return Redirect::to("/account?error=disabled").into_response();
    };
    match pb.auth_with_password(&form.email, &form.password).await {
        Ok((token, _user)) => {
            let secure = state.pb_config.as_ref().is_some_and(|c| c.secure_cookies);
            let mut resp = Redirect::to(&safe_next(form.next.as_ref())).into_response();
            let cookie = session_cookie(&token, secure);
            resp.headers_mut()
                .append(header::SET_COOKIE, header_value(cookie.to_string()));
            resp
        }
        Err(crate::pb::PbError::Rejected(_)) => {
            Redirect::to("/account?error=invalid").into_response()
        }
        Err(other) => {
            tracing::warn!("login failed: {other}");
            Redirect::to("/account?error=unavailable").into_response()
        }
    }
}

pub async fn signup(
    State(state): State<AppState>,
    axum::Form(form): axum::Form<SignupForm>,
) -> Response {
    let Some(pb) = &state.pb else {
        return Redirect::to("/account?error=disabled").into_response();
    };
    if form.password != form.password_confirm {
        return Redirect::to("/account?error=mismatch").into_response();
    }
    if let Err(e) = pb
        .create_user(&form.email, &form.password, &form.name)
        .await
    {
        return match e {
            crate::pb::PbError::Rejected(_) => {
                Redirect::to("/account?error=signup").into_response()
            }
            other => {
                tracing::warn!("signup failed: {other}");
                Redirect::to("/account?error=unavailable").into_response()
            }
        };
    }
    match pb.auth_with_password(&form.email, &form.password).await {
        Ok((token, _)) => {
            let secure = state.pb_config.as_ref().is_some_and(|c| c.secure_cookies);
            let mut resp = Redirect::to(&safe_next(form.next.as_ref())).into_response();
            let cookie = session_cookie(&token, secure);
            resp.headers_mut()
                .append(header::SET_COOKIE, header_value(cookie.to_string()));
            resp
        }
        Err(other) => {
            tracing::warn!("post-signup login failed: {other}");
            Redirect::to("/account?error=unavailable").into_response()
        }
    }
}

pub async fn logout(
    State(state): State<AppState>,
    axum::Form(form): axum::Form<NextParam>,
) -> Response {
    let _ = &state;
    let mut resp = Redirect::to(&safe_next(form.next.as_ref())).into_response();
    let cookie = cleared_cookie();
    resp.headers_mut()
        .append(header::SET_COOKIE, header_value(cookie.to_string()));
    resp
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(pb_cfg) = &state.pb_config else {
        return (
            StatusCode::OK,
            axum::Json(serde_json::json!({ "enabled": false, "authenticated": false })),
        )
            .into_response();
    };
    let pb = state.pb.clone().unwrap();
    let (sess, rotate) = session::resolve(&headers, &pb, pb_cfg.secure_cookies).await;
    let body = match sess {
        Some(s) => match pb.auth_refresh(&s.token).await {
            Ok((fresh_token, user)) => {
                let _ = rotate;
                let name = user.display();
                let roles = user.roles.clone();
                let mut resp = (
                    StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "enabled": true,
                        "authenticated": true,
                        "userId": user.id,
                        "name": name,
                        "roles": roles,
                    })),
                )
                    .into_response();
                if fresh_token != s.token {
                    let cookie = session_cookie(&fresh_token, pb_cfg.secure_cookies);
                    resp.headers_mut()
                        .append(header::SET_COOKIE, header_value(cookie.to_string()));
                }
                return resp;
            }
            Err(crate::pb::PbError::Unauthorized) => {
                let mut resp = (
                    StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "enabled": true,
                        "authenticated": false,
                    })),
                )
                    .into_response();
                resp.headers_mut()
                    .append(header::SET_COOKIE, header_value(cleared_cookie().to_string()));
                return resp;
            }
            Err(other) => {
                tracing::warn!("auth/me refresh failed: {other}");
                serde_json::json!({ "enabled": true, "authenticated": true })
            }
        },
        None => serde_json::json!({ "enabled": true, "authenticated": false }),
    };
    let mut resp = (StatusCode::OK, axum::Json(body)).into_response();
    if let Some(cookie) = rotate {
        resp.headers_mut()
            .append(header::SET_COOKIE, header_value(cookie.to_string()));
    }
    resp
}

pub(crate) fn header_value(raw: String) -> axum::http::HeaderValue {
    axum::http::HeaderValue::from_str(&raw).unwrap_or_else(|_| {
        axum::http::HeaderValue::from_static("brillante.session=; Path=/; HttpOnly")
    })
}

/// Date part of a PocketBase creation stamp ("YYYY-MM-DD HH:MM:SS.mmmZ").
fn member_since(created: &str) -> Option<&str> {
    created.get(..10).filter(|d| {
        d.len() == 10
            && d.as_bytes()[4] == b'-'
            && d.as_bytes()[7] == b'-'
            && d.bytes().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_next_allows_relative_only() {
        assert_eq!(safe_next(Some(&"/course/x".to_string())), "/course/x");
        assert_eq!(safe_next(Some(&"//evil.com".to_string())), "/");
        assert_eq!(safe_next(Some(&"http://evil.com".to_string())), "/");
        assert_eq!(safe_next(None), "/");
    }

    #[test]
    fn member_since_expects_pb_stamp() {
        assert_eq!(member_since("2026-10-08 10:03:11.456Z"), Some("2026-10-08"));
        assert_eq!(member_since(""), None);
        assert_eq!(member_since("nope"), None);
        assert_eq!(member_since("2026-10"), None);
    }
}
