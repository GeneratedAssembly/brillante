use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use leptos::prelude::*;

use crate::content::course::{Course, Lesson};
use crate::content::markdown::{self, MarkdownCtx, NoteHere};
use crate::pb::PbError;
use crate::render::{render_document, render_html};
use crate::web::AppState;
use crate::web::access::{self, Viewer};
use crate::web::pages::Shell;
use crate::web::progress::slug_ok;

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
        ai_locked: false,
        auth_available: false,
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

/// Drawer entries in saved order (oldest first; `created` then id, both
/// PB-assigned and stable lexicographically).
pub(crate) fn sorted_entries(
    entries: &[crate::pb::NotebookEntry],
) -> Vec<&crate::pb::NotebookEntry> {
    let mut refs: Vec<&crate::pb::NotebookEntry> = entries.iter().collect();
    refs.sort_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)));
    refs
}

/// PB timestamps render as `YYYY-MM-DD HH:MM:SS.000Z`; the date prefix is
/// enough for a notebook listing.
fn note_date(ts: &str) -> String {
    ts.get(..10).unwrap_or(ts).to_string()
}

/// PB record ids are plain alphanumerics; reject anything else before the
/// value reaches a PB URL path.
fn entry_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The lesson-page note drawer: saved notes (rendered, each with edit and
/// delete controls), the composer, and the autocomplete payload. `status` is
/// the post-save redirect flag; `edit_id` (from `?note=`) opens the composer
/// over one saved note.
pub(crate) fn drawer(
    state: &AppState,
    course: &Course,
    lesson: &Lesson,
    entries: &[crate::pb::NotebookEntry],
    status: Option<&str>,
    edit_id: Option<&str>,
) -> AnyView {
    let sorted = sorted_entries(entries);
    let editing = edit_id.and_then(|id| sorted.iter().copied().find(|e| e.id == id));
    let course_esc = crate::web::pages::urlencode(&course.slug);
    let lesson_esc = crate::web::pages::urlencode(&lesson.slug);
    let create_action = format!("/notes/{course_esc}/{lesson_esc}");
    let index_href = format!("/notes/{course_esc}");
    let lesson_url = format!("/course/{course_esc}/{lesson_esc}");

    let raw = editing.map(|e| e.body.clone()).unwrap_or_default();
    let anchor = editing.map(|e| e.anchor.clone()).unwrap_or_default();
    let here = editing.map(|e| e.here_title.clone()).unwrap_or_default();
    let action = match editing {
        Some(e) => format!("{create_action}/{}", e.id),
        None => create_action.clone(),
    };

    let items: Vec<AnyView> = sorted
        .iter()
        .filter(|e| !e.body.trim().is_empty())
        .map(|e| {
            let views = render_note_views(state, course, lesson, &e.body, &e.anchor, &e.here_title);
            let edit_href = format!("?note={}", e.id);
            let delete_action = format!("{create_action}/{}/delete", e.id);
            let here_chip: Option<AnyView> = if e.anchor.is_empty() {
                None
            } else {
                let label = if e.here_title.is_empty() {
                    format!("§ {}", e.anchor)
                } else {
                    format!("§ {}", e.here_title)
                };
                let jump = format!("{lesson_url}#{}", e.anchor);
                Some(
                    view! {
                        <a class="notebook-here" href={jump}>{label}</a>
                    }
                    .into_any(),
                )
            };
            let item_raw = e.body.clone();
            let item_anchor = e.anchor.clone();
            let item_here = e.here_title.clone();
            view! {
                <div
                    class="notebook-item"
                    data-notebook-entry-item=""
                    data-notebook-entry={e.id.clone()}
                    data-entry-anchor={item_anchor}
                    data-entry-here={item_here}
                >
                    <div class="notebook-item-meta">
                        <span class="notebook-item-date">{note_date(&e.created)}</span>
                        {here_chip}
                        <span class="notebook-item-actions">
                            <a class="notebook-mini" data-notebook-edit data-notebook-entry={e.id.clone()} href={edit_href}>"Edit"</a>
                            <form method="post" action={delete_action} data-notebook-delete>
                                <button type="submit" class="notebook-mini">"Delete"</button>
                            </form>
                        </span>
                    </div>
                    <div class="note-markdown">{views}</div>
                    <textarea class="notebook-raw" data-notebook-raw data-notebook-entry={e.id.clone()} hidden>{item_raw}</textarea>
                </div>
            }
            .into_any()
        })
        .collect();

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
    let aside_class = if editing.is_some() { "open" } else { "" };
    let editing_hidden = editing.is_none();
    view! {
        <aside id="notebook" class={aside_class} aria-label="Lesson notes">
            <div class="notebook-head">
                <span class="notebook-title">"Notes"</span>
                {status_view}
                <a class="notebook-index" href={index_href}>"All notes"</a>
                <button type="button" class="notebook-btn" data-notebook-close>"Close"</button>
            </div>
            <div class="notebook-items">{items}</div>
            <div class="notebook-editing" data-notebook-editing hidden={editing_hidden}>
                <span>"Editing a saved note — Save replaces it. Clear the text and Save to delete it."</span>
                <a class="notebook-mini" data-notebook-new href={lesson_url}>"New note"</a>
            </div>
            <form
                method="post"
                action={action}
                class="notebook-form"
                data-notebook-form
                data-notebook-create-action={create_action}
            >
                <textarea
                    name="body"
                    class="notebook-input"
                    data-notebook-input
                    rows="6"
                    placeholder="Write a note. Saving adds one. @ links sections, [[ links lessons."
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
            "Notes"
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
    let mut by_lesson: HashMap<&str, Vec<&crate::pb::NotebookEntry>> = HashMap::new();
    for entry in &entries {
        by_lesson
            .entry(entry.lesson.as_str())
            .or_default()
            .push(entry);
    }
    for group in by_lesson.values_mut() {
        group.sort_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)));
    }

    let mut sections: Vec<AnyView> = Vec::new();
    let mut rendered_lessons: std::collections::HashSet<String> = std::collections::HashSet::new();
    for lesson in &course.lessons {
        let Some(group) = by_lesson.get(lesson.slug.as_str()) else {
            continue;
        };
        rendered_lessons.insert(lesson.slug.clone());
        let href = format!("/course/{}/{}", course.slug, lesson.slug);
        let lesson_title = lesson.title.clone();
        let notes: Vec<AnyView> = group
            .iter()
            .map(|entry| {
                let views = render_note_views(
                    &state,
                    course,
                    lesson,
                    &entry.body,
                    &entry.anchor,
                    &entry.here_title,
                );
                let date = note_date(&entry.created);
                let here_chip: Option<AnyView> = if entry.anchor.is_empty() {
                    None
                } else {
                    let label = if entry.here_title.is_empty() {
                        format!("§ {}", entry.anchor)
                    } else {
                        format!("§ {}", entry.here_title)
                    };
                    let jump = format!("{href}#{}", entry.anchor);
                    Some(view! { <a class="notebook-here" href={jump}>{label}</a> }.into_any())
                };
                view! {
                    <article class="notebook-entry-note">
                        <div class="notebook-item-meta">
                            <span class="notebook-item-date">{date}</span>
                            {here_chip}
                        </div>
                        <div class="note-markdown">{views}</div>
                    </article>
                }
                .into_any()
            })
            .collect();
        sections.push(
            view! {
                <section class="notebook-entry">
                    <h2 class="section-title">
                        <a href={href}>{lesson_title}</a>
                    </h2>
                    {notes}
                </section>
            }
            .into_any(),
        );
    }
    let mut orphans: Vec<(&str, usize)> = by_lesson
        .iter()
        .filter(|(lesson, _)| !rendered_lessons.contains(**lesson))
        .map(|(lesson, group)| (*lesson, group.len()))
        .collect();
    orphans.sort();
    for (lesson_slug, count) in orphans {
        let lesson_title = lesson_slug.to_string();
        let count_label = format!("{count} note{}", if count == 1 { "" } else { "s" });
        sections.push(
            view! {
                <section class="notebook-entry notebook-orphan">
                    <h2 class="section-title">
                        {lesson_title}
                        <span class="chip chip-private">"lesson removed"</span>
                        <span class="notebook-item-date">{count_label}</span>
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

/// Gate for the note write routes: PB enabled, slugs valid, signed in,
/// course + lesson exist, viewer may see the course. Auth failures redirect
/// to /account (the drawer only renders signed in, so this is a stale
/// session); everything else is a 404 page.
async fn note_viewer(
    state: &AppState,
    headers: &HeaderMap,
    course_slug: &str,
    lesson_slug: &str,
) -> Result<Viewer, Box<Response>> {
    if state.pb_config.is_none() || !slug_ok(course_slug) || !slug_ok(lesson_slug) {
        return Err(Box::new(crate::web::pages::not_found_page()));
    }
    let viewer = viewer_for(state, headers).await.map_err(|_| {
        Box::new(
            Redirect::to(&format!(
                "/account?next=/course/{}/{}",
                crate::web::pages::urlencode(course_slug),
                crate::web::pages::urlencode(lesson_slug)
            ))
            .into_response(),
        )
    })?;
    let store = state.store.read().await.clone();
    let Some(course) = store.course(course_slug) else {
        return Err(Box::new(crate::web::pages::not_found_page()));
    };
    if course.lesson(lesson_slug).is_none() {
        return Err(Box::new(crate::web::pages::not_found_page()));
    }
    if !note_here_ok(state, &viewer, course_slug).await {
        return Err(Box::new(crate::web::pages::not_found_page()));
    }
    Ok(viewer)
}

/// Trimmed body + validated anchor fields for one note. Callers borrow these
/// into a `NoteDraft` (empty body allowed; callers decide what empty means).
fn note_parts(form: &NoteForm, lesson: &Lesson) -> (String, String, String) {
    let body = form.body.trim().to_string();
    let (anchor, here_title) = sanitize_anchor(lesson, form.anchor.trim(), form.here_title.trim());
    (body, anchor, here_title)
}

/// `POST /notes/{course}/{lesson}` — append a new note; an empty body is a
/// no-op. Always redirects back to the lesson page.
pub async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_slug, lesson_slug)): Path<(String, String)>,
    axum::Form(form): axum::Form<NoteForm>,
) -> Response {
    let back = |flag: &str| back_to_lesson(&course_slug, &lesson_slug, flag);
    let viewer = match note_viewer(&state, &headers, &course_slug, &lesson_slug).await {
        Ok(v) => v,
        Err(resp) => return *resp,
    };
    let store = state.store.read().await.clone();
    let Some(lesson) = store
        .course(&course_slug)
        .and_then(|c| c.lesson(&lesson_slug))
    else {
        return crate::web::pages::not_found_page();
    };
    let sess = viewer.session.as_ref().unwrap();
    let pb = state.pb.as_ref().unwrap();
    let (body, anchor, here_title) = note_parts(&form, lesson);
    if body.is_empty() {
        return back("");
    }
    if body.len() > 65536 {
        return back("error");
    }
    let draft = crate::pb::NoteDraft {
        course: &course_slug,
        lesson: &lesson_slug,
        anchor: &anchor,
        here_title: &here_title,
        body: &body,
    };
    match pb
        .create_notebook_entry(&sess.token, &sess.user_id, &draft)
        .await
    {
        Ok(()) => back("saved"),
        Err(e) => {
            tracing::warn!("note create failed for {course_slug}/{lesson_slug}: {e}");
            back("error")
        }
    }
}

