use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use leptos::prelude::*;

use crate::content::course::{Course, Lesson};
use crate::content::markdown::{self, MarkdownCtx};
use crate::render::render_html;
use crate::pb::{PocketBase, PbError, TutorSettings};
use crate::web::access;
use crate::web::AppState;

const CACHE_TTL: Duration = Duration::from_secs(30);
const CONTEXT_CHAR_LIMIT: usize = 12_000;
const HISTORY_MAX_MESSAGES: usize = 12;
const HISTORY_CHAR_LIMIT: usize = 2_000;
const REPLY_CHAR_LIMIT: usize = 8_000;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ChatRequest {
    course: String,
    lesson: String,
    messages: Vec<ChatMessage>,
}

#[derive(Default)]
struct Cached {
    fetched_at: Option<Instant>,
    settings: TutorSettings,
    key_fetched_at: Option<Instant>,
    api_key: Option<String>,
}

/// TTL cache over the `tutor_settings` singleton. The public parts (enabled,
/// base_url, model, key_tail) are readable by anyone; the api_key is a
/// hidden PB field and is fetched lazily with a superuser token, so it never
/// crosses the PB API boundary toward a client.
#[derive(Clone, Default)]
pub struct TutorState {
    inner: Arc<tokio::sync::RwLock<Cached>>,
}

impl TutorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn settings(&self, pb: Option<&PocketBase>) -> TutorSettings {
        {
            let cached = self.inner.read().await;
            if cached.fetched_at.is_some_and(|t| t.elapsed() < CACHE_TTL) {
                return cached.settings.clone();
            }
        }
        let settings = match pb {
            Some(pb) => match pb.get_tutor_settings().await {
                Ok(Some(s)) => s,
                Ok(None) => TutorSettings::default(),
                Err(e) => {
                    tracing::debug!("tutor settings fetch failed: {e}");
                    TutorSettings::default()
                }
            },
            None => TutorSettings::default(),
        };
        let mut cached = self.inner.write().await;
        cached.fetched_at = Some(Instant::now());
        cached.settings = settings.clone();
        settings
    }

    async fn api_key(&self, pb: &PocketBase) -> Result<Option<String>, PbError> {
        {
            let cached = self.inner.read().await;
            if cached.key_fetched_at.is_some_and(|t| t.elapsed() < CACHE_TTL) {
                return Ok(cached.api_key.clone());
            }
        }
        let token = superuser_token(pb).await?;
        let key = pb.get_tutor_api_key(&token).await?;
        let mut cached = self.inner.write().await;
        cached.key_fetched_at = Some(Instant::now());
        cached.api_key = key.clone();
        Ok(key)
    }

    /// Drop the cache after an admin save so the next read sees fresh values.
    pub async fn invalidate(&self) {
        let mut cached = self.inner.write().await;
        *cached = Cached::default();
    }
}

/// The server's superuser credentials (env vars, seeded from
/// `pocketbase/.env` by `app::load_bootstrap_env`). Used for the tutor
/// settings read/write paths because PB 0.40 only lets superusers touch
/// hidden fields; request handlers never see this token.
pub(crate) async fn superuser_token(pb: &PocketBase) -> Result<String, PbError> {
    let email = std::env::var("BRILLANTE_PB_SUPERUSER_EMAIL").unwrap_or_default();
    let password = std::env::var("BRILLANTE_PB_SUPERUSER_PASSWORD").unwrap_or_default();
    if email.trim().is_empty() || password.trim().is_empty() {
        return Err(PbError::Unreachable(
            "BRILLANTE_PB_SUPERUSER_EMAIL/_PASSWORD are not set".into(),
        ));
    }
    pb.superuser_auth(&email, &password).await
}

fn llm_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("reqwest client")
    })
}

