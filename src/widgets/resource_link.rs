use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

const KINDS: [(&str, &str); 5] = [
    ("video", "Video"),
    ("pdf", "PDF"),
    ("assignment", "Assignment"),
    ("exam", "Exam"),
    ("web", "Resource"),
];

const ICONS: [(&str, &str); 5] = [
    ("video", "▶"),
    ("pdf", "📄"),
    ("assignment", "✎"),
    ("exam", "✓"),
    ("web", "↗"),
];

fn kind_entry(kind: &str) -> (&'static str, &'static str) {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(k, l)| (*k, *l))
        .unwrap_or(("web", "Resource"))
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let title = w.req_str("title")?;
    let url = w.req_str("url")?;
    let source = w.req_str("source")?;
    let description = w.opt_str("description")?;
    let kind = match w.opt_str("kind")? {
        None => "web".to_string(),
        Some(k) => {
            if KINDS.iter().any(|(name, _)| *name == k) {
                k
            } else {
                let allowed = KINDS
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(WidgetError::Invalid(format!(
                    "'kind' must be one of: {allowed} (got '{k}')"
                )));
            }
        }
    };
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(WidgetError::Invalid(
            "'url' must be an absolute http(s) URL".into(),
        ));
    }

    let (kind_name, kind_label) = kind_entry(&kind);
    let icon = ICONS
        .iter()
        .find(|(k, _)| *k == kind_name)
        .map(|(_, i)| *i)
        .unwrap_or("↗");
    let meta = format!("{source} · {kind_label}");

    let desc_view: AnyView = match description {
        Some(d) => view! { <span class="resource-link-desc">{d}</span> }.into_any(),
        None => view! { <span class="resource-link-desc" hidden></span> }.into_any(),
    };

    let inner = view! {
        <a class="resource-link-card" href={url} target="_blank" rel="noopener">
            <span class="resource-link-icon" aria-hidden="true">{icon}</span>
            <span class="resource-link-text">
                <span class="resource-link-title">{title}</span>
                {desc_view}
                <span class="resource-link-meta">{meta}</span>
            </span>
        </a>
    };

    Ok(widget_shell(
        "resource-link",
        &w.id,
        serialize_config(w.config.as_ref()),
        inner.into_any(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "resource-link".into(),
            id: "t.res".into(),
            config,
        }
    }

    fn html(config: Option<serde_json::Value>) -> String {
        crate::render::render_html(move || render(&instance(config)).unwrap())
    }

    #[test]
    fn renders_card_with_defaults() {
        let html = html(Some(json!({
            "title": "MIT 2.71 video lectures",
            "url": "https://ocw.mit.edu/courses/2-71-optics-spring-2009/",
            "source": "MIT OpenCourseWare"
        })));
        assert!(html.contains("MIT 2.71 video lectures"));
        assert!(html.contains(r#"href="https://ocw.mit.edu/courses/2-71-optics-spring-2009/""#));
        assert!(html.contains(r#"target="_blank""#));
        assert!(html.contains(r#"rel="noopener""#));
        assert!(html.contains("Resource"));
        assert!(
            html.contains(r#"class="resource-link-desc"></span>"#),
            "description span must be empty and hidden when absent"
        );
    }

    #[test]
    fn kind_labels_and_icons() {
        let pdf = html(Some(json!({
            "title": "t",
            "url": "https://x.example/notes.pdf",
            "source": "s",
            "kind": "pdf"
        })));
        assert!(pdf.contains("PDF"));
        let video = html(Some(json!({
            "title": "t",
            "url": "https://x.example/v",
            "source": "s",
            "kind": "video"
        })));
        assert!(video.contains("Video"));
    }

    #[test]
    fn description_renders_when_present() {
        let html = html(Some(json!({
            "title": "t",
            "url": "https://x.example/",
            "source": "s",
            "description": "The full notes"
        })));
        assert!(html.contains("The full notes"));
    }

    #[test]
    fn rejects_bad_kind() {
        let result = render(&instance(Some(json!({
            "title": "t",
            "url": "https://x.example/",
            "source": "s",
            "kind": "torrent"
        }))));
        assert!(matches!(result, Err(WidgetError::Invalid(msg)) if msg.contains("kind")));
    }

    #[test]
    fn rejects_non_http_urls() {
        let result = render(&instance(Some(json!({
            "title": "t",
            "url": "javascript:alert(1)",
            "source": "s"
        }))));
        assert!(matches!(result, Err(WidgetError::Invalid(msg)) if msg.contains("http(s)")));
    }

    #[test]
    fn requires_all_keys() {
        assert!(render(&instance(Some(json!({ "title": "t" })))).is_err());
        assert!(render(&instance(Some(json!({ "url": "https://x" })))).is_err());
        assert!(render(&instance(Some(json!({
            "title": "t",
            "url": "https://x"
        }))))
        .is_err());
        assert!(render(&instance(None)).is_err());
    }
}
