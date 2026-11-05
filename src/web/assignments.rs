use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::content::course::{Course, Lesson};
use crate::pb::SubmissionDraft;
use crate::web::access::{self, Viewer};
use crate::web::progress::slug_ok;
use crate::web::{AppState, grader, tutor};

pub(crate) const ANSWER_CHAR_LIMIT: usize = 8_000;

#[derive(Debug, serde::Deserialize)]
pub struct PreviewRequest {
    course: String,
    lesson: String,
    body: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct GradeRequest {
    course: String,
    lesson: String,
    widget: String,
    answer: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SubmissionQuery {
    course: String,
    lesson: String,
    widget: String,
}

pub(crate) fn reject(status: StatusCode, msg: &str) -> Response {
    (status, axum::Json(serde_json::json!({ "error": msg }))).into_response()
}

pub(crate) fn not_found(msg: &str) -> Response {
    reject(StatusCode::NOT_FOUND, msg)
}

/// Widget ids are reverse-DNS config keys (letters, digits, `.`, `-`, `_`).
pub(crate) fn widget_id_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 300
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Resolve the gated (course, lesson) pair shared by every assignment
/// surface: slugs validated, course + lesson exist, viewer may see them.
pub(crate) async fn resolve_gated(
    state: &AppState,
    viewer: &Viewer,
    course_slug: &str,
    lesson_slug: &str,
) -> Result<(Course, Lesson), Box<Response>> {
    if !slug_ok(course_slug) || !slug_ok(lesson_slug) {
        return Err(Box::new(not_found("unknown course or lesson")));
    }
    let store = state.store.read().await.clone();
    let Some(course) = store.course(course_slug) else {
        return Err(Box::new(not_found("unknown course")));
    };
    let Some(lesson) = course.lesson(lesson_slug) else {
        return Err(Box::new(not_found("unknown lesson")));
    };
    let map = state.access.get(state.pb.as_ref()).await;
    if !access::can_view(&course.slug, viewer, &map) {
        return Err(Box::new(not_found("unknown course")));
    }
    Ok((course.clone(), lesson.clone()))
}

/// The widget's assignment config: it must exist and carry the two required
/// string keys (`reference` is resolved here, server-side, and is never part
/// of the client's request).
fn assignment_config(course: &Course, widget: &str) -> Option<(String, String)> {
    let cfg = course.widget_config(widget)?;
    let prompt = cfg.get("prompt")?.as_str()?.to_string();
    let reference = cfg.get("reference")?.as_str()?.to_string();
    Some((prompt, reference))
}

/// `POST /api/assignment/preview` — render a draft answer to HTML for the
/// widget's Preview toggle. Open to anonymous visitors (grading is what
/// costs money); the body is never persisted.
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<PreviewRequest>,
) -> Response {
    if req.body.chars().count() > ANSWER_CHAR_LIMIT {
        return reject(StatusCode::BAD_REQUEST, "the answer is too long");
    }
    let viewer = access::viewer(&state, &headers).await;
    let (course, lesson) = match resolve_gated(&state, &viewer, &req.course, &req.lesson).await {
        Ok(pair) => pair,
        Err(resp) => return *resp,
    };
    let html = tutor::render_reply(&state, &course, &lesson, &req.body);
    axum::Json(serde_json::json!({ "html": html })).into_response()
}

/// `POST /api/assignment/grade` — grade one answer through the configured
/// grader and persist the verdict. Signed-in only; the reference solution is
/// resolved from the course config and returned only as part of a graded
/// attempt's response.
pub async fn grade(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<GradeRequest>,
) -> Response {
    if !widget_id_ok(&req.widget) {
        return not_found("unknown assignment");
    }
    if req.answer.trim().is_empty() {
        return reject(StatusCode::BAD_REQUEST, "write an answer before submitting");
    }
    if req.answer.chars().count() > ANSWER_CHAR_LIMIT {
        return reject(StatusCode::BAD_REQUEST, "the answer is too long");
    }
    let Some(pb) = state.pb.as_ref() else {
        return reject(StatusCode::UNAUTHORIZED, "authentication required");
    };
    let settings = state.grader.settings(Some(pb)).await;
    if !settings.ready() {
        return not_found("grading is not available");
    }
    let viewer = access::viewer(&state, &headers).await;
    if !viewer.signed_in() {
        return reject(StatusCode::UNAUTHORIZED, "authentication required");
    }
    let (course, lesson) = match resolve_gated(&state, &viewer, &req.course, &req.lesson).await {
        Ok(pair) => pair,
        Err(resp) => return *resp,
    };
    let Some((prompt, reference)) = assignment_config(&course, &req.widget) else {
        return not_found("unknown assignment");
    };
    let Some(api_key) = (match state.grader.api_key(pb).await {
        Ok(key) => key,
        Err(e) => {
            tracing::warn!("grader key fetch failed: {e}");
            None
        }
    }) else {
        return reject(
            StatusCode::SERVICE_UNAVAILABLE,
            "the grader is temporarily unavailable",
        );
    };

    let problem = grader::GradedProblem {
        problem: &prompt,
        reference: &reference,
        answer: &req.answer,
    };
    let grading = match grader::grade(&settings, &api_key, &problem).await {
        Ok(g) => g,
        Err(grader::GradeError::Upstream) => {
            return reject(
                StatusCode::BAD_GATEWAY,
                "could not reach the grading service",
            );
        }
        Err(grader::GradeError::Unparsable) => {
            return reject(
                StatusCode::BAD_GATEWAY,
                "the grader returned an unreadable verdict; please try again",
            );
        }
    };

    let session = viewer.session.as_ref().expect("signed-in viewer");
    let previous = pb
        .get_assignment_submission(
            &session.token,
            &session.user_id,
            &course.slug,
            &lesson.slug,
            &req.widget,
        )
        .await;
    let attempt = match &previous {
        Ok(Some(row)) => row.attempt.unwrap_or(0.0) as u32 + 1,
        Ok(None) => 1,
        Err(e) => {
            tracing::warn!("submission lookup failed: {e}");
            return reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "the submission store is temporarily unavailable",
            );
        }
    };
    let draft = SubmissionDraft {
        course: &course.slug,
        lesson: &lesson.slug,
        widget: &req.widget,
        answer: &req.answer,
        verdict: grading.verdict.as_str(),
        score: grading.score,
        feedback: &grading.feedback,
        attempt,
    };
    if let Err(e) = pb
        .upsert_assignment_submission(&session.token, &session.user_id, &draft)
        .await
    {
        tracing::warn!("submission save failed: {e}");
        return reject(
            StatusCode::SERVICE_UNAVAILABLE,
            "the submission store is temporarily unavailable",
        );
    }