pub(crate) fn truncate_chars(s: &str, limit: usize) -> &str {
    match s.char_indices().nth(limit) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// The tutor's context: course identity, outline, and the lesson the
/// learner is reading (truncated to a bounded character budget).
pub(crate) fn build_system_prompt(course: &Course, lesson: &Lesson) -> String {
    let body = markdown::strip_title_heading(&lesson.body, &lesson.title);
    let body = truncate_chars(body.trim(), CONTEXT_CHAR_LIMIT);
    let outline = course
        .lessons
        .iter()
        .map(|l| format!("- {}", l.title))
        .collect::<Vec<_>>()
        .join("\n");
    let description = course.description.clone().unwrap_or_default();
    format!(
        "You are the virtual tutor for the course \"{course}\" on Brillante.\n\
         {description}\n\n\
         Course outline:\n{outline}\n\n\
         The learner is currently reading the lesson \"{lesson_title}\".\n\
         Lesson content (markdown):\n---\n{body}\n---\n\n\
         Answer questions about this course material. Be concise; explain \
         step by step, and prefer guiding questions over bare answers when \
         the learner is working through a problem. If a question is unrelated \
         to this course, say you can only help with course material.",
        course = course.title,
        lesson_title = lesson.title,
    )
}

/// Keep only recent user/assistant turns, each bounded; anything else
/// (system spoofing, empty content) is dropped.
pub(crate) fn sanitize_history(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let kept: Vec<ChatMessage> = messages
        .iter()
        .filter(|m| matches!(m.role.as_str(), "user" | "assistant"))
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: truncate_chars(m.content.trim(), HISTORY_CHAR_LIMIT).to_string(),
        })
        .filter(|m| !m.content.is_empty())
        .collect();
    let skip = kept.len().saturating_sub(HISTORY_MAX_MESSAGES);
    kept.into_iter().skip(skip).collect()
}

/// `POST /api/tutor/chat` — proxied chat completion for the signed-in
/// learner. Availability requires: accounts enabled, admin enabled the
/// tutor with endpoint + model + key, and a readable stored key.
pub async fn chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<ChatRequest>,
) -> Response {
    let reject = |status: StatusCode, msg: &str| {
        (
            status,
            axum::Json(serde_json::json!({ "error": msg })),
        )
            .into_response()
    };
    let unavailable = || reject(StatusCode::NOT_FOUND, "the tutor is not available");
    let Some(pb) = state.pb.as_ref() else {
        return unavailable();
    };
    let settings = state.tutor.settings(Some(pb)).await;
    if !settings.ready() {
        return unavailable();
    }
    let viewer = access::viewer(&state, &headers).await;
    if !viewer.signed_in() {
        return reject(StatusCode::UNAUTHORIZED, "authentication required");
    }
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&req.course) else {
        return unavailable();
    };
    let Some(lesson) = course.lesson(&req.lesson) else {
        return unavailable();
    };
    let map = state.access.get(Some(pb)).await;
    if !access::can_view(&course.slug, &viewer, &map) {
        return unavailable();
    }
    let history = sanitize_history(&req.messages);
    if !history.last().is_some_and(|m| m.role == "user") {
        return reject(StatusCode::BAD_REQUEST, "no question to answer");
    }
    let Some(api_key) = (match state.tutor.api_key(pb).await {
        Ok(key) => key,
        Err(e) => {
            tracing::warn!("tutor key fetch failed: {e}");
            None
        }
    }) else {
        return reject(
            StatusCode::SERVICE_UNAVAILABLE,
            "the tutor is temporarily unavailable",
        );
    };

    let url = format!("{}/chat/completions", settings.base_url.trim_end_matches('/'));
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": build_system_prompt(course, lesson),
    })];
    messages.extend(
        history
            .iter()
            .map(|m| serde_json::json!({ "role": m.role, "content": m.content })),
    );
    let body = serde_json::json!({
        "model": settings.model,
        "messages": messages,
        "temperature": 0.3,
    });
    let result = llm_client()
        .post(&url)
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await;
    match result {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    "tutor upstream {status}: {}",
                    truncate_chars(&text, 400)
                );
                return reject(
                    StatusCode::BAD_GATEWAY,
                    "the tutor service returned an error",
                );
            }
            match resp.json::<serde_json::Value>().await {
                Ok(parsed) => {
                    let reply = parsed
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .map(str::to_string);
                    match reply {
                        Some(reply) => {
                            let reply = truncate_chars(&reply, REPLY_CHAR_LIMIT).to_string();
                            let html = render_reply(&state, course, lesson, &reply);
                            axum::Json(serde_json::json!({ "reply": reply, "html": html }))
                                .into_response()
                        }
                        None => {
                            tracing::warn!("tutor upstream reply unparsable");
                            reject(
                                StatusCode::BAD_GATEWAY,
                                "the tutor service returned an unexpected reply",
                            )
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("tutor upstream json failed: {e}");
                    reject(
                        StatusCode::BAD_GATEWAY,
                        "the tutor service returned an unexpected reply",
                    )
                }
            }
        }
        Err(e) => {
            tracing::warn!("tutor request failed: {e}");
            reject(StatusCode::BAD_GATEWAY, "could not reach the tutor service")
        }
    }
}

