//! End-to-end auth + progress flow against a real PocketBase instance.
//!
//! Skipped unless POCKETBASE_TEST_URL points at a running server whose
//! `progress` collection exists (see pocketbase/pb_migrations and
//! pocketbase/run.sh). Example:
//!
//!   ./pocketbase/run.sh
//!   POCKETBASE_TEST_URL=http://127.0.0.1:8090 cargo test --test pocketbase_live

use std::fs;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use brillante::app;
use brillante::config::{AppConfig, PocketBaseConfig};

fn url() -> Option<String> {
    std::env::var("POCKETBASE_TEST_URL").ok()
}

fn fixture(pb_url: &str) -> (tempfile::TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let course = dir.path().join("live-course");
    fs::create_dir_all(&course).unwrap();
    fs::write(
        course.join("course.toml"),
        "title = \"Live Course\"\nslug = \"live-course\"\n",
    )
    .unwrap();
    fs::write(
        course.join("01-first.md"),
        "---\ntitle: First\n---\n\nHello.\n",
    )
    .unwrap();
    fs::write(
        course.join("02-second.md"),
        "---\ntitle: Second\n---\n\nMore.\n",
    )
    .unwrap();
    let config = AppConfig {
        host: "127.0.0.1".into(),
        port: 0,
        courses: vec![course],
        pocketbase: Some(PocketBaseConfig {
            url: pb_url.to_string(),
            secure_cookies: false,
            public_url: "http://127.0.0.1:3000".into(),
            bootstrap_admins: Vec::new(),
        }),
        warnings: Vec::new(),
    };
    (dir, app::router(&config))
}

async fn body_of(response: axum::response::Response) -> (StatusCode, String, Vec<String>) {
    let status = response.status();
    let cookies: Vec<String> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        String::from_utf8_lossy(&bytes).into_owned(),
        cookies,
    )
}