    let feedback_html = tutor::render_reply(&state, &course, &lesson, &grading.feedback);
    let reference_html = tutor::render_reply(&state, &course, &lesson, &reference);
    axum::Json(serde_json::json!({
        "verdict": grading.verdict.as_str(),
        "score": grading.score,
        "feedback": grading.feedback,
        "feedback_html": feedback_html,
        "attempt": attempt,
        "reference_html": reference_html,
    }))
    .into_response()
}

/// `GET /api/assignment/submission?course=&lesson=&widget=` — the signed-in
/// learner's latest submission row, or `null`. A returned row re-unlocks the
/// verdict, feedback, and reference panels after a page refresh.
pub async fn submission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SubmissionQuery>,
) -> Response {
    if !widget_id_ok(&q.widget) {
        return not_found("unknown assignment");
    }
    let Some(pb) = state.pb.as_ref() else {
        return reject(StatusCode::UNAUTHORIZED, "authentication required");
    };
    let viewer = access::viewer(&state, &headers).await;
    if !viewer.signed_in() {
        return reject(StatusCode::UNAUTHORIZED, "authentication required");
    }
    let (course, lesson) = match resolve_gated(&state, &viewer, &q.course, &q.lesson).await {
        Ok(pair) => pair,
        Err(resp) => return *resp,
    };
    let Some((_prompt, reference)) = assignment_config(&course, &q.widget) else {
        return not_found("unknown assignment");
    };
    let session = viewer.session.as_ref().expect("signed-in viewer");
    let row = match pb
        .get_assignment_submission(
            &session.token,
            &session.user_id,
            &course.slug,
            &lesson.slug,
            &q.widget,
        )
        .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!("submission fetch failed: {e}");
            return reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "the submission store is temporarily unavailable",
            );
        }
    };
    let Some(row) = row else {
        return axum::Json(serde_json::Value::Null).into_response();
    };

    let feedback_html = tutor::render_reply(&state, &course, &lesson, &row.feedback);
    let reference_html = tutor::render_reply(&state, &course, &lesson, &reference);
    axum::Json(serde_json::json!({
        "answer": row.answer,
        "verdict": row.verdict,
        "score": row.score,
        "feedback_html": feedback_html,
        "reference_html": reference_html,
        "attempt": row.attempt.unwrap_or(1.0),
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_ids_are_reverse_dns_keys() {
        assert!(widget_id_ok("com.github.richbai90.stats.08-assign-1a.a1"));
        assert!(widget_id_ok("a"));
        assert!(!widget_id_ok(""));
        assert!(!widget_id_ok("has space"));
        assert!(!widget_id_ok("quote\"id"));
        assert!(!widget_id_ok(&"x".repeat(301)));
    }
}
