use std::fs;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use brillante::app;
use brillante::config::AppConfig;

/// Every test builds its own course in a temp directory, so no assertion
/// depends on course content shipped in the repository.
struct Fixture {
    _dir: tempfile::TempDir,
    router: axum::Router,
}

const COURSE_TOML: &str = r##"title = "Fixture Course"
slug = "fixture-course"
description = "A generated test course"
accent = "#0ea5e9"

[[sections]]
title = "Getting started"
slug = "basics"
description = "First steps in the fixture."

[[sections]]
title = "Extras"
"##;

const CONFIG_TOML: &str = r##"[widgets."com.example.fixture.quiz_basics"]
question = "What does the type attribute select?"
options = ["The widget kind", "The server URL"]
answer = 0
explanation = "type names the kind; id looks up the config."

[widgets."com.example.fixture.slider_oven"]
label = "Oven temperature"
min = 120
max = 260
step = 5
initial = 180
unit = "°C"

[widgets."com.example.fixture.card_flip"]
front = "What is Brillante?"
back = "A course platform that renders markdown."

[widgets."com.example.fixture.plot_growth"]
title = "Exponential growth"
series = [
    { name = "doubles every generation", color = "#6366f1", points = [[0, 1], [1, 2], [2, 4], [3, 8]] },
]

[widgets."com.example.fixture.ack_policy"]
statement = "You acknowledge the fixture citation policy."
button_label = "I acknowledge"
done_message = "Policy acknowledged."

[widgets."com.example.fixture.link_mit"]
title = "MIT OpenCourseWare"
url = "https://ocw.mit.edu/courses/2-71-optics-spring-2009/"
source = "MIT OpenCourseWare"
kind = "video"
description = "Video lectures mirrored by this fixture."

[widgets."com.example.fixture.slider_ratio"]
label = "Wavelength"
min = 380
max = 750
step = 5
initial = 450
display = "{value} nm — {result:.1}× vs 650 nm"
readout = "(650/x)^4"

[widgets."com.example.fixture.int_probe"]
expression = "1.5 + 0.5 / (x * x)"
domain = [0.4, 2.0]
xlabel = "distance x"
readout = "f({x:0.1f}) = {y:0.3f}"
"##;

const LESSON_WELCOME: &str = r#"---
title: Welcome
description: The first fixture lesson.
order: 1
section: basics
---

Welcome to the **fixture** course, with *italics*, `inline code`, and
~~strikethrough~~.

| Feature | Where it lives |
|:--- | ---: |
| Lesson metadata | frontmatter |
| Widget config | `config.toml` |

> Quoted material renders as a blockquote.

Ground rules:

- **Bold lead.** trailing text
- **Another lead.** more text
- see <https://example.com/docs> for reference

```rust
fn main() {
    println!("lessons are just files");
}
```

<script>alert('never rendered')</script>

<b>bold via HTML</b>
"#;

const LESSON_WIDGETS: &str = r#"---
title: Widgets
description: The second fixture lesson.
order: 2
section: basics
---

Here is a quiz configured in `config.toml`:

<Widget type="quiz" id="com.example.fixture.quiz_basics" />

And a widget whose type does not exist:

<Widget type="nonexistent" id="com.example.fixture.does_not_matter" />
"#;

const LESSON_SLIDERS: &str = r#"---
title: Sliders and flashcards
description: The third fixture lesson.
order: 3
section: extras
---

<Widget type="slider" id="com.example.fixture.slider_oven" />

<Widget type="slider" id="com.example.fixture.slider_ratio" />

<Widget type="flashcard" id="com.example.fixture.card_flip" />

<Widget type="interactive" id="com.example.fixture.int_probe" />
"#;

const LESSON_PLOTS: &str = r#"---
title: Plots and links
description: The fourth fixture lesson.
order: 4
---

<Widget type="plot" id="com.example.fixture.plot_growth" />

[Back to *Welcome*](01-welcome.md)

![The fixture mark](images/logo.svg)
"#;

const LESSON_ACK: &str = r#"---
title: Acknowledgment
description: The fifth fixture lesson.
order: 5
section: basics
---

<Widget type="acknowledge" id="com.example.fixture.ack_policy" />
"#;

const LESSON_RESOURCES: &str = r#"---
title: Resources
description: The sixth fixture lesson.
order: 6
section: extras
---

<Widget type="resource-link" id="com.example.fixture.link_mit" />
"#;

const LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="6" fill="#0ea5e9"/></svg>
"##;

fn fixture() -> Fixture {
    build_fixture(None)
}

/// Fixture whose state has a PocketBase configured that no test will reach:
/// exercises the auth-enabled UI and anonymous API behavior without any
/// network dependency.
fn fixture_with_auth() -> Fixture {
    build_fixture(Some(brillante::config::PocketBaseConfig {
        url: "http://127.0.0.1:1".into(),
        secure_cookies: false,
        public_url: "http://127.0.0.1:3000".into(),
        bootstrap_admins: Vec::new(),
    }))
}

fn build_fixture(pocketbase: Option<brillante::config::PocketBaseConfig>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let course = dir.path().join("fixture-course");
    fs::create_dir_all(course.join("images")).unwrap();
    fs::write(course.join("course.toml"), COURSE_TOML).unwrap();
    fs::write(course.join("config.toml"), CONFIG_TOML).unwrap();
    fs::write(course.join("01-welcome.md"), LESSON_WELCOME).unwrap();
    fs::write(course.join("02-widgets.md"), LESSON_WIDGETS).unwrap();
    fs::write(course.join("03-sliders.md"), LESSON_SLIDERS).unwrap();
    fs::write(course.join("04-plots.md"), LESSON_PLOTS).unwrap();
    fs::write(course.join("05-ack.md"), LESSON_ACK).unwrap();
    fs::write(course.join("06-resources.md"), LESSON_RESOURCES).unwrap();
    fs::write(course.join("images/logo.svg"), LOGO_SVG).unwrap();

    let config = AppConfig {
        host: "127.0.0.1".into(),
        port: 0,
        courses: vec![course],
        pocketbase,
        warnings: Vec::new(),
    };
    Fixture {
        _dir: dir,
        router: app::router(&config),
    }
}

