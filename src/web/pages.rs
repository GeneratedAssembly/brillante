use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use leptos::prelude::*;

use crate::content::course::Course;
use crate::content::markdown::{self, MarkdownCtx};
use crate::diag::{log_diagnostics, Diagnostic};
use crate::render::render_document;
use crate::web::AppState;

fn lessons_by_file(course: &Course) -> HashMap<String, String> {
    course
        .lessons
        .iter()
        .map(|l| (l.file_name.clone(), l.slug.clone()))
        .collect()
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

#[component]
fn Shell(
    title: String,
    #[prop(optional)] accent: Option<String>,
    #[prop(optional)] data_course: Option<String>,
    #[prop(optional)] data_lesson: Option<String>,
    #[prop(optional)] extra_css: Vec<String>,
    #[prop(optional)] extra_js: Vec<String>,
    content: Vec<AnyView>,
) -> impl IntoView {
    let style = accent.map(|a| format!("--accent: {a}"));
    let extra_css = extra_css
        .into_iter()
        .map(|href| view! { <link rel="stylesheet" href={href} /> }.into_any())
        .collect::<Vec<_>>();
    let extra_js = extra_js
        .into_iter()
        .map(|src| view! { <script src={src} defer></script> }.into_any())
        .collect::<Vec<_>>();

    let header: AnyView = view! {
        <header class="topbar">
            <div class="topbar-inner">
                <a class="brand" href="/">"Brillante"</a>
                <nav class="topnav">
                    <a href="/">"Courses"</a>
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
        site_js,
        view! { <div>{extra_js}</div> }.into_any(),
    ];

    let body = match (data_course, data_lesson) {
        (Some(c), Some(l)) => view! {
            <body style={style} data-course={c} data-lesson={l}>{inner}</body>
        }
        .into_any(),
        (Some(c), None) => view! {
            <body style={style} data-course={c}>{inner}</body>
        }
        .into_any(),
        _ => view! { <body style={style}>{inner}</body> }.into_any(),
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

fn not_found_page() -> Response {
    let content = vec![
        view! { <h1>"404"</h1> }.into_any(),
        view! { <p>"That page does not exist. "<a href="/">"Back to courses"</a>"."</p> }
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

pub async fn home(State(state): State<AppState>) -> Response {
    let store = state.store.read().await.clone();
    let cards: Vec<AnyView> = store
        .courses
        .iter()
        .map(course_card)
        .collect();
    let content = vec![
        view! {
            <section class="hero">
                <h1>"Learn something brilliant."</h1>
                <p class="hero-sub">"Interactive courses, served straight from markdown."</p>
            </section>
        }
        .into_any(),
        view! { <section class="course-grid">{cards}</section> }.into_any(),
    ];
    Html(render_document(move || {
        view! {
            <Shell title="Brillante".to_string() content />
        }
    }))
    .into_response()
}

fn course_card(c: &Course) -> AnyView {
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
    view! {
        <a class="course-card" href={href} style={style}>
            <h3>{title}</h3>
            <p class="course-card-desc">{desc}</p>
            <span class="course-card-meta">{count}</span>
        </a>
    }
    .into_any()
}

pub async fn course_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Response {
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&slug) else {
        return not_found_page();
    };

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
                view! {
                    <li class="lesson-item" data-lesson={lesson.slug.clone()}>
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
    let start_href = course
        .lessons
        .first()
        .map(|l| format!("/course/{}/{}", course.slug, l.slug))
        .unwrap_or_else(|| format!("/course/{}", course.slug));
    let ack = ack_banner(course);
    let mut content = vec![
        view! {
            <section class="course-hero" style={format!("--accent: {accent}")}>
                <p class="crumb">
                    <a href="/">"Courses"</a>" / "{course.title.clone()}
                </p>
                <h1>{course.title.clone()}</h1>
                <p class="course-lede">{description}</p>
                <div class="course-actions">
                    <a class="button button-primary" href={start_href}>"Start course"</a>
                    <span class="course-progress">
                        <span data-progress-count>"0"</span>" of "{total_text}" lessons completed"
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
            <Shell title={title} accent={accent} data_course content />
        }
    }))
    .into_response()
}

pub async fn lesson_page(
    State(state): State<AppState>,
    Path((slug, lesson_slug)): Path<(String, String)>,
) -> Response {
    let store = state.store.read().await.clone();
    let Some(course) = store.course(&slug) else {
        return not_found_page();
    };
    let Some(lesson) = course.lesson(&lesson_slug) else {
        return not_found_page();
    };

    let mut diags: Vec<Diagnostic> = Vec::new();
    let body_md = markdown::strip_title_heading(&lesson.body, &lesson.title);
    let file_path = course.dir.join(&lesson.file_name);
    let by_file = lessons_by_file(course);
    let ctx = MarkdownCtx {
        course,
        source: &file_path,
        lessons_by_file: &by_file,
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
                <a href="/">"Courses"</a>" / "
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
                        class="button button-outline"
                        data-course={mark_course}
                        data-lesson={mark_lesson}
                    >
                        "Mark complete"
                    </button>
                </div>
                <nav class="pager">{pager}</nav>
            </article>
        }
        .into_any(),
    );

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
                content
            />
        }
    }))
    .into_response()
}