#[tokio::test]
async fn signup_login_and_progress_round_trip() {
    let Some(pb) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let (_dir, router) = fixture(&pb);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let email = format!("live-{}-{stamp}@test.local", std::process::id());
    let password = "livepassword123";

    let response = router
        .clone()
        .oneshot(
            Request::post("/auth/signup")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "email={email}&password={password}&passwordConfirm={password}&name=Live+Tester"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, _, cookies) = body_of(response).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "signup should redirect");
    let session = cookies
        .iter()
        .find(|c| c.starts_with("brillante.session="))
        .unwrap_or_else(|| panic!("no session cookie in {cookies:?}"))
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert!(
        session.contains("HttpOnly") || cookies.iter().any(|c| c.contains("HttpOnly")),
        "session cookie must be HttpOnly"
    );

    let response = router
        .clone()
        .oneshot(
            Request::get("/auth/me")
                .header(header::COOKIE, &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body, _) = body_of(response).await;
    assert_eq!(status, StatusCode::OK);
    let me: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(me["authenticated"], serde_json::json!(true));
    assert_eq!(me["name"], serde_json::json!("Live Tester"));

    let response = router
        .clone()
        .oneshot(
            Request::post("/api/progress")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(
                    r#"{"course":"live-course","lesson":"first","done":true}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = router
        .clone()
        .oneshot(
            Request::get("/api/progress")
                .header(header::COOKIE, &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (_, body, _) = body_of(response).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["courses"]["live-course"],
        serde_json::json!(["first"])
    );

    let (_, lesson_html, _) = body_of(
        router
            .clone()
            .oneshot(
                Request::get("/course/live-course/first")
                    .header(header::COOKIE, &session)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(lesson_html.contains(r#"data-auth="server""#));
    assert!(lesson_html.contains("Completed"));

    let response = router
        .clone()
        .oneshot(
            Request::post("/api/progress")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(
                    r#"{"course":"live-course","lesson":"first","done":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = router
        .clone()
        .oneshot(
            Request::get("/api/progress")
                .header(header::COOKIE, &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (_, body, _) = body_of(response).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        json["courses"]
            .get("live-course")
            .is_none_or(|v| v.as_array().is_some_and(|a| a.is_empty())),
        "lesson should be unmarked: {body}"
    );

}

#[tokio::test]
async fn login_rejects_bad_password_generically() {
    let Some(pb) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let (_dir, router) = fixture(&pb);
    let response = router
        .clone()
        .oneshot(
            Request::post("/auth/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("email=nobody-404@test.local&password=whatever12"))
                .unwrap(),
        )
        .await
        .unwrap();
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        location.contains("error=invalid"),
        "bad credentials must redirect to the generic error: {location}"
    );
}

#[tokio::test]
async fn oauth_start_reflects_pb_enabled_providers() {
    let Some(pb_url) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let pb = brillante::pb::PocketBase::new(&pb_url);
    let providers = pb.oauth_providers().await.expect("auth-methods parses");
    for p in &providers {
        assert!(!p.name.is_empty());
        assert!(!p.auth_url.is_empty());
        assert!(!p.state.is_empty(), "PB must pre-generate an OAuth2 state");
    }

    let (_dir, router) = fixture(&pb_url);
    // Probe an enabled provider when one exists, otherwise one that must be
    // rejected as unknown.
    let probe = providers
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "oidc".into());
    let response = router
        .clone()
        .oneshot(
            Request::get(format!("/auth/oauth/{probe}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if providers.is_empty() {
        assert_eq!(location, "/account?error=oauth");
    } else {
        assert!(
            location.starts_with("https://") || location.starts_with("http://"),
            "an enabled provider must redirect to the identity provider authorize URL: {location}"
        );
        assert!(
            location.contains("redirect_uri="),
            "authorize URL must carry the Brillante callback redirect_uri: {location}"
        );
        assert!(
            !location.contains("redirect_uri=$"),
            "authorize URL must not leak PB's empty placeholder: {location}"
        );
    }
}

#[tokio::test]
async fn oauth_callback_without_flow_cookie_redirects_expired() {
    let Some(pb) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let (_dir, router) = fixture(&pb);
    let response = router
        .clone()
        .oneshot(
            Request::get("/auth/oauth/oidc/callback?code=abc&state=def")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(location, "/account?error=oauth_state");
    let cookies: Vec<String> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_string))
        .collect();
    assert!(
        cookies.iter().any(|c| c.starts_with("brillante.oauth=;")),
        "the flow cookie must be cleared on callback: {cookies:?}"
    );
}

#[tokio::test]
async fn oauth_exchange_with_bogus_code_is_rejected_gracefully() {
    let Some(pb_url) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let pb = brillante::pb::PocketBase::new(&pb_url);
    let providers = pb.oauth_providers().await.expect("auth-methods parses");
    let Some(provider) = providers.first() else {
        eprintln!("no OAuth2 providers enabled on the test PB; skipping");
        return;
    };
    let result = pb
        .auth_with_oauth2(
            &provider.name,
            "bogus-code",
            &provider.code_verifier,
            "http://127.0.0.1:3000/auth/oauth/oidc/callback",
        )
        .await;
    assert!(
        matches!(result, Err(brillante::pb::PbError::Rejected(_))),
        "a bad authorization code must read as a user-facing rejection, got {result:?}"
    );
}

/// Helper: sign up a throwaway account and return its session cookie pair.
async fn signup_session(router: &axum::Router) -> String {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let email = format!("live-dash-{}-{stamp}@test.local", std::process::id());
    let password = "livepassword123";
    let response = router
        .clone()
        .oneshot(
            Request::post("/auth/signup")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "email={email}&password={password}&passwordConfirm={password}&name=Dash+Tester"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let (_, _, cookies) = body_of(response).await;
    cookies
        .iter()
        .find(|c| c.starts_with("brillante.session="))
        .unwrap_or_else(|| panic!("no session cookie in {cookies:?}"))
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn post(router: &axum::Router, path: &str, body: &str, session: &str) -> StatusCode {
    let response = router
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, session)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

async fn get(router: &axum::Router, path: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body, _) = body_of(response).await;
    (status, body)
}

async fn get_with(router: &axum::Router, path: &str, session: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(
            Request::get(path)
                .header(header::COOKIE, session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body, _) = body_of(response).await;
    (status, body)
}

#[tokio::test]
async fn dashboard_and_profile_reflect_progress() {
    let Some(pb) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let (_dir, router) = fixture(&pb);
    let session = signup_session(&router).await;

    assert_eq!(
        post(&router, "/my/subscriptions", "course=live-course&action=subscribe&next=/", &session)
            .await,
        StatusCode::SEE_OTHER
    );
    let response = router
        .clone()
        .oneshot(
            Request::post("/api/progress")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(
                    r#"{"course":"live-course","lesson":"first","done":true}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let (_, home, _) = body_of(
        router
            .clone()
            .oneshot(
                Request::get("/").header(header::COOKIE, &session).body(Body::empty()).unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(home.contains("Welcome back"), "signed-in home is a dashboard");
    assert!(home.contains("My courses"));
    assert!(home.contains("1 of 2 lessons"), "progress count rendered");
    assert!(home.contains("width: 50%"), "progress bar rendered");
    assert!(
        home.contains(r#"href="/course/live-course/second""#) && home.contains(">Continue<"),
        "continue targets the first unfinished lesson"
    );

    let (_, account, _) = body_of(
        router
            .clone()
            .oneshot(
                Request::get("/account")
                    .header(header::COOKIE, &session)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(account.contains("Dash Tester"), "profile shows display name");
    assert!(account.contains("Member since"), "profile shows member-since");
    assert!(account.contains("profile-stats"), "stats block rendered");
    assert!(account.contains("Keep going"), "in-progress list rendered");

    let (_, browse, _) = body_of(
        router
            .clone()
            .oneshot(
                Request::get("/browse?q=second")
                    .header(header::COOKIE, &session)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(browse.contains("1 course(s) match"));
    assert!(browse.contains("/course/live-course/second"));
}

#[tokio::test]
async fn notebook_save_preview_and_index_round_trip() {
    let Some(pb) = url() else {
        eprintln!("POCKETBASE_TEST_URL not set; skipping live pocketbase test");
        return;
    };
    let (_dir, router) = fixture(&pb);
    let session = signup_session(&router).await;

    let (_, lesson) = get_with(&router, "/course/live-course/first", &session).await;
    assert!(lesson.contains("id=\"notebook\""), "signed-in drawer renders");
    assert!(lesson.contains("/assets/notes.js"), "drawer pulls its assets");
    let (_, anon) = get(&router, "/course/live-course/first").await;
    assert!(!anon.contains("id=\"notebook\""), "anonymous sees no drawer");

    let status = post(
        &router,
        "/notes/live-course/first",
        "body=See+[[second]]+and+[[nope]].&anchor=&here_title=",
        &session,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, saved_page) = get_with(&router, "/course/live-course/first?notes=saved", &session).await;
    assert!(saved_page.contains("Note saved."), "status line on redirect target");

    let (_, index) = get_with(&router, "/notes/live-course", &session).await;
    assert!(index.contains("Notes — Live Course"));
    assert!(index.contains("/course/live-course/second"), "lesson shorthand");
    assert!(index.contains("dead-link"), "unknown slug renders dead");

    let response = router
        .clone()
        .oneshot(
            Request::post("/api/notes/preview")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(
                    r#"{"course":"live-course","lesson":"first","body":"draft [[second]]","anchor":"","here_title":""}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, fragment, _) = body_of(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(fragment.contains("/course/live-course/second"), "preview expands shorthand");

    let status = post(
        &router,
        "/notes/live-course/first",
        "body=at+%40here&anchor=bogus-anchor&here_title=Bogus+Anchor",
        &session,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, index) = get_with(&router, "/notes/live-course", &session).await;
    assert!(index.contains("at @here"), "unknown anchor drops to literal text");

    let status = post(&router, "/notes/live-course/first", "body=&anchor=&here_title=", &session).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, index) = get_with(&router, "/notes/live-course", &session).await;
    assert!(index.contains("No notes yet"), "empty body deletes the entry");

    let other = signup_session(&router).await;
    let (_, other_index) = get_with(&router, "/notes/live-course", &other).await;
    assert!(other_index.contains("No notes yet"), "entries are owner-scoped");
    let other_status = post(
        &router,
        "/notes/live-course/first",
        "body=intruder&anchor=&here_title=",
        &other,
    )
    .await;
    assert_eq!(other_status, StatusCode::SEE_OTHER);
    let (_, mine) = get_with(&router, "/notes/live-course", &session).await;
    assert!(!mine.contains("intruder"), "one user's save never appears in another's notebook");
}
