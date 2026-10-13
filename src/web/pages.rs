use std::collections::{HashMap, HashSet};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use leptos::prelude::*;

use crate::content::course::Course;
use crate::content::markdown::{self, MarkdownCtx};
use crate::diag::{log_diagnostics, Diagnostic};
use crate::render::render_document;
use crate::web::access::{self, Viewer};
use crate::web::search;
use crate::web::session::{self, Session};
use crate::web::AppState;

fn lessons_by_file(course: &Course) -> HashMap<String, String> {
    course
        .lessons
        .iter()
        .map(|l| (l.file_name.clone(), l.slug.clone()))
        .collect()
}

/// Completed (course, lesson) pairs for the session user; errors degrade to
/// an empty set (render as no progress rather than erroring the page).
pub(crate) async fn server_progress(
    state: &AppState,
    headers: &HeaderMap,
) -> HashSet<(String, String)> {
    let Some(pb) = &state.pb else {
        return HashSet::new();
    };
    let Some(raw) = session::cookie_from_headers(headers) else {
        return HashSet::new();
    };
    let Some((user_id, exp)) = session::decode_claims(&raw) else {
        return HashSet::new();
    };
    if exp <= session::now_unix() {
        return HashSet::new();
    }
    let sess = Session {
        token: raw,
        user_id,
        exp,
    };
    match pb.list_progress(&sess.token).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| (r.course, r.lesson))
            .collect::<HashSet<_>>(),
        Err(e) => {
            tracing::debug!("progress fetch for page render failed: {e}");
            HashSet::new()
        }
    }
}

/// Gate for private-course pages: anonymous visitors are sent to sign-in
/// (they learn only that signing in may help); signed-in users without
/// access get the plain 404 page so the course's existence stays hidden.
fn access_denied(course_slug: &str, viewer: &Viewer) -> Response {
    if viewer.signed_in() {
        not_found_page()
    } else {
        Redirect::to(&format!(
            "/account?next=/course/{}",
            urlencode(course_slug)
        ))
        .into_response()
    }
}

pub(crate) fn urlencode(raw: &str) -> String {
    let mut out = String::new();
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

const ACCENT_PALETTE: [&str; 6] = [
    "#6366f1", "#0ea5e9", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6",
];

fn default_accent(slug: &str) -> &'static str {
    let mut h: u64 = 5381;
    for b in slug.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    ACCENT_PALETTE[(h % ACCENT_PALETTE.len() as u64) as usize]
}

fn ack_banner(course: &Course) -> Option<AnyView> {
    if course.ack_widget_ids.is_empty() {
        return None;
    }
    let ids = serde_json::to_string(&course.ack_widget_ids).ok()?;
    let message = "Before you start, please read this course's references & licensing policy and acknowledge it.";
    let link: AnyView = match &course.ack_lesson_slug {
        Some(lesson_slug) => {
            let href = format!("/course/{}/{}", course.slug, lesson_slug);
            view! { <a class="button button-outline ack-banner-link" href={href}>"Read & acknowledge"</a> }
                .into_any()
        }
        None => view! { <span class="ack-banner-link" hidden></span> }.into_any(),
    };
    Some(
        view! {
            <div
                class="ack-banner"
                hidden
                data-ack-ids={ids}
                data-ack-message={message.to_string()}
            >
                <span class="ack-banner-text">{message}</span>
                {link}
            </div>
        }
        .into_any(),
    )
}

fn scratchpad_view() -> AnyView {
    view! {
        <aside id="scratchpad" aria-label="Python scratch pad">
            <div class="scratchpad-head">
                <span class="scratchpad-title">"Python scratch pad"</span>
                <span class="scratchpad-status" data-scratch-status data-state="loading">
                    "idle"
                </span>
                <button type="button" class="scratchpad-btn" data-scratch-reset disabled>
                    "Reset"
                </button>
                <button type="button" class="scratchpad-btn" data-scratch-close>"Close"</button>
            </div>
            <div class="scratchpad-log" data-scratch-log>
                <div class="scratchpad-row scratchpad-info">
                    "Python runs locally in your browser (WebAssembly). Nothing you type is sent to the server; the runtime downloads on first use."
                </div>
            </div>
            <form class="scratchpad-entry" data-scratch-form>
                <span class="scratchpad-prompt" data-scratch-prompt>"In [1]:"</span>
                <textarea
                    class="scratchpad-input"
                    data-scratch-input
                    rows={1}
                    placeholder="type Python here"
                    disabled
                ></textarea>
                <button type="submit" class="scratchpad-btn" data-scratch-run disabled>
                    "Run"
                </button>
            </form>
        </aside>
        <button
            id="scratchpad-toggle"
            type="button"
            aria-controls="scratchpad"
            aria-expanded="false"
            hidden
        >
            "Scratch pad"
        </button>
    }
    .into_any()
}

