use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine as _;
use cookie::{Cookie, CookieBuilder, SameSite};
use cookie::time::Duration;
use percent_encoding::AsciiSet;
use serde::{Deserialize, Serialize};

use crate::pb::PbError;
use crate::web::auth::{header_value, safe_next, NextParam};
use crate::web::session;
use crate::web::AppState;

/// Short-lived httpOnly cookie carrying the in-flight OAuth2 roundtrip
/// (PB-generated state, PKCE verifier, post-login destination). The identity
/// provider redirects back with the state; only a browser that received this
/// cookie can complete the flow.
pub const OAUTH_COOKIE: &str = "brillante.oauth";
const TEN_MINUTES: i64 = 10 * 60;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct OAuthFlow {
    provider: String,
    state: String,
    verifier: String,
    next: String,
}

fn encode_flow(flow: &OAuthFlow) -> String {
    let bytes = serde_json::to_vec(flow).expect("oauth flow serializes");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn decode_flow(raw: &str) -> Option<OAuthFlow> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn oauth_cookie(flow: &OAuthFlow, secure: bool) -> Cookie<'static> {
    CookieBuilder::new(OAUTH_COOKIE, encode_flow(flow))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::seconds(TEN_MINUTES))
        .secure(secure)
        .build()
}

fn cleared_oauth_cookie() -> Cookie<'static> {
    CookieBuilder::new(OAUTH_COOKIE, "")
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::seconds(0))
        .build()
}

/// Percent-escape set for query component values (RFC 3986 unreserved
/// characters stay readable).
const QUERY_ESCAPE: &AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'~');

fn percent_encode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, QUERY_ESCAPE).to_string()
}

/// Login-page link for one provider button.
pub fn start_href(provider: &str, next: &str) -> String {
    if next == "/" {
        format!("/auth/oauth/{provider}")
    } else {
        format!("/auth/oauth/{provider}?next={}", percent_encode(next))
    }
}

/// The absolute URL this server expects the identity provider to redirect
/// back to. It must be registered with the provider (Auth0 application's
/// Allowed Callback URLs).
fn callback_url(public_url: &str, provider: &str) -> String {
    format!(
        "{}/auth/oauth/{}/callback",
        public_url.trim_end_matches('/'),
        provider
    )
}

/// PocketBase pre-builds each provider's authorize URL and leaves a trailing
/// `&redirect_uri=` placeholder for the caller to fill in.
fn with_redirect_uri(auth_url: &str, redirect_uri: &str) -> String {
    let encoded = percent_encode(redirect_uri);
    if auth_url.ends_with("&redirect_uri=") {
        format!("{auth_url}{encoded}")
    } else {
        format!("{auth_url}&redirect_uri={encoded}")
    }
}

fn error_redirect(code: &str) -> Response {
    Redirect::to(&format!("/account?error={code}")).into_response()
}

fn public_url(state: &AppState) -> String {
    state
        .pb_config
        .as_ref()
        .map(|c| c.public_url.clone())
        .unwrap_or_default()
}