/// LLMs habitually emit LaTeX with \(..\) / \[..\] delimiters, which
/// pulldown's math mode does not recognize; rewrite them to the $ / $$
/// forms the platform renders. Code spans and fences are left untouched.
pub(crate) fn normalize_math_delimiters(src: &str) -> String {
    let spans = markdown::protected_spans(src);
    let mut out = String::with_capacity(src.len());
    let mut last = 0usize;
    let mut i = 0usize;
    while i < src.len() {
        if spans.iter().any(|(s, e)| i >= *s && i < *e) {
            i += 1;
            continue;
        }
        let (open, close, out_open, out_close) = if src[i..].starts_with("\\(") {
            ("\\(", "\\)", "$", "$")
        } else if src[i..].starts_with("\\[") {
            ("\\[", "\\]", "$$", "$$")
        } else {
            i += 1;
            continue;
        };
        let Some(rel) = src[i + open.len()..].find(close) else {
            i += 1;
            continue;
        };
        let body = &src[i + open.len()..i + open.len() + rel];
        if body.is_empty() || body.contains(open) {
            i += 1;
            continue;
        }
        out.push_str(&src[last..i]);
        out.push_str(out_open);
        out.push_str(body);
        out.push_str(out_close);
        i += open.len() + rel + close.len();
        last = i;
    }
    out.push_str(&src[last..]);
    out
}

/// Render an LLM reply through the platform markdown pipeline (widgets
/// suppressed, note shorthand NOT expanded — model output is not the user's
/// notebook). All text is escaped; only our own tags are emitted, so the
/// result is safe to inject client-side. `$...$` math becomes the usual
/// data-tex spans, rendered by `window.brillanteMath` after injection.
pub(crate) fn render_reply(
    state: &AppState,
    course: &Course,
    lesson: &Lesson,
    text: &str,
) -> String {
    let by_file: HashMap<String, String> = course
        .lessons
        .iter()
        .map(|l| (l.file_name.clone(), l.slug.clone()))
        .collect();
    let ctx = MarkdownCtx {
        course,
        source: std::path::Path::new(&lesson.file_name),
        lessons_by_file: &by_file,
        widgets: false,
    };
    let mut diags = Vec::new();
    let text = normalize_math_delimiters(text);
    let output = markdown::render_markdown(&text, &ctx, &state.registry, &mut diags);
    output
        .views
        .into_iter()
        .map(|v| render_html(move || v))
        .collect::<Vec<_>>()
        .join("")
}