/// `POST /notes/{course}/{lesson}/{entry}` — replace one saved note; an
/// empty body deletes it. Redirects back to the lesson page.
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_slug, lesson_slug, entry_id)): Path<(String, String, String)>,
    axum::Form(form): axum::Form<NoteForm>,
) -> Response {
    let back = |flag: &str| back_to_lesson(&course_slug, &lesson_slug, flag);
    if !entry_id_ok(&entry_id) {
        return back("error");
    }
    let viewer = match note_viewer(&state, &headers, &course_slug, &lesson_slug).await {
        Ok(v) => v,
        Err(_) => return back("error"),
    };
    let sess = viewer.session.as_ref().unwrap();
    let pb = state.pb.as_ref().unwrap();
    if form.body.trim().is_empty() {
        return match pb.delete_notebook_entry(&sess.token, &entry_id).await {
            Ok(()) => back("removed"),
            Err(e) => {
                tracing::warn!("note delete failed for {entry_id}: {e}");
                back("error")
            }
        };
    }
    let store = state.store.read().await.clone();
    let Some(lesson) = store
        .course(&course_slug)
        .and_then(|c| c.lesson(&lesson_slug))
    else {
        return back("error");
    };
    let (body, anchor, here_title) = note_parts(&form, lesson);
    if body.len() > 65536 {
        return back("error");
    }
    let draft = crate::pb::NoteDraft {
        course: &course_slug,
        lesson: &lesson_slug,
        anchor: &anchor,
        here_title: &here_title,
        body: &body,
    };
    match pb
        .update_notebook_entry(&sess.token, &entry_id, &draft)
        .await
    {
        Ok(()) => back("saved"),
        Err(e) => {
            tracing::warn!("note update failed for {course_slug}/{lesson_slug}/{entry_id}: {e}");
            back("error")
        }
    }
}

