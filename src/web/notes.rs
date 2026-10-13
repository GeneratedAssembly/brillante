use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use leptos::prelude::*;

use crate::content::course::{Course, Lesson};
use crate::content::markdown::{self, MarkdownCtx, NoteHere};
use crate::pb::PbError;
use crate::render::{render_document, render_html};
use crate::web::access::{self, Viewer};
use crate::web::pages::Shell;
use crate::web::progress::slug_ok;
use crate::web::AppState;

#[derive(Debug, Default, serde::Deserialize)]
pub struct NoteForm {
    body: String,
    #[serde(default)]
    anchor: String,
    #[serde(default)]
    here_title: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct PreviewRequest {
    course: String,
    lesson: String,
    body: String,
    #[serde(default)]
    anchor: String,
    #[serde(default)]
    here_title: String,
}

fn lessons_by_file(course: &Course) -> HashMap<String, String> {
    course
        .lessons
        .iter()
        .map(|l| (l.file_name.clone(), l.slug.clone()))
        .collect()
}

/// Shorthand-expanded, widget-suppressed markdown views for one note.
/// Loader diagnostics are deliberately dropped (per-user content is not part
/// of them).
pub(crate) fn render_note_views(
    state: &AppState,
    course: &Course,
    lesson: &Lesson,
    body: &str,
    anchor: &str,
    here_title: &str,
) -> Vec<AnyView> {
    let here = (!anchor.is_empty() || !here_title.is_empty()).then(|| NoteHere {
        lesson: lesson.slug.as_str(),
        anchor: (!anchor.is_empty()).then_some(anchor),
        title: (!here_title.is_empty()).then_some(here_title),
    });
    let expanded = markdown::expand_note_links(body, &course.lessons, here);
    let by_file = lessons_by_file(course);
    let ctx = MarkdownCtx {
        course,
        source: std::path::Path::new(&lesson.file_name),
        lessons_by_file: &by_file,
        widgets: false,
    };
    let mut diags = Vec::new();
    let output = markdown::render_markdown(&expanded, &ctx, &state.registry, &mut diags);
    for d in diags {
        tracing::debug!("note render: {d}");
    }
    output.views
}

pub(crate) fn render_note_body(
    state: &AppState,
    course: &Course,
    lesson: &Lesson,
    body: &str,
    anchor: &str,
    here_title: &str,
) -> String {
    render_note_views(state, course, lesson, body, anchor, here_title)
        .into_iter()
        .map(|v| render_html(move || v))
        .collect::<Vec<_>>()
        .join("")
}

/// Anchor is only meaningful while the heading still exists in the lesson;
/// unknown ids are dropped at save time (never rejected mid-edit).
pub(crate) fn sanitize_anchor(lesson: &Lesson, anchor: &str, here_title: &str) -> (String, String) {
    if anchor.is_empty() {
        return (String::new(), String::new());
    }
    let body = markdown::strip_title_heading(&lesson.body, &lesson.title);
    if markdown::heading_ids(&body).contains(anchor) {
        (anchor.to_string(), here_title.to_string())
    } else {
        (String::new(), String::new())
    }
}

/// Shared gate for all note surfaces: signed in, course + lesson exist, and
/// the viewer may see the course. Returns the viewer on success.
async fn viewer_for(state: &AppState, headers: &HeaderMap) -> Result<Viewer, PbError> {
    if state.pb_config.is_none() {
        return Err(PbError::Unauthorized);
    }
    let viewer = access::viewer(state, headers).await;
    if !viewer.signed_in() {
        return Err(PbError::Unauthorized);
    }
    Ok(viewer)
}

/// The lesson-page note drawer: saved note (rendered), edit form, and the
/// autocomplete payload. `status` is the post-save redirect flag.
pub(crate) fn drawer(
    state: &AppState,
    course: &Course,
    lesson: &Lesson,
    entry: Option<&crate::pb::NotebookEntry>,
    status: Option<&str>,
) -> AnyView {
    let saved: AnyView = match entry {
        Some(e) if !e.body.trim().is_empty() => {
            let views = render_note_views(state, course, lesson, &e.body, &e.anchor, &e.here_title);
            view! {
                <div class="notebook-saved">
                    <h3>"Saved note"</h3>
                    <div class="note-markdown">{views}</div>
                </div>
            }
            .into_any()
        }
        _ => view! { <span hidden></span> }.into_any(),
    };
    let raw = entry.map(|e| e.body.clone()).unwrap_or_default();
    let anchor = entry.map(|e| e.anchor.clone()).unwrap_or_default();
    let here = entry.map(|e| e.here_title.clone()).unwrap_or_default();
    let action = format!(
        "/notes/{}/{}",
        crate::web::pages::urlencode(&course.slug),
        crate::web::pages::urlencode(&lesson.slug)
    );
    let index_href = format!("/notes/{}", crate::web::pages::urlencode(&course.slug));
    let lessons_json = serde_json::to_string(
        &course
            .lessons
            .iter()
            .map(|l| serde_json::json!({ "slug": l.slug, "title": l.title }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    let status_view: AnyView = match status {
        Some("saved") => view! { <span class="notebook-status">"Note saved."</span> }.into_any(),
        Some("removed") => {
            view! { <span class="notebook-status">"Note removed."</span> }.into_any()
        }
        Some("error") => view! {
            <span class="notebook-status notebook-status-error">"Could not save the note."</span>
        }
        .into_any(),
        _ => view! { <span hidden></span> }.into_any(),
    };
    view! {
        <aside id="notebook" aria-label="Lesson notes">
            <div class="notebook-head">
                <span class="notebook-title">"Note"</span>
                {status_view}
                <a class="notebook-index" href={index_href}>"All notes"</a>
                <button type="button" class="notebook-btn" data-notebook-close>"Close"</button>
            </div>
            {saved}
            <form method="post" action={action} class="notebook-form">
                <textarea
                    name="body"
                    class="notebook-input"
                    data-notebook-input
                    rows="6"
                    placeholder="Write a note. @ links sections, [[ links lessons."
                >{raw}</textarea>
                <input type="hidden" name="anchor" data-notebook-anchor value={anchor} />
                <input type="hidden" name="here_title" data-notebook-here value={here} />
                <div class="notebook-actions">
                    <button type="button" class="notebook-btn" data-notebook-preview>"Preview"</button>
                    <button type="submit" class="notebook-btn">"Save"</button>
                </div>
                <div class="notebook-preview note-markdown" data-notebook-preview-out hidden></div>
            </form>
            <div class="notebook-complete" data-notebook-complete hidden></div>
            <script type="application/json" data-notebook-lessons>{lessons_json}</script>
        </aside>
        <button
            id="notebook-toggle"
            type="button"
            aria-controls="notebook"
            aria-expanded="false"
            hidden
        >
            "Note"
        </button>
    }
    .into_any()
}

async fn note_here_ok(state: &AppState, viewer: &Viewer, course_slug: &str) -> bool {
    let map = state.access.get(state.pb.as_ref()).await;
    access::can_view(course_slug, viewer, &map)
}

/// `GET /notes/{course}` — the user's notebook index for one course, in
/// lesson order; orphan entries surface with a "lesson removed" chip.
pub async fn index(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    if state.pb_config.is_none() || !slug_ok(&slug) {
        return crate::web::pages::not_found_page();
    }
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&slug) else {
        return crate::web::pages::not_found_page();
    };
    let viewer = access::viewer(&state, &headers).await;
    if !viewer.signed_in() {
        return Redirect::to(&format!(
            "/account?next=/notes/{}",
            crate::web::pages::urlencode(&slug)
        ))
        .into_response();
    }
    if !note_here_ok(&state, &viewer, &slug).await {
        return crate::web::pages::not_found_page();
    }
    let sess = viewer.session.as_ref().unwrap();
    let entries = match state
        .pb
        .as_ref()
        .unwrap()
        .list_notebook_entries(&sess.token, &sess.user_id, &slug)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("notebook list failed for /notes/{slug}: {e}");
            Vec::new()
        }
    };
    let by_lesson: HashMap<&str, &crate::pb::NotebookEntry> = entries
        .iter()
        .map(|e| (e.lesson.as_str(), e))
        .collect();

    let mut sections: Vec<AnyView> = Vec::new();
    let mut rendered_lessons: std::collections::HashSet<String> = std::collections::HashSet::new();
    for lesson in &course.lessons {
        let Some(entry) = by_lesson.get(lesson.slug.as_str()) else {
            continue;
        };
        rendered_lessons.insert(lesson.slug.clone());
        let href = format!("/course/{}/{}", course.slug, lesson.slug);
        let lesson_title = lesson.title.clone();
        let views = render_note_views(
            &state,
            course,
            lesson,
            &entry.body,
            &entry.anchor,
            &entry.here_title,
        );
        sections.push(
            view! {
                <section class="notebook-entry">
                    <h2 class="section-title">
                        <a href={href}>{lesson_title}</a>
                    </h2>
                    <div class="notebook-entry-body note-markdown">{views}</div>
                </section>
            }
            .into_any(),
        );
    }
    for entry in &entries {
        if rendered_lessons.contains(&entry.lesson) {
            continue;
        }
        let lesson_title = entry.lesson.clone();
        sections.push(
            view! {
                <section class="notebook-entry notebook-orphan">
                    <h2 class="section-title">
                        {lesson_title}
                        <span class="chip chip-private">"lesson removed"</span>
                    </h2>
                </section>
            }
            .into_any(),
        );
    }

    let body: AnyView = if sections.is_empty() {
        view! {
            <p class="browse-empty">
                "No notes yet in this course. Open a lesson and use the Note panel."
            </p>
        }
        .into_any()
    } else {
        view! { <div>{sections}</div> }.into_any()
    };

    let course_title = course.title.clone();
    let outline_href = format!("/course/{}", course.slug);
    let content = vec![
        view! {
            <section class="browse-head">
                <h1>{format!("Notes — {course_title}")}</h1>
                <p class="hero-sub">
                    "Your notebook for "<a href={outline_href}>"the course outline"</a>"."
                </p>
            </section>
        }
        .into_any(),
        body,
    ];
    let title = format!("Notes — {} — Brillante", course.title);
    let auth_enabled = state.pb_config.is_some();
    let signed_in = true;
    let admin = viewer.is_admin();
    Html(render_document(move || {
        view! {
            <Shell title={title} auth_enabled signed_in admin={admin} content />
        }
    }))
    .into_response()
}

/// `POST /notes/{course}/{lesson}` — create or replace the user's note;
/// an empty body deletes it. Always redirects back to the lesson page.
pub async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_slug, lesson_slug)): Path<(String, String)>,
    axum::Form(form): axum::Form<NoteForm>,
) -> Response {
    let back = |flag: &str| {
        Redirect::to(&format!(
            "/course/{}/{}?notes={}",
            crate::web::pages::urlencode(&course_slug),
            crate::web::pages::urlencode(&lesson_slug),
            flag
        ))
        .into_response()
    };
    if state.pb_config.is_none() {
        return crate::web::pages::not_found_page();
    }
    if !slug_ok(&course_slug) || !slug_ok(&lesson_slug) {
        return back("error");
    }
    let viewer = match viewer_for(&state, &headers).await {
        Ok(v) => v,
        Err(_) => return Redirect::to(&format!(
            "/account?next=/course/{}/{}",
            crate::web::pages::urlencode(&course_slug),
            crate::web::pages::urlencode(&lesson_slug)
        ))
        .into_response(),
    };
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&course_slug) else {
        return crate::web::pages::not_found_page();
    };
    let Some(lesson) = course.lesson(&lesson_slug) else {
        return crate::web::pages::not_found_page();
    };
    if !note_here_ok(&state, &viewer, &course_slug).await {
        return crate::web::pages::not_found_page();
    }
    let sess = viewer.session.as_ref().unwrap();
    let pb = state.pb.as_ref().unwrap();
    let body = form.body.trim();
    if body.is_empty() {
        let existing = pb
            .get_notebook_entry(&sess.token, &sess.user_id, &course_slug, &lesson_slug)
            .await;
        match existing {
            Ok(Some(entry)) => {
                if let Err(e) = pb.delete_notebook_entry(&sess.token, &entry.id).await {
                    tracing::warn!("note delete failed: {e}");
                    return back("error");
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("note lookup before delete failed: {e}");
                return back("error");
            }
        }
        return back("removed");
    }
    let (anchor, here_title) = sanitize_anchor(lesson, form.anchor.trim(), form.here_title.trim());
    if body.len() > 65536 {
        return back("error");
    }
    let draft = crate::pb::NoteDraft {
        course: &course_slug,
        lesson: &lesson_slug,
        anchor: &anchor,
        here_title: &here_title,
        body,
    };
    match pb.upsert_notebook_entry(&sess.token, &sess.user_id, &draft).await {
        Ok(()) => back("saved"),
        Err(e) => {
            tracing::warn!("note save failed for {course_slug}/{lesson_slug}: {e}");
            back("error")
        }
    }
}

/// `POST /api/notes/preview` — render a draft note to an HTML fragment for
/// the drawer's Preview button.
pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<PreviewRequest>,
) -> Result<Response, PbError> {
    if !slug_ok(&req.course) || !slug_ok(&req.lesson) {
        return Err(PbError::Rejected("unknown course or lesson".into()));
    }
    let viewer = viewer_for(&state, &headers).await?;
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&req.course) else {
        return Err(PbError::Rejected("unknown course".into()));
    };
    let Some(lesson) = course.lesson(&req.lesson) else {
        return Err(PbError::Rejected("unknown lesson".into()));
    };
    if !note_here_ok(&state, &viewer, &req.course).await {
        return Err(PbError::Unauthorized);
    }
    let (anchor, here_title) = sanitize_anchor(lesson, &req.anchor, &req.here_title);
    let html = render_note_body(&state, course, lesson, &req.body, &anchor, &here_title);
    Ok((
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}