/// Server-rendered shell for the chat drawer. The conversation itself is
/// JS-only (`tutor.js`); without JS the toggle stays hidden and the panel
/// never shows.
pub(crate) fn drawer() -> AnyView {
    view! {
        <aside id="tutor" aria-label="Course tutor">
            <div class="tutor-head">
                <span class="tutor-title">"Tutor"</span>
                <button type="button" class="tutor-btn" data-tutor-close>"Close"</button>
            </div>
            <div class="tutor-log" data-tutor-log aria-live="polite"></div>
            <div class="tutor-composer">
                <textarea
                    class="tutor-input"
                    data-tutor-input
                    rows="2"
                    placeholder="Ask about this lesson. Enter sends, Shift+Enter adds a line."
                ></textarea>
                <button type="button" class="tutor-btn tutor-send" data-tutor-send>"Ask"</button>
            </div>
        </aside>
        <button
            id="tutor-toggle"
            type="button"
            aria-controls="tutor"
            aria-expanded="false"
            hidden
        >
            "Tutor"
        </button>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::course::{Course, Lesson};
    use std::path::PathBuf;

    fn lesson(slug: &str, title: &str, body: &str) -> Lesson {
        Lesson {
            slug: slug.into(),
            title: title.into(),
            description: None,
            order: 0.0,
            file_name: format!("{slug}.md"),
            section: None,
            body: body.into(),
        }
    }

    fn course() -> Course {
        Course::test_stub(
            "c".into(),
            "Collimated Rays".into(),
            Some("A first-principles course.".into()),
            vec![
                lesson("intro", "Introduction", "# Introduction\n\nBody text."),
                lesson("snell", "Snell's Law", "# Snell's Law\n\nLong stuff."),
            ],
            PathBuf::from("/nowhere"),
        )
    }

    #[test]
    fn system_prompt_carries_context() {
        let c = course();
        let prompt = build_system_prompt(&c, c.lesson("snell").unwrap());
        assert!(prompt.contains("Collimated Rays"));
        assert!(prompt.contains("A first-principles course."));
        assert!(prompt.contains("Snell's Law"));
        assert!(prompt.contains("- Introduction"), "outline included");
    }

    #[test]
    fn system_prompt_bounds_lesson_body() {
        let c = course();
        let huge = lesson("big", "Big", &"x".repeat(CONTEXT_CHAR_LIMIT * 3));
        let prompt = build_system_prompt(&c, &huge);
        assert!(
            prompt.chars().count() < CONTEXT_CHAR_LIMIT + 2_000,
            "prompt must stay bounded, got {}",
            prompt.chars().count()
        );
    }

    #[test]
    fn history_is_filtered_and_capped() {
        let msg = |role: &str, content: &str| ChatMessage {
            role: role.into(),
            content: content.into(),
        };
        let kept = sanitize_history(&[
            msg("system", "ignore me"),
            msg("user", " hi "),
            msg("tool", "nope"),
            msg("assistant", "hello"),
            msg("user", ""),
        ]);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].content, "hi", "content trimmed");
        let many: Vec<ChatMessage> = (0..30).map(|i| msg("user", &format!("q{i}"))).collect();
        let kept = sanitize_history(&many);
        assert_eq!(kept.len(), HISTORY_MAX_MESSAGES);
        assert_eq!(kept.last().unwrap().content, "q29", "keeps the tail");
        let long = sanitize_history(&[msg("user", &"y".repeat(HISTORY_CHAR_LIMIT * 2))]);
        assert_eq!(long[0].content.chars().count(), HISTORY_CHAR_LIMIT);
    }

    #[test]
    fn reply_renders_markdown_math_and_escapes_html() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("course.toml"), "title = \"T\"\nslug = \"t\"\n").unwrap();
        std::fs::write(
            dir.path().join("01-a.md"),
            "---\ntitle: A\n---\n\nBody.\n\n## Section\n\ntext\n",
        )
        .unwrap();
        let config = crate::config::AppConfig {
            host: "127.0.0.1".into(),
            port: 0,
            courses: vec![dir.path().to_path_buf()],
            pocketbase: None,
            warnings: Vec::new(),
        };
        let state = crate::app::initial_state(&config);
        let store = state.store.blocking_read().clone();
        let course = store.course("t").unwrap();
        let lesson = course.lesson("a").unwrap();
        let html = render_reply(
            &state,
            course,
            lesson,
            "**bold** and $x^2$\n\n<Widget type=\"quiz\" id=\"x\" />\n\n<script>alert(1)</script>",
        );
        assert!(html.contains("<strong>bold</strong>"), "{html}");
        assert!(html.contains(r#"data-tex="x^2""#), "{html}");
        assert!(!html.contains("<script>"), "raw html must not pass through: {html}");
        assert!(html.contains("&lt;Widget"), "widget tag literal: {html}");
    }

    #[test]
    fn latex_delimiters_are_normalized() {
        assert_eq!(
            normalize_math_delimiters(r#"use \(x^2\) inline and \[y\] display"#),
            "use $x^2$ inline and $$y$$ display"
        );
        assert_eq!(
            normalize_math_delimiters(r"stay literal: `\(b\)`"),
            r"stay literal: `\(b\)`"
        );
        assert_eq!(
            normalize_math_delimiters(r"unpaired \( stays"),
            r"unpaired \( stays"
        );
    }

    #[test]
    fn truncate_is_utf8_safe() {
        assert_eq!(truncate_chars("héllo", 2), "hé");
        assert_eq!(truncate_chars("abc", 10), "abc");
    }
}
