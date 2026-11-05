use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::content::markdown;
use crate::pb::SubmissionDraft;
use crate::web::access;
use crate::web::assignments::{ANSWER_CHAR_LIMIT, not_found, reject, resolve_gated, widget_id_ok};
use crate::web::{AppState, grader, tutor};

/// The teach widget's config: `prompt` is required; `key_points` is the
/// optional grading bound. Key points are resolved here, server-side, and
/// are never part of the client's request.
fn teach_config(
    course: &crate::content::course::Course,
    widget: &str,
) -> Option<(String, Option<String>)> {
    let cfg = course.widget_config(widget)?;
    let prompt = cfg.get("prompt")?.as_str()?.to_string();
    let key_points = cfg
        .get("key_points")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some((prompt, key_points))
}

/// The grader's view of the lesson being taught: raw markdown, title
/// stripped (same as the tutor's context builder), bounded downstream.
fn lesson_context(lesson: &crate::content::course::Lesson) -> String {
    markdown::strip_title_heading(&lesson.body, &lesson.title)
}

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
    explanation: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct SubmissionQuery {
    course: String,
    lesson: String,
    widget: String,
}

/// `POST /api/teach/preview` — render a draft explanation to HTML for the
/// widget's Preview toggle. Open to anonymous visitors (grading is what
/// costs money); the body is never persisted.
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<PreviewRequest>,
) -> Response {
    if req.body.chars().count() > ANSWER_CHAR_LIMIT {
        return reject(StatusCode::BAD_REQUEST, "the explanation is too long");
    }
    let viewer = access::viewer(&state, &headers).await;
    let (course, lesson) = match resolve_gated(&state, &viewer, &req.course, &req.lesson).await {
        Ok(pair) => pair,
        Err(resp) => return *resp,
    };
    let html = tutor::render_reply(&state, &course, &lesson, &req.body);
    axum::Json(serde_json::json!({ "html": html })).into_response()
}

/// `POST /api/teach/grade` — grade one teaching explanation through the
/// configured grader (lesson content as context, Decisions model for the
/// verdict, chat model for the feedback) and persist it. Signed-in only.
pub async fn grade(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<GradeRequest>,
) -> Response {
    if !widget_id_ok(&req.widget) {
        return not_found("unknown teach widget");
    }
    if req.explanation.trim().is_empty() {
        return reject(
            StatusCode::BAD_REQUEST,
            "write an explanation before submitting",
        );
    }
    if req.explanation.chars().count() > ANSWER_CHAR_LIMIT {
        return reject(StatusCode::BAD_REQUEST, "the explanation is too long");
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
    let Some((prompt, key_points)) = teach_config(&course, &req.widget) else {
        return not_found("unknown teach widget");
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

    let lesson_body = lesson_context(&lesson);
    let problem = grader::TeachProblem {
        prompt: &prompt,
        lesson_title: &lesson.title,
        lesson_body: &lesson_body,
        key_points: key_points.as_deref(),
        explanation: &req.explanation,
    };
    let grading = match grader::grade_teaching(&settings, &api_key, &problem).await {
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
        answer: &req.explanation,
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
    let key_points_html = key_points
        .as_deref()
        .map(|k| tutor::render_reply(&state, &course, &lesson, k));
    axum::Json(serde_json::json!({
        "verdict": grading.verdict.as_str(),
        "score": grading.score,
        "feedback": grading.feedback,
        "feedback_html": feedback_html,
        "attempt": attempt,
        "key_points_html": key_points_html,
    }))
    .into_response()
}

/// `GET /api/teach/submission?course=&lesson=&widget=` — the signed-in
/// learner's latest teaching attempt, or `null`. A returned row re-unlocks
/// the verdict, feedback, and key-points panels after a page refresh.
pub async fn submission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SubmissionQuery>,
) -> Response {
    if !widget_id_ok(&q.widget) {
        return not_found("unknown teach widget");
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
    let Some((_, key_points)) = teach_config(&course, &q.widget) else {
        return not_found("unknown teach widget");
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
    let key_points_html = key_points
        .as_deref()
        .map(|k| tutor::render_reply(&state, &course, &lesson, k));
    axum::Json(serde_json::json!({
        "answer": row.answer,
        "verdict": row.verdict,
        "score": row.score,
        "feedback_html": feedback_html,
        "key_points_html": key_points_html,
        "attempt": row.attempt.unwrap_or(1.0),
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::course::{Course, Lesson};

    fn course_with_config(config_toml: &str) -> Course {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("l.md"), "---\ntitle: L\n---\n").unwrap();
        std::fs::write(dir.path().join("config.toml"), config_toml).unwrap();
        let mut diags = Vec::new();
        // The loaded Course keeps its config table in memory; the tempdir may
        // be deleted after this returns.
        crate::content::course::load_course_dir(dir.path(), &mut diags).unwrap()
    }

    #[test]
    fn teach_config_reads_optional_key_points() {
        let course = course_with_config(
            "[widgets.\"com.x.t\"]\nprompt = \"teach it\"\nkey_points = \"the main idea\"\n",
        );
        let (prompt, key_points) = teach_config(&course, "com.x.t").unwrap();
        assert_eq!(prompt, "teach it");
        assert_eq!(key_points.as_deref(), Some("the main idea"));

        let course = course_with_config("[widgets.\"com.x.t\"]\nprompt = \"teach it\"\n");
        let (prompt, key_points) = teach_config(&course, "com.x.t").unwrap();
        assert_eq!(prompt, "teach it");
        assert_eq!(key_points, None);

        let course = course_with_config("[widgets.\"com.x.t\"]\nprompt = \"teach it\"\n");
        assert!(teach_config(&course, "com.x.absent").is_none());
    }

    #[test]
    fn lesson_context_strips_the_title_heading() {
        let lesson = Lesson {
            slug: "l".into(),
            title: "The Lesson".into(),
            description: None,
            order: 0.0,
            file_name: "l.md".into(),
            section: None,
            body: "# The Lesson\n\nBody text.".into(),
        };
        let ctx = lesson_context(&lesson);
        assert_eq!(ctx, "Body text.");
    }
}