async fn get(router: &axum::Router, path: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// GET that keeps the response metadata: (status, Location, Set-Cookie list).
async fn get_redirect(
    router: &axum::Router,
    path: &str,
) -> (StatusCode, String, Vec<String>) {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(axum::http::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let cookies: Vec<String> = response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    (status, location, cookies)
}

async fn post_form(router: &axum::Router, path: &str, body: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_json(router: &axum::Router, path: &str) -> (StatusCode, serde_json::Value) {
    let (status, body) = get(router, path).await;
    let json = serde_json::from_str(&body).unwrap_or_default();
    (status, json)
}

async fn post_json_with_cookie(
    router: &axum::Router,
    path: &str,
    body: &str,
    cookie: &str,
) -> StatusCode {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

#[tokio::test]
async fn home_lists_registered_courses() {
    let f = fixture();
    let (status, body) = get(&f.router, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Fixture Course"));
    assert!(body.contains("6 lessons"));
    assert!(body.contains("/course/fixture-course"));
}

#[tokio::test]
async fn course_page_lists_lessons_in_order() {
    let f = fixture();
    let (status, body) = get(&f.router, "/course/fixture-course").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Welcome"));
    assert!(body.contains("Widgets"));
    let welcome = body.find("Welcome").unwrap();
    let widgets = body.find("Widgets").unwrap();
    assert!(welcome < widgets, "lessons must be ordered by frontmatter");
}

#[tokio::test]
async fn course_page_groups_sections() {
    let f = fixture();
    let (status, body) = get(&f.router, "/course/fixture-course").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"class="section-title""#));
    assert!(body.contains("Getting started"));
    assert!(body.contains("First steps in the fixture."));
    assert!(body.contains("Extras"));
    // ungrouped lesson (Plots) renders before any section heading
    let plots = body.find("Plots and links").unwrap();
    let first_section = body.find("Getting started").unwrap();
    assert!(plots < first_section, "ungrouped lessons render before sections");
    // global numbering continues across groups: Acknowledgment is lesson 5
    let ack = body.find(r#"data-lesson="ack""#).unwrap();
    assert!(body[ack..].contains("<span class=\"lesson-num\">5</span>"));
    // two section lists plus the ungrouped list
    assert_eq!(body.matches("<ol class=\"lesson-list\">").count(), 3);
}

#[tokio::test]
async fn acknowledge_banner_and_widget_render() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course").await;
    assert!(body.contains("ack-banner"));
    assert!(body.contains("com.example.fixture.ack_policy"));
    assert!(body.contains(r#"data-ack-href="/course/fixture-course/ack""#) || body.contains(r#"href="/course/fixture-course/ack""#));
    assert!(body.contains("Read &amp; acknowledge") || body.contains("Read & acknowledge"));

    let (_, body) = get(&f.router, "/course/fixture-course/ack").await;
    assert!(body.contains(r#"data-widget-type="acknowledge""#));
    assert!(body.contains(r#"data-widget-id="com.example.fixture.ack_policy""#));
    assert!(body.contains("You acknowledge the fixture citation policy."));
    assert!(body.contains("/widgets/acknowledge/script.js"));
    assert!(body.contains("/widgets/acknowledge/style.css"));
}

#[tokio::test]
async fn resource_link_renders_outbound_card() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/resources").await;
    assert!(body.contains(r#"data-widget-type="resource-link""#));
    assert!(body.contains(r#"href="https://ocw.mit.edu/courses/2-71-optics-spring-2009/""#));
    assert!(body.contains(r#"target="_blank""#));
    assert!(body.contains("MIT OpenCourseWare"));
    assert!(body.contains("/widgets/resource-link/script.js"));
    assert!(body.contains("/widgets/resource-link/style.css"));
}

#[tokio::test]
async fn lesson_renders_widget_with_config_from_toml() {
    let f = fixture();
    let (status, body) = get(&f.router, "/course/fixture-course/widgets").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"data-widget-type="quiz""#));
    assert!(body.contains(r#"data-widget-id="com.example.fixture.quiz_basics""#));
    assert!(
        body.contains("The widget kind"),
        "config.toml content must reach the widget"
    );
    assert!(
        body.contains("unknown widget type &#x27;nonexistent&#x27;")
            || body.contains("unknown widget type 'nonexistent'"),
        "an unknown widget type must render a contained error card"
    );
}

#[tokio::test]
async fn lesson_suppresses_author_html() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(!body.contains("<script>alert"));
    assert!(!body.contains("<b>bold via HTML</b>"));
    assert!(body.contains("<blockquote>"));
    assert!(body.contains("<table>"));
    assert!(body.contains("language-rust"));
    assert!(body.contains("<s>strikethrough</s>"));
}

#[tokio::test]
async fn lesson_loads_only_needed_widget_assets() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/widgets").await;
    assert!(body.contains("/widgets/quiz/style.css"));
    assert!(body.contains("/widgets/quiz/script.js"));
    assert!(!body.contains("/widgets/plot/"));
}

#[tokio::test]
async fn unknown_course_and_lesson_404() {
    let f = fixture();
    let (status, _) = get(&f.router, "/course/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&f.router, "/course/fixture-course/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn widget_asset_routes() {
    let f = fixture();
    let (status, body) = get(&f.router, "/widgets/quiz/script.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("data-widget-type=\"quiz\""));
    let (status, _) = get(&f.router, "/widgets/quiz/style.css").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = get(&f.router, "/widgets/doesnotexist/script.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&f.router, "/widgets/quiz/other.bin").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn content_files_served_safely() {
    let f = fixture();
    let (status, body) = get(&f.router, "/content/fixture-course/images/logo.svg").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<svg"));
    let (status, _) = get(&f.router, "/content/fixture-course/config.toml").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = get(&f.router, "/content/fixture-course/../../Cargo.toml").await;
    assert_ne!(status, StatusCode::OK);
    let (status, _) = get(&f.router, "/content/fixture-course/.hidden").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn site_assets_served() {
    let f = fixture();
    let (status, body) = get(&f.router, "/assets/site.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("course-card"));
    let (status, body) = get(&f.router, "/assets/site.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("brillante.progress.v1"));
    assert!(body.contains("brillante.ack.v1"));
    let (status, _) = get(&f.router, "/assets/favicon.svg").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = get(&f.router, "/assets/nope.css").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn scratchpad_renders_on_course_pages_only() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(body.contains(r#"id="scratchpad""#));
    assert!(body.contains("scratchpad-toggle"));
    assert!(body.contains("/assets/scratchpad.js"));
    assert!(body.contains("/assets/scratchpad.css"));

    let (_, body) = get(&f.router, "/course/fixture-course").await;
    assert!(body.contains(r#"id="scratchpad""#));
    assert!(body.contains("/assets/scratchpad.js"));

    let (_, body) = get(&f.router, "/").await;
    assert!(!body.contains(r#"id="scratchpad""#));
    assert!(!body.contains("/assets/scratchpad.js"));

    let (status, body) = get(&f.router, "/course/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!body.contains(r#"id="scratchpad""#));
}

#[tokio::test]
async fn scratchpad_assets_served() {
    let f = fixture();
    let (status, body) = get(&f.router, "/assets/scratchpad.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("pyodide"));
    assert!(body.contains("brillante.scratch.history.v1"));
    let (status, body) = get(&f.router, "/assets/scratchpad.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("#scratchpad"));
    let (status, _) = get(&f.router, "/assets/scratchpad.nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn admin_endpoints_work() {
    let f = fixture();
    let (status, _) = get(&f.router, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    let response = f
        .router
        .clone()
        .oneshot(
            Request::post("/admin/reload")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (status, body) = get(&f.router, "/admin/diagnostics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn markdown_link_to_lesson_is_rewritten() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/plots").await;
    assert!(body.contains(r#"href="/course/fixture-course/welcome""#));
    assert!(body.contains(r#"src="/content/fixture-course/images/logo.svg""#));
}

#[tokio::test]
async fn sliders_and_plots_render_values() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/sliders").await;
    assert!(body.contains("180 °C"));
    assert!(body.contains(r#"data-widget-type="flashcard""#));
    let (_, body) = get(&f.router, "/course/fixture-course/plots").await;
    assert!(body.contains(r#"data-widget-type="plot""#));
    assert!(body.contains("Exponential growth"));
}

#[tokio::test]
async fn tight_lists_keep_inline_elements_in_one_paragraph() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(
        body.contains("<li><p><strong>Bold lead.</strong> trailing text</p></li>"),
        "a bold lead must not split its list item into multiple paragraphs"
    );
    assert!(
        body.contains("<li><p>see <a href=\"https://example.com/docs\""),
        "autolinks must stay inside the item's paragraph"
    );
    assert!(!body.contains("</strong><p>"), "inline tags must not start a new <p>");
    assert!(!body.contains("</p><a"), "links must not sit between two <p>s");
}

#[tokio::test]
async fn slider_renders_computed_readout() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/sliders").await;
    assert!(
        body.contains("450 nm — 4.4× vs 650 nm"),
        "the slider must render the evaluated readout at its initial value"
    );
    assert!(body.contains("readoutAst"));
    assert!(!body.contains("(650/x)^4"), "the client must not receive the source expression");
}

#[tokio::test]
async fn interactive_renders_x_slider_and_evaluated_readout() {
    let f = fixture();
    let (_, body) = get(&f.router, "/course/fixture-course/sliders").await;
    assert!(
        body.contains(r#"data-param="x""#),
        "a readout referencing x/y must get an adjustable x slider"
    );
    assert!(
        body.contains("f(1.2) = 1.847"),
        "the readout must be server-evaluated at the probe point"
    );
    assert!(
        !body.contains("<div class=\"interactive-readout\">{x"),
        "raw placeholders must never leak into the rendered readout"
    );
}

#[tokio::test]
async fn account_page_reports_disabled_without_pocketbase() {
    let f = fixture();
    let (status, body) = get(&f.router, "/account").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Accounts are not configured"));
    let (_, home) = get(&f.router, "/").await;
    assert!(
        !home.contains(r#"href="/account""#),
        "no account link in the header when auth is disabled"
    );
}

#[tokio::test]
async fn login_post_without_pocketbase_redirects_with_error() {
    let f = fixture();
    let (status, _) = post_form(&f.router, "/auth/login", "email=a@b.c&password=x").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn auth_enabled_header_shows_account_link() {
    let f = fixture_with_auth();
    let (_, home) = get(&f.router, "/").await;
    assert!(home.contains(r#"href="/account""#));
    assert!(home.contains(r#"id="account-link""#));
    assert!(
        !home.contains(r#"data-auth="server""#),
        "anonymous visitors must not get server-backed progress"
    );
}

#[tokio::test]
async fn progress_api_requires_authentication() {
    let f = fixture_with_auth();
    let (status, _) = get(&f.router, "/api/progress").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let status =
        post_json_with_cookie(&f.router, "/api/progress", r#"{"course":"a","lesson":"b","done":true}"#, "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn progress_api_rejects_garbage_cookie() {
    let f = fixture_with_auth();
    let status = post_json_with_cookie(
        &f.router,
        "/api/progress",
        r#"{"course":"a","lesson":"b","done":true}"#,
        "brillante.session=not.a.jwt",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_me_reports_anonymous_without_network() {
    let f = fixture_with_auth();
    let (status, json) = get_json(&f.router, "/auth/me").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["enabled"], serde_json::json!(true));
    assert_eq!(json["authenticated"], serde_json::json!(false));
}

#[tokio::test]
async fn oauth_start_without_pocketbase_redirects_disabled() {
    let f = fixture();
    let (status, location, _) = get_redirect(&f.router, "/auth/oauth/google").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location, "/account?error=disabled");
}

#[tokio::test]
async fn oauth_start_degrades_when_pocketbase_unreachable() {
    let f = fixture_with_auth();
    let (status, location, _) = get_redirect(&f.router, "/auth/oauth/oidc").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location, "/account?error=unavailable");
}

#[tokio::test]
async fn oauth_callback_without_flow_cookie_redirects_expired() {
    let f = fixture();
    let (status, location, cookies) = get_redirect(
        &f.router,
        "/auth/oauth/google/callback?code=abc&state=def",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location, "/account?error=oauth_state");
    assert!(
        cookies.iter().any(|c| c.starts_with("brillante.oauth=;")),
        "the flow cookie must be cleared on callback: {cookies:?}"
    );
}

#[tokio::test]
async fn account_page_has_no_oauth_buttons_when_pb_unreachable() {
    let f = fixture_with_auth();
    let (_, body) = get(&f.router, "/account").await;
    assert!(!body.contains("oauth-button"));
    assert!(!body.contains("/auth/oauth/"));
}

#[tokio::test]
async fn lesson_progress_ui_renders_anonymously() {
    let f = fixture_with_auth();
    let (_, body) = get(&f.router, "/course/fixture-course/widgets").await;
    assert_eq!(body.matches("mark-complete").count(), 1);
    assert!(!body.contains(r#"data-auth="server""#));
    let (_, course) = get(&f.router, "/course/fixture-course").await;
    assert!(course.contains(r#"<span data-progress-count>0</span>"#));
}

#[tokio::test]
async fn catalog_without_accounts_has_no_subscribe_ui() {
    let f = fixture();
    let (_, body) = get(&f.router, "/").await;
    assert!(body.contains("Fixture Course"));
    assert!(!body.contains("/my/subscriptions"), "no form posts without accounts");
    assert!(!body.contains("My courses"), "no my-courses section without accounts");
    // Courses with no access record stay public by default.
    let (status, _) = get(&f.router, "/course/fixture-course").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn admin_pages_404_without_accounts() {
    let f = fixture();
    let (status, _) = get(&f.router, "/admin/courses").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&f.router, "/admin/users").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn subscriptions_require_sign_in() {
    let f = fixture();
    let (status, location) = post_form_redirect(
        &f.router,
        "/my/subscriptions",
        "course=fixture-course&action=subscribe&next=/",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location, "/account?next=%2F");
}

#[tokio::test]
async fn catalog_degrades_to_anonymous_when_pb_unreachable() {
    let f = fixture_with_auth();
    let (_, body) = get(&f.router, "/").await;
    assert!(body.contains("Fixture Course"), "no access record means public");
    assert!(
        !body.contains("/my/subscriptions"),
        "an unresolvable viewer must not see subscribe controls"
    );
    assert!(!body.contains("My courses"));
}

#[tokio::test]
async fn browse_lists_courses_with_nav_links() {
    let f = fixture();
    let (status, body) = get(&f.router, "/browse").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Browse courses"));
    assert!(body.contains("Fixture Course"));
    assert!(body.contains("6 lessons"));
    assert!(body.contains("/course/fixture-course"));
    // topbar navigation on every page
    assert!(body.contains(r#"<a href="/">Home</a>"#));
    assert!(body.contains(r#"<a href="/browse">Browse</a>"#));
    let (_, lesson) = get(&f.router, "/course/fixture-course/widgets").await;
    assert!(lesson.contains(r#"<a href="/browse">Browse</a>"#), "nav on lesson pages");
    assert!(lesson.contains(r#"class="crumb""#));
    let (_, not_found) = get(&f.router, "/nope").await;
    assert!(not_found.contains("Browse all courses"));
}

#[tokio::test]
async fn browse_without_accounts_has_no_subscribe_ui() {
    let f = fixture();
    let (_, body) = get(&f.router, "/browse").await;
    assert!(body.contains("Fixture Course"));
    assert!(
        !body.contains("/my/subscriptions"),
        "no form posts without accounts"
    );
}

#[tokio::test]
async fn browse_degrades_to_anonymous_when_pb_unreachable() {
    let f = fixture_with_auth();
    let (_, body) = get(&f.router, "/browse").await;
    assert!(body.contains("Fixture Course"), "no access record means public");
    assert!(
        !body.contains("/my/subscriptions"),
        "an unresolvable viewer must not see subscribe controls"
    );
}

#[tokio::test]
async fn browse_search_matches_courses_and_lessons() {
    let f = fixture();
    let (status, body) = get(&f.router, "/browse?q=widgets").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("1 course(s) match"), "summary counts courses");
    assert!(body.contains("/course/fixture-course/widgets"), "lesson deep link");
    assert!(body.contains("class=\"search-lessons\""));
    assert!(body.contains("Fixture Course"), "matched course card");
    assert!(
        body.contains("value=\"widgets\""),
        "search box retains the query"
    );
}

#[tokio::test]
async fn browse_search_is_case_insensitive_and_scoped() {
    let f = fixture();
    let (_, body) = get(&f.router, "/browse?q=SLIDERS").await;
    assert!(body.contains("1 course(s) match"));
    assert!(body.contains("/course/fixture-course/sliders"));
    let (_, body) = get(&f.router, "/browse?q=zzz-nothing").await;
    assert!(body.contains("No matches"));
    assert!(body.contains("0 course(s) match"));
}

#[tokio::test]
async fn browse_unfiltered_shows_all_and_no_summary() {
    let f = fixture();
    let (_, body) = get(&f.router, "/browse").await;
    assert!(body.contains("Browse courses"));
    assert!(!body.contains("course(s) match"));
    assert!(body.contains("class=\"search-form\""));
}

/// POST form helper that keeps the redirect target.
async fn post_form_redirect(router: &axum::Router, path: &str, body: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header(
                    axum::http::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(axum::http::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    (status, location)
}

#[tokio::test]
async fn notebook_surfaces_need_accounts() {
    let f = fixture();
    let (status, _) = get(&f.router, "/notes/fixture-course").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no accounts means no notebooks");
    let (status, _) = get(&f.router, "/course/fixture-course/welcome").await;
    assert_eq!(status, StatusCode::OK);
    let (_, lesson) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(!lesson.contains("id=\"notebook\""), "no drawer without accounts");
    let (status, _) = post_form(
        &f.router,
        "/notes/fixture-course/welcome",
        "body=hello&anchor=&here_title=",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn notebook_degrades_when_pb_unreachable() {
    let f = fixture_with_auth();
    let (_, lesson) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(
        !lesson.contains("id=\"notebook\""),
        "unresolvable viewer (PB down) renders no drawer"
    );
    let (status, _) = get(&f.router, "/notes/fixture-course").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "index redirects to sign-in");
}

#[tokio::test]
async fn admin_delete_requires_admin() {
    let f = fixture();
    let (status, _) = post_form(
        &f.router,
        "/admin/users/delete",
        "user_id=abc&confirm=on",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&f.router, "/admin").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let f = fixture_with_auth();
    let (status, _) = get(&f.router, "/admin").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unreachable PB degrades to no admin");
    let (status, _) = post_form(&f.router, "/admin/users/delete", "user_id=abc&confirm=on").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tutor_is_inert_without_configuration() {
    let f = fixture();
    let (_, lesson) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(!lesson.contains("id=\"tutor\""), "no drawer without accounts");
    let (status, _) = get(&f.router, "/admin/tutor").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = post_json(
        &f.router,
        "/api/tutor/chat",
        r#"{"course":"fixture-course","lesson":"welcome","messages":[{"role":"user","content":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no pb means no tutor");
    let _: &str = &body;

    let f = fixture_with_auth();
    let (_, lesson) = get(&f.router, "/course/fixture-course/welcome").await;
    assert!(
        !lesson.contains("id=\"tutor\""),
        "unreachable pb means not ready"
    );
    let (status, _) = post_json(
        &f.router,
        "/api/tutor/chat",
        r#"{"course":"fixture-course","lesson":"welcome","messages":[{"role":"user","content":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn post_json(router: &axum::Router, path: &str, body: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}