#[component]
pub fn Shell(
    title: String,
    #[prop(optional)] accent: Option<String>,
    #[prop(optional)] data_course: Option<String>,
    #[prop(optional)] data_lesson: Option<String>,
    #[prop(optional)] extra_css: Vec<String>,
    #[prop(optional)] extra_js: Vec<String>,
    #[prop(optional)] auth_enabled: bool,
    #[prop(optional)] signed_in: bool,
    #[prop(optional)] admin: bool,
    content: Vec<AnyView>,
) -> impl IntoView {
    let style = accent.map(|a| format!("--accent: {a}"));
    let mut extra_css = extra_css
        .into_iter()
        .map(|href| view! { <link rel="stylesheet" href={href} /> }.into_any())
        .collect::<Vec<_>>();
    let mut extra_js = extra_js
        .into_iter()
        .map(|src| view! { <script src={src} defer></script> }.into_any())
        .collect::<Vec<_>>();
    let scratchpad: AnyView = if data_course.is_some() {
        extra_css.push(
            view! { <link rel="stylesheet" href="/assets/scratchpad.css" /> }.into_any(),
        );
        extra_js.push(view! { <script src="/assets/scratchpad.js" defer></script> }.into_any());
        scratchpad_view()
    } else {
        view! { <span hidden></span> }.into_any()
    };

    let account_nav: AnyView = if auth_enabled {
        view! { <a href="/account" id="account-link">"Account"</a> }.into_any()
    } else {
        view! { <span hidden></span> }.into_any()
    };
    let admin_nav: AnyView = if auth_enabled && admin {
        view! { <a href="/admin/courses" id="admin-link">"Admin"</a> }.into_any()
    } else {
        view! { <span hidden></span> }.into_any()
    };
    let header: AnyView = view! {
        <header class="topbar">
            <div class="topbar-inner">
                <a class="brand" href="/">"Brillante"</a>
                <nav class="topnav">
                    <a href="/">"Home"</a>
                    <a href="/browse">"Browse"</a>
                    {account_nav}
                    {admin_nav}
                </nav>
            </div>
        </header>
    }
    .into_any();
    let footer: AnyView = view! {
        <footer class="site-footer">
            <p>"Powered by Brillante — markdown courses with interactive widgets."</p>
        </footer>
    }
    .into_any();
    let site_js: AnyView = view! { <script src="/assets/site.js" defer></script> }.into_any();
    let inner: Vec<AnyView> = vec![
        header,
        view! { <main>{content}</main> }.into_any(),
        footer,
        scratchpad,
        site_js,
        view! { <div>{extra_js}</div> }.into_any(),
    ];

    let auth_attr = signed_in.then(|| "server".to_string());
    let body = match (data_course, data_lesson) {
        (Some(c), Some(l)) => view! {
            <body style={style} data-course={c} data-lesson={l} data-auth={auth_attr}>{inner}</body>
        }
        .into_any(),
        (Some(c), None) => view! {
            <body style={style} data-course={c} data-auth={auth_attr}>{inner}</body>
        }
        .into_any(),
        _ => view! { <body style={style} data-auth={auth_attr}>{inner}</body> }.into_any(),
    };

    view! {
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>{title}</title>
                <link rel="icon" href="/assets/favicon.svg" type="image/svg+xml" />
                <link rel="stylesheet" href="/assets/site.css" />
                {extra_css}
            </head>
            {body}
        </html>
    }
    .into_any()
}