/// `GET /auth/oauth/{provider}` — sends the visitor to the identity provider.
pub async fn start(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(next): Query<NextParam>,
) -> Response {
    let Some(pb) = &state.pb else {
        return error_redirect("disabled");
    };
    let secure = state.pb_config.as_ref().is_some_and(|c| c.secure_cookies);
    let providers = match pb.oauth_providers().await {
        Ok(list) => list,
        Err(e) => {
            tracing::warn!("oauth start ({}): {e}", provider);
            return error_redirect("unavailable");
        }
    };
    let Some(found) = providers.iter().find(|p| p.name == provider) else {
        tracing::warn!("oauth start: provider '{provider}' is not enabled on pocketbase");
        return error_redirect("oauth");
    };
    let redirect_uri = callback_url(&public_url(&state), &provider);
    let flow = OAuthFlow {
        provider: provider.clone(),
        state: found.state.clone(),
        verifier: found.code_verifier.clone(),
        next: safe_next(next.next.as_ref()),
    };
    let auth_url = with_redirect_uri(&found.auth_url, &redirect_uri);
    let mut resp = Redirect::to(&auth_url).into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        header_value(oauth_cookie(&flow, secure).to_string()),
    );
    resp
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct CallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// `GET /auth/oauth/{provider}/callback` — verifies the roundtrip, hands the
/// authorization code to PocketBase, and promotes the returned PB token into
/// the regular `brillante.session` cookie.
pub async fn callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> Response {
    let secure = state.pb_config.as_ref().is_some_and(|c| c.secure_cookies);
    let clear = header_value(cleared_oauth_cookie().to_string());
    let cleared = |resp: &mut Response| {
        resp.headers_mut().append(header::SET_COOKIE, clear.clone());
    };

    let flow = session::named_cookie(&headers, OAUTH_COOKIE)
        .as_deref()
        .and_then(decode_flow);
    let Some(flow) = flow else {
        let mut resp = error_redirect("oauth_state");
        cleared(&mut resp);
        return resp;
    };
    if flow.provider != provider {
        tracing::warn!("oauth callback: provider path '{}' does not match the started flow", provider);
        let mut resp = error_redirect("oauth_state");
        cleared(&mut resp);
        return resp;
    }
    if let Some(err) = query.error.as_deref() {
        tracing::info!("oauth callback: provider reported error '{err}' for '{provider}'");
        let mut resp = error_redirect("oauth");
        cleared(&mut resp);
        return resp;
    }
    if query.state.as_deref() != Some(flow.state.as_str()) {
        tracing::warn!("oauth callback: state mismatch for '{provider}'");
        let mut resp = error_redirect("oauth_state");
        cleared(&mut resp);
        return resp;
    }
    let Some(code) = query.code.as_deref().filter(|c| !c.is_empty()) else {
        let mut resp = error_redirect("oauth");
        cleared(&mut resp);
        return resp;
    };

    let Some(pb) = &state.pb else {
        let mut resp = error_redirect("disabled");
        cleared(&mut resp);
        return resp;
    };
    let redirect_uri = callback_url(&public_url(&state), &provider);
    match pb
        .auth_with_oauth2(&provider, code, &flow.verifier, &redirect_uri)
        .await
    {
        Ok((token, _user)) => {
            let mut resp = Redirect::to(&flow.next).into_response();
            resp.headers_mut().append(
                header::SET_COOKIE,
                header_value(session::session_cookie(&token, secure).to_string()),
            );
            cleared(&mut resp);
            resp
        }
        Err(PbError::Rejected(m)) => {
            tracing::info!("oauth callback rejected ({provider}): {m}");
            let mut resp = error_redirect("oauth");
            cleared(&mut resp);
            resp
        }
        Err(other) => {
            tracing::warn!("oauth callback failed ({provider}): {other}");
            let mut resp = error_redirect("unavailable");
            cleared(&mut resp);
            resp
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_cookie_roundtrips() {
        let flow = OAuthFlow {
            provider: "oidc".into(),
            state: "pb_state_123".into(),
            verifier: "a_verifier_value".into(),
            next: "/course/crlb".into(),
        };
        let cookie = oauth_cookie(&flow, false);
        assert_eq!(cookie.name(), OAUTH_COOKIE);
        assert!(cookie.http_only().unwrap());
        let decoded = decode_flow(cookie.value()).unwrap();
        assert_eq!(decoded, flow);
    }

    #[test]
    fn decode_flow_rejects_garbage() {
        assert!(decode_flow("not-base64!!").is_none());
        assert!(decode_flow("aGVsbG8=" /* "hello" */).is_none());
    }

    #[test]
    fn redirect_uri_appends_to_pb_placeholder() {
        let auth_url = "https://tenant.auth0.com/authorize?connection=google-oauth2&state=s&redirect_uri=";
        assert_eq!(
            with_redirect_uri(auth_url, "http://127.0.0.1:3000/auth/oauth/oidc/callback"),
            "https://tenant.auth0.com/authorize?connection=google-oauth2&state=s&redirect_uri=http%3A%2F%2F127.0.0.1%3A3000%2Fauth%2Foauth%2Foidc%2Fcallback"
        );
    }

    #[test]
    fn redirect_uri_added_when_placeholder_missing() {
        assert_eq!(
            with_redirect_uri("https://idp.example/authorize?state=s", "https://x/y"),
            "https://idp.example/authorize?state=s&redirect_uri=https%3A%2F%2Fx%2Fy"
        );
    }

    #[test]
    fn callback_url_joins_base_and_provider() {
        assert_eq!(
            callback_url("http://127.0.0.1:3000/", "oidc2"),
            "http://127.0.0.1:3000/auth/oauth/oidc2/callback"
        );
        assert_eq!(
            callback_url("https://courses.example.com", "oidc"),
            "https://courses.example.com/auth/oauth/oidc/callback"
        );
    }

    #[test]
    fn start_href_encodes_next() {
        assert_eq!(start_href("oidc", "/"), "/auth/oauth/oidc");
        assert_eq!(
            start_href("oidc", "/course/crlb/intro"),
            "/auth/oauth/oidc?next=%2Fcourse%2Fcrlb%2Fintro"
        );
    }
}