/// `POST /notes/{course}/{lesson}/{entry}/delete` — remove one saved note.
pub async fn delete_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_slug, lesson_slug, entry_id)): Path<(String, String, String)>,
) -> Response {
    let back = |flag: &str| back_to_lesson(&course_slug, &lesson_slug, flag);
    if !entry_id_ok(&entry_id) {
        return back("error");
    }
    let viewer = match note_viewer(&state, &headers, &course_slug, &lesson_slug).await {
        Ok(v) => v,
        Err(_) => return back("error"),
    };
    let sess = viewer.session.as_ref().unwrap();
    let pb = state.pb.as_ref().unwrap();
    match pb.delete_notebook_entry(&sess.token, &entry_id).await {
        Ok(()) => back("removed"),
        Err(e) => {
            tracing::warn!("note delete failed for {course_slug}/{lesson_slug}/{entry_id}: {e}");
            back("error")
        }
    }
}

fn back_to_lesson(course_slug: &str, lesson_slug: &str, flag: &str) -> Response {
    let base = format!(
        "/course/{}/{}",
        crate::web::pages::urlencode(course_slug),
        crate::web::pages::urlencode(lesson_slug)
    );
    let url = if flag.is_empty() {
        base
    } else {
        format!("{base}?notes={flag}")
    };
    Redirect::to(&url).into_response()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_sort_by_created_then_id() {
        let mk = |id: &str, created: &str| crate::pb::NotebookEntry {
            id: id.into(),
            created: created.into(),
            ..Default::default()
        };
        let entries = vec![
            mk("b", "2026-11-05 05:00:00.000Z"),
            mk("a", "2026-11-05 05:00:00.000Z"),
            mk("c", "2026-11-04 05:00:00.000Z"),
        ];
        let ids: Vec<&str> = sorted_entries(&entries)
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["c", "a", "b"]);
    }

    #[test]
    fn note_date_keeps_the_day() {
        assert_eq!(note_date("2026-11-05 05:00:00.000Z"), "2026-11-05");
        assert_eq!(note_date(""), "");
    }

    #[test]
    fn entry_ids_must_be_plain_alphanumerics() {
        assert!(entry_id_ok("abc123XYZ456de"));
        assert!(!entry_id_ok(""));
        assert!(!entry_id_ok("../../etc"));
        assert!(!entry_id_ok("with space"));
        assert!(!entry_id_ok(&"x".repeat(64)));
    }
}