pub(crate) fn not_found_page() -> Response {
    let content = vec![
        view! { <h1>"404"</h1> }.into_any(),
        view! { <p>"That page does not exist. "<a href="/browse">"Browse all courses"</a>"."</p> }
            .into_any(),
    ];
    (
        StatusCode::NOT_FOUND,
        Html(render_document(move || {
            view! {
                <Shell title="Not found — Brillante".to_string() content />
            }
        }))
            .into_response(),
    )
        .into_response()
}

pub async fn not_found() -> Response {
    not_found_page()
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct CatalogQuery {
    error: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct LessonQuery {
    notes: Option<String>,
}

pub async fn home(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CatalogQuery>,
) -> Response {
    let store = state.store.read().await.clone();
    let auth_enabled = state.pb_config.is_some();
    let map = state.access.get(state.pb.as_ref()).await;
    let viewer = access::viewer(&state, &headers).await;
    let signed_in = viewer.signed_in();
    let admin = auth_enabled && viewer.is_admin();
    let show_subscribe = auth_enabled && signed_in;

    let error_view: AnyView = if params.error.as_deref() == Some("subscribe") {
        view! { <p class="auth-error">"Could not update your subscription. Please try again."</p> }
            .into_any()
    } else {
        view! { <span hidden></span> }.into_any()
    };

    let content: Vec<AnyView> = if show_subscribe {
        let done = server_progress(&state, &headers).await;
        let my_cards: Vec<AnyView> = store
            .courses
            .iter()
            .filter(|c| access::can_view(&c.slug, &viewer, &map))
            .filter(|c| viewer.subscriptions.contains(&c.slug))
            .map(|c| {
                course_card(
                    c,
                    is_private(&map, &c.slug),
                    true,
                    true,
                    "/",
                    Some(&card_progress(c, &done)),
                )
            })
            .collect();
        let body: AnyView = if my_cards.is_empty() {
            view! {
                <p class="my-courses-empty">
                    "Nothing subscribed yet. "<a href="/browse">"Browse courses"</a>" to add one."
                </p>
            }
            .into_any()
        } else {
            view! { <div class="course-grid">{my_cards}</div> }.into_any()
        };
        let my_courses = view! {
            <section class="my-courses" id="my-courses">
                <h2 class="section-title">"My courses"</h2>
                {body}
            </section>
        }
        .into_any();
        vec![
            view! {
                <section class="hero">
                    <h1>"Welcome back."</h1>
                    <p class="hero-sub">"Pick up where you left off."</p>
                </section>
            }
            .into_any(),
            error_view,
            my_courses,
            view! {
                <p class="browse-prompt">
                    "Looking for something new? "<a href="/browse">"Browse all courses"</a>"."
                </p>
            }
            .into_any(),
        ]
    } else {
        let catalog_cards: Vec<AnyView> = store
            .courses
            .iter()
            .filter(|c| access::can_view(&c.slug, &viewer, &map))
            .map(|c| {
                course_card(
                    c,
                    is_private(&map, &c.slug),
                    false,
                    false,
                    "/",
                    None,
                )
            })
            .collect();
        vec![
            view! {
                <section class="hero">
                    <h1>"Learn something brilliant."</h1>
                    <p class="hero-sub">"Interactive courses, served straight from markdown."</p>
                </section>
            }
            .into_any(),
            error_view,
            view! { <span hidden></span> }.into_any(),
            view! { <section class="course-grid">{catalog_cards}</section> }.into_any(),
        ]
    };
    Html(render_document(move || {
        view! {
            <Shell
                title="Brillante".to_string()
                auth_enabled
                signed_in
                admin
                content
            />
        }
    }))
    .into_response()
}

fn is_private(map: &access::AccessMap, slug: &str) -> bool {
    map.get(slug)
        .is_some_and(|a| a.visibility == access::Visibility::Private)
}

/// Where a progress card's action button points: the first lesson not yet
/// completed ("Continue"), else the last lesson ("Review"), else the course
/// outline when there are no lessons ("Open").
pub(crate) fn continue_target(
    course: &Course,
    done: &HashSet<(String, String)>,
) -> (&'static str, String) {
    let href = |slug: &str| format!("/course/{}/{}", course.slug, slug);
    if let Some(lesson) = course
        .lessons
        .iter()
        .find(|l| !done.contains(&(course.slug.clone(), l.slug.clone())))
    {
        return ("Continue", href(&lesson.slug));
    }
    if let Some(lesson) = course.lessons.last() {
        return ("Review", href(&lesson.slug));
    }
    ("Open", format!("/course/{}", course.slug))
}

/// Progress rendering for My Courses cards.
struct CardProgress {
    completed: usize,
    total: usize,
    action_label: &'static str,
    action_href: String,
}

fn card_progress(course: &Course, done: &HashSet<(String, String)>) -> CardProgress {
    let completed = course
        .lessons
        .iter()
        .filter(|l| done.contains(&(course.slug.clone(), l.slug.clone())))
        .count();
    let (action_label, action_href) = continue_target(course, done);
    CardProgress {
        completed,
        total: course.lessons.len(),
        action_label,
        action_href,
    }
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct BrowseQuery {
    q: Option<String>,
}

pub async fn browse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<BrowseQuery>,
) -> Response {
    let store = state.store.read().await.clone();
    let auth_enabled = state.pb_config.is_some();
    let map = state.access.get(state.pb.as_ref()).await;
    let viewer = access::viewer(&state, &headers).await;
    let signed_in = viewer.signed_in();
    let admin = auth_enabled && viewer.is_admin();
    let show_subscribe = auth_enabled && signed_in;

    let query = params.q.unwrap_or_default().trim().to_string();
    let searching = !query.is_empty();
    let next = if searching {
        format!("/browse?q={}", urlencode(&query))
    } else {
        "/browse".to_string()
    };
    let visible = |slug: &str| access::can_view(slug, &viewer, &map);

    let private_of = |slug: &str| {
        map.get(slug)
            .is_some_and(|a| a.visibility == access::Visibility::Private)
    };

    let results: AnyView = if searching {
        let hits = search::search(&store.courses, &query, visible);
        let cards: Vec<AnyView> = hits
            .iter()
            .map(|hit| {
                course_card(
                    hit.course,
                    private_of(&hit.course.slug),
                    viewer.subscriptions.contains(&hit.course.slug),
                    show_subscribe,
                    &next,
                    None,
                )
            })
            .collect();
        let lesson_links: Vec<AnyView> = hits
            .iter()
            .flat_map(|hit| {
                hit.matched_lessons.iter().map(move |l| {
                    let href = format!("/course/{}/{}", hit.course.slug, l.slug);
                    let lesson_title = l.title.clone();
                    let course_title = hit.course.title.clone();
                    view! {
                        <li>
                            <a href={href}>{lesson_title}</a>
                            <span class="search-lesson-course">{course_title}</span>
                        </li>
                    }
                    .into_any()
                })
            })
            .collect();
        let lessons_block: AnyView = if lesson_links.is_empty() {
            view! { <span hidden></span> }.into_any()
        } else {
            view! {
                <section class="search-lessons">
                    <h2 class="section-title">"Lessons"</h2>
                    <ul>{lesson_links}</ul>
                </section>
            }
            .into_any()
        };
        let count = hits.len();
        let summary = format!("{count} course(s) match");
        let empty: AnyView = if cards.is_empty() {
            view! {
                <p class="browse-empty">
                    "No matches. "<a href="/browse">"Browse all courses"</a>"."
                </p>
            }
            .into_any()
        } else {
            view! { <span hidden></span> }.into_any()
        };
        view! {
            <div>
                <p class="search-summary">{summary}" — “"{query.clone()}"”"</p>
                {empty}
                <div class="course-grid">{cards}</div>
                {lessons_block}
            </div>
        }
        .into_any()
    } else {
        let cards: Vec<AnyView> = store
            .courses
            .iter()
            .filter(|c| visible(&c.slug))
            .map(|c| {
                course_card(
                    c,
                    private_of(&c.slug),
                    viewer.subscriptions.contains(&c.slug),
                    show_subscribe,
                    &next,
                    None,
                )
            })
            .collect();
        let empty: AnyView = if cards.is_empty() {
            view! {
                <p class="browse-empty">"No courses are available to you yet."</p>
            }
            .into_any()
        } else {
            view! { <span hidden></span> }.into_any()
        };
        view! {
            <div>
                {empty}
                <div class="course-grid">{cards}</div>
            </div>
        }
        .into_any()
    };

    let search_box: AnyView = view! {
        <form method="get" action="/browse" class="search-form" role="search">
            <input
                type="search"
                name="q"
                value={query.clone()}
                placeholder="Search courses and lessons"
                aria-label="Search courses"
            />
            <button type="submit" class="button button-outline">"Search"</button>
        </form>
    }
    .into_any();

    let content = vec![
        view! {
            <section class="browse-head">
                <h1>"Browse courses"</h1>
                <p class="hero-sub">"Every course available on this server."</p>
                {search_box}
            </section>
        }
        .into_any(),
        results,
    ];
    Html(render_document(move || {
        view! {
            <Shell
                title="Browse — Brillante".to_string()
                auth_enabled
                signed_in
                admin
                content
            />
        }
    }))
    .into_response()
}

fn subscribe_button(course_slug: &str, subscribed: bool, next: &str) -> AnyView {
    let (action, label, class) = if subscribed {
        ("unsubscribe", "Subscribed", "button button-small button-outline subscribed")
    } else {
        ("subscribe", "Subscribe", "button button-small button-outline")
    };
    view! {
        <form method="post" action="/my/subscriptions" class="subscribe-form">
            <input type="hidden" name="course" value={course_slug.to_string()} />
            <input type="hidden" name="action" value={action.to_string()} />
            <input type="hidden" name="next" value={next.to_string()} />
            <button type="submit" class={class}>{label}</button>
        </form>
    }
    .into_any()
}

fn course_card(
    c: &Course,
    private: bool,
    subscribed: bool,
    show_subscribe: bool,
    next: &str,
    progress: Option<&CardProgress>,
) -> AnyView {
    let title = c.title.clone();
    let desc = c.description.clone().unwrap_or_default();
    let accent = c.accent.clone().unwrap_or_else(|| default_accent(&c.slug).to_string());
    let style = format!("--accent: {accent}");
    let href = format!("/course/{}", c.slug);
    let count = if c.lessons.len() == 1 {
        "1 lesson".to_string()
    } else {
        format!("{} lessons", c.lessons.len())
    };
    let chip: AnyView = if private {
        view! { <span class="chip chip-private">"Private"</span> }.into_any()
    } else {
        view! { <span hidden></span> }.into_any()
    };
    let subscribe: AnyView = if show_subscribe {
        subscribe_button(&c.slug, subscribed, next)
    } else {
        view! { <span hidden></span> }.into_any()
    };
    let (bar, action): (AnyView, AnyView) = match progress {
        Some(p) => {
            let pct = p
                .completed
                .saturating_mul(100)
                .checked_div(p.total)
                .unwrap_or(0);
            let width = format!("width: {pct}%");
            let text = format!("{} of {} lessons done", p.completed, p.total);
            let href = p.action_href.clone();
            let label = p.action_label;
            (
                view! {
                    <span class="course-card-progress">
                        <span class="progress-track">
                            <span class="progress-fill" style={width}></span>
                        </span>
                        <span class="progress-text">{text}</span>
                    </span>
                }
                .into_any(),
                view! {
                    <a class="button button-small" href={href}>{label}</a>
                }
                .into_any(),
            )
        }
        None => (
            view! { <span hidden></span> }.into_any(),
            view! { <span hidden></span> }.into_any(),
        ),
    };
    view! {
        <div class="course-card" style={style}>
            <a class="course-card-link" href={href}>
                <h3>{title}</h3>
                <p class="course-card-desc">{desc}</p>
                <span class="course-card-meta">{count}</span>
                {bar}
            </a>
            <div class="course-card-foot">
                {chip}
                {subscribe}
                {action}
            </div>
        </div>
    }
    .into_any()
}

pub async fn course_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&slug) else {
        return not_found_page();
    };

    let auth_enabled = state.pb_config.is_some();
    let map = state.access.get(state.pb.as_ref()).await;
    let viewer = access::viewer(&state, &headers).await;
    if !access::can_view(&course.slug, &viewer, &map) {
        return access_denied(&course.slug, &viewer);
    }
    let signed_in = viewer.signed_in();
    let admin = auth_enabled && viewer.is_admin();
    let done = server_progress(&state, &headers).await;

    let accent = course
        .accent
        .clone()
        .unwrap_or_else(|| default_accent(&course.slug).to_string());
    let total = course.lessons.len();
    let mut num_of: HashMap<String, usize> = HashMap::new();
    for (i, lesson) in course.lessons.iter().enumerate() {
        num_of.insert(lesson.slug.clone(), i + 1);
    }

    let mut outline: Vec<AnyView> = Vec::new();
    for (sec, lessons) in course.outline_groups() {
        if lessons.is_empty() {
            continue;
        }
        let items: Vec<AnyView> = lessons
            .iter()
            .map(|lesson| {
                let num = num_of[&lesson.slug].to_string();
                let href = format!("/course/{}/{}", course.slug, lesson.slug);
                let title = lesson.title.clone();
                let desc = lesson.description.clone().unwrap_or_default();
                let li_class = if done.contains(&(course.slug.clone(), lesson.slug.clone())) {
                    "lesson-item done"
                } else {
                    "lesson-item"
                };
                view! {
                    <li class={li_class} data-lesson={lesson.slug.clone()}>
                        <a href={href}>
                            <span class="lesson-num">{num}</span>
                            <span class="lesson-item-text">
                                <span class="lesson-item-title">{title}</span>
                                <span class="lesson-item-desc">{desc}</span>
                            </span>
                            <span class="lesson-check" aria-hidden="true"></span>
                        </a>
                    </li>
                }
                .into_any()
            })
            .collect();
        match sec {
            Some(s) => {
                let items = items;
                let lede: AnyView = match &s.description {
                    Some(d) => view! { <p class="section-lede">{d.clone()}</p> }.into_any(),
                    None => view! { <p class="section-lede" hidden></p> }.into_any(),
                };
                outline.push(
                    view! {
                        <section class="course-section">
                            <h2 class="section-title">{s.title.clone()}</h2>
                            {lede}
                            <ol class="lesson-list">{items}</ol>
                        </section>
                    }
                    .into_any(),
                );
            }
            None => outline.push(view! { <ol class="lesson-list">{items}</ol> }.into_any()),
        }
    }

    if course.lessons.is_empty() {
        outline.push(
            view! {
                <ol class="lesson-list">
                    <li class="lesson-item"><p>"This course has no lessons yet."</p></li>
                </ol>
            }
            .into_any(),
        );
    }

    let description = course.description.clone().unwrap_or_default();
    let total_text = total.to_string();
    let completed = if signed_in {
        course
            .lessons
            .iter()
            .filter(|l| done.contains(&(course.slug.clone(), l.slug.clone())))
            .count()
    } else {
        0
    };
    let completed_text = completed.to_string();
    let start_href = course
        .lessons
        .first()
        .map(|l| format!("/course/{}/{}", course.slug, l.slug))
        .unwrap_or_else(|| format!("/course/{}", course.slug));
    let ack = ack_banner(course);
    let private = map
        .get(&course.slug)
        .is_some_and(|a| a.visibility == access::Visibility::Private);
    let privacy_chip: AnyView = if private {
        view! { <span class="chip chip-private">"Private"</span> }.into_any()
    } else {
        view! { <span hidden></span> }.into_any()
    };
    let subscribe: AnyView = if auth_enabled && signed_in {
        subscribe_button(&course.slug, viewer.subscriptions.contains(&course.slug), &format!("/course/{}", course.slug))
    } else {
        view! { <span hidden></span> }.into_any()
    };
    let mut content = vec![
        view! {
            <section class="course-hero" style={format!("--accent: {accent}")}>
                <p class="crumb">
                    <a href="/browse">"Browse"</a>" / "{course.title.clone()}
                </p>
                <h1>{course.title.clone()}</h1>
                <p class="course-lede">{description}</p>
                <div class="course-actions">
                    <a class="button button-primary" href={start_href}>"Start course"</a>
                    {subscribe}
                    {privacy_chip}
                    <span class="course-progress">
                        <span data-progress-count>{completed_text}</span>" of "{total_text}" lessons completed"
                    </span>
                </div>
            </section>
        }
        .into_any(),
    ];
    if let Some(banner) = ack {
        content.push(banner);
    }
    content.extend(outline);

    let title = course.title.clone();
    let data_course = course.slug.clone();
    Html(render_document(move || {
        view! {
            <Shell
                title={title}
                accent={accent}
                data_course
                auth_enabled
                signed_in
                admin
                content
            />
        }
    }))
    .into_response()
}

