use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};

use crate::pb::PbError;
use crate::web::access;
use crate::web::auth::safe_next;
use crate::web::pages::urlencode;
use crate::web::progress::slug_ok;
use crate::web::session::cleared_cookie;
use crate::web::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct SubscriptionForm {
    course: String,
    action: String,
    next: Option<String>,
}

/// Subscribe / unsubscribe form endpoint (works without JS, like /auth/*).
/// Access is re-checked server-side before the row is written: a user who
/// cannot view a course cannot subscribe to it either.
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<SubscriptionForm>,
) -> Response {
    let next = safe_next(form.next.as_ref());
    let back_with_error = || Redirect::to(&format!("{next}?error=subscribe")).into_response();

    let viewer = access::viewer(&state, &headers).await;
    let Some(sess) = viewer.session.clone() else {
        return Redirect::to(&format!(
            "/account?next={}",
            urlencode(&next)
        ))
        .into_response();
    };
    let Some(pb) = &state.pb else {
        return back_with_error();
    };
    if !slug_ok(&form.course) {
        return back_with_error();
    }
    let map = state.access.get(state.pb.as_ref()).await;
    if !access::can_view(&form.course, &viewer, &map) {
        // Do not confirm the course exists.
        return back_with_error();
    }
    let result = match form.action.as_str() {
        "subscribe" => pb.subscribe(&sess.token, &sess.user_id, &form.course).await,
        "unsubscribe" => pb
            .unsubscribe(&sess.token, &sess.user_id, &form.course)
            .await,
        _ => Err(PbError::Rejected("unknown action".into())),
    };
    match result {
        Ok(()) => Redirect::to(&next).into_response(),
        Err(PbError::Unauthorized) => {
            let mut resp = Redirect::to(&format!("/account?next={}", urlencode(&next))).into_response();
            resp.headers_mut().append(
                axum::http::header::SET_COOKIE,
                crate::web::auth::header_value(cleared_cookie().to_string()),
            );
            resp
        }
        Err(e) => {
            tracing::warn!("subscription update failed: {e}");
            back_with_error()
        }
    }
}