pub async fn lesson_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<LessonQuery>,
    Path((slug, lesson_slug)): Path<(String, String)>,
) -> Response {
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&slug) else {
        return not_found_page();
    };
    let Some(lesson) = course.lesson(&lesson_slug) else {
        return not_found_page();
    };

    let auth_enabled = state.pb_config.is_some();
    let map = state.access.get(state.pb.as_ref()).await;
    let viewer = access::viewer(&state, &headers).await;
    if !access::can_view(&course.slug, &viewer, &map) {
        return access_denied(&course.slug, &viewer);
    }
    let signed_in = viewer.signed_in();
    let admin = auth_enabled && viewer.is_admin();
    let done = server_progress(&state, &headers).await;
    let lesson_done = done.contains(&(course.slug.clone(), lesson_slug.clone()));
    let note_drawer: Option<AnyView> = if signed_in {
        let sess = viewer.session.as_ref().unwrap();
        match state
            .pb
            .as_ref()
            .unwrap()
            .get_notebook_entry(&sess.token, &sess.user_id, &course.slug, &lesson_slug)
            .await
        {
            Ok(entry) => Some(crate::web::notes::drawer(
                &state,
                course,
                lesson,
                entry.as_ref(),
                params.notes.as_deref(),
            )),
            Err(e) => {
                tracing::warn!("notebook fetch failed for {slug}/{lesson_slug}: {e}");
                None
            }
        }
    } else {
        None
    };
    let tutor_ready = signed_in && state.tutor.settings(state.pb.as_ref()).await.ready();

    let mut diags: Vec<Diagnostic> = Vec::new();
    let body_md = markdown::strip_title_heading(&lesson.body, &lesson.title);
    let file_path = course.dir.join(&lesson.file_name);
    let by_file = lessons_by_file(course);
    let ctx = MarkdownCtx {
        course,
        source: &file_path,
        lessons_by_file: &by_file,
        widgets: true,
    };
    let output = markdown::render_markdown(&body_md, &ctx, &state.registry, &mut diags);
    log_diagnostics(&diags);

    let mut extra_css: Vec<String> = Vec::new();
    let mut extra_js: Vec<String> = Vec::new();
    let mut kinds = output.widget_kinds.clone();
    if kinds.contains("stepper") {
        kinds.insert("math".to_string());
    }
    for kind in &kinds {
        extra_css.push(format!("/widgets/{kind}/style.css"));
        extra_js.push(format!("/widgets/{kind}/script.js"));
    }
    if note_drawer.is_some() {
        extra_css.push("/assets/notes.css".to_string());
        extra_js.push("/assets/notes.js".to_string());
    }
    if tutor_ready {
        extra_css.push("/assets/tutor.css".to_string());
        extra_js.push("/assets/tutor.js".to_string());
    }
    if (note_drawer.is_some() || tutor_ready) && !kinds.contains("math") {
        // Preview and chat replies render server-side markdown with $...$
        // math; KaTeX must be present even when the lesson itself has none.
        extra_css.push("/widgets/math/style.css".to_string());
        extra_js.push("/widgets/math/script.js".to_string());
    }

    let idx = course.lesson_index(&lesson_slug);
    let mut pager: Vec<AnyView> = Vec::new();
    if let Some(i) = idx {
        if i > 0 {
            let prev = &course.lessons[i - 1];
            let href = format!("/course/{}/{}", course.slug, prev.slug);
            let label = prev.title.clone();
            pager.push(
                view! {
                    <a class="pager-link pager-prev" href={href}>
                        <span class="pager-dir">"Previous"</span>
                        <span class="pager-title">{label}</span>
                    </a>
                }
                .into_any(),
            );
        }
        if i + 1 < course.lessons.len() {
            let next = &course.lessons[i + 1];
            let href = format!("/course/{}/{}", course.slug, next.slug);
            let label = next.title.clone();
            pager.push(
                view! {
                    <a class="pager-link pager-next" href={href}>
                        <span class="pager-dir">"Next"</span>
                        <span class="pager-title">{label}</span>
                    </a>
                }
                .into_any(),
            );
        }
    }

    let course_title = course.title.clone();
    let course_href = format!("/course/{}", course.slug);
    let description = lesson.description.clone().unwrap_or_default();
    let title = format!("{} — {}", lesson.title, course.title);
    let mark_course = course.slug.clone();
    let mark_lesson = lesson.slug.clone();
    let mut content = vec![
        view! {
            <p class="crumb">
                <a href="/browse">"Browse"</a>" / "
                <a href={course_href}>{course_title}</a>
            </p>
        }
        .into_any(),
    ];
    if let Some(banner) = ack_banner(course) {
        content.push(banner);
    }
    content.push(
        view! {
            <article class="lesson">
                <h1>{lesson.title.clone()}</h1>
                <p class="lesson-lede">{description}</p>
                <div class="lesson-body">{output.views}</div>
                <div class="lesson-complete">
                    <button
                        id="mark-complete"
                        type="button"
                        class={if lesson_done {
                            "button button-outline done"
                        } else {
                            "button button-outline"
                        }}
                        data-course={mark_course}
                        data-lesson={mark_lesson}
                    >
                        {if lesson_done { "Completed" } else { "Mark complete" }}
                    </button>
                </div>
                <nav class="pager">{pager}</nav>
            </article>
        }
        .into_any(),
    );
    if let Some(drawer) = note_drawer {
        content.push(drawer);
    }
    if tutor_ready {
        content.push(crate::web::tutor::drawer());
    }

    let accent = course
        .accent
        .clone()
        .unwrap_or_else(|| default_accent(&course.slug).to_string());
    let data_course = course.slug.clone();
    let data_lesson = lesson.slug.clone();
    Html(render_document(move || {
        view! {
            <Shell
                title={title}
                accent={accent}
                data_course
                data_lesson
                extra_css
                extra_js
                auth_enabled
                signed_in
                admin
                content
            />
        }
    }))
    .into_response()
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::course::Lesson;
    use std::path::PathBuf;

    fn lesson(slug: &str) -> Lesson {
        Lesson {
            slug: slug.into(),
            title: slug.into(),
            description: None,
            order: 0.0,
            file_name: format!("{slug}.md"),
            section: None,
            body: String::new(),
        }
    }

    fn course(lessons: &[&str]) -> Course {
        Course::test_stub(
            "c".into(),
            "Course".into(),
            None,
            lessons.iter().map(|s| lesson(s)).collect(),
            PathBuf::from("/nowhere"),
        )
    }

    fn done(pairs: &[&str]) -> HashSet<(String, String)> {
        pairs.iter().map(|p| ("c".to_string(), p.to_string())).collect()
    }

    #[test]
    fn continue_targets_first_unfinished_lesson() {
        let c = course(&["a", "b", "c"]);
        let (label, href) = continue_target(&c, &done(&["a"]));
        assert_eq!(label, "Continue");
        assert_eq!(href, "/course/c/b");
    }

    #[test]
    fn all_done_offers_review_of_last_lesson() {
        let c = course(&["a", "b"]);
        let (label, href) = continue_target(&c, &done(&["a", "b"]));
        assert_eq!(label, "Review");
        assert_eq!(href, "/course/c/b");
    }

    #[test]
    fn empty_course_opens_outline() {
        let c = course(&[]);
        let (label, href) = continue_target(&c, &done(&[]));
        assert_eq!(label, "Open");
        assert_eq!(href, "/course/c");
    }

    #[test]
    fn progress_counts_only_this_courses_lessons() {
        let c = course(&["a", "b"]);
        let p = card_progress(&c, &done(&["a", "foreign"]));
        assert_eq!((p.completed, p.total), (1, 2));
    }
}
