use leptos::prelude::*;

use crate::widgets::assignment::{required_nonempty, segment_views, split_math_segments};
use crate::widgets::{WidgetError, WidgetInstance, widget_shell};

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let prompt = required_nonempty(w, "prompt")?;
    // Validated here so a malformed key is a visible error card at authoring
    // time; the value itself is NEVER rendered or serialized into the page
    // (it only leaves the server inside a graded attempt's response, where it
    // doubles as the key-points panel).
    let _key_points = w.opt_str("key_points")?.filter(|k| !k.trim().is_empty());
    let hint = w.opt_str("hint")?.filter(|h| !h.trim().is_empty());
    let label = w
        .opt_str("label")?
        .filter(|l| !l.trim().is_empty())
        .unwrap_or_else(|| "Be the teacher".to_string());
    let button_label = w
        .opt_str("button_label")?
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_else(|| "Submit for grading".to_string());
    let placeholder = w
        .opt_str("placeholder")?
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| {
            "Explain it as if you were teaching the class. Use $...$ for inline math and $$...$$ for display math."
                .to_string()
        });

    let label_view: AnyView = view! { <span class="teach-label">{label}</span> }.into_any();
    let hint_view: AnyView = match hint {
        Some(h) => {
            let body = segment_views(&split_math_segments(&h));
            view! {
                <details class="teach-hint">
                    <summary>"Hint"</summary>
                    <div class="teach-hint-body">{body}</div>
                </details>
            }
            .into_any()
        }
        None => view! { <span hidden></span> }.into_any(),
    };
    let prompt_views = segment_views(&split_math_segments(&prompt));

    let inner = view! {
        <div class="teach-head">
            {label_view}
            <span class="teach-attempts" data-teach-attempts hidden></span>
        </div>
        <div class="teach-prompt">{prompt_views}</div>
        {hint_view}
        <div class="teach-editor">
            <textarea
                class="teach-input"
                data-teach-input
                rows="8"
                maxlength="8000"
                placeholder={placeholder}
                spellcheck="false"
            ></textarea>
            <div class="teach-editor-bar">
                <span class="teach-count" data-teach-count>"0 / 8000"</span>
                <button
                    type="button"
                    class="teach-btn teach-preview-toggle"
                    data-teach-preview
                >
                    "Preview"
                </button>
                <button
                    type="button"
                    class="teach-btn teach-submit"
                    data-teach-submit
                >
                    {button_label}
                </button>
            </div>
        </div>
        <div class="teach-preview-out note-markdown" data-teach-preview-out hidden></div>
        <div class="teach-result" data-teach-result hidden>
            <span class="teach-verdict" data-teach-verdict></span>
            <div class="teach-feedback note-markdown" data-teach-feedback></div>
        </div>
        <div class="teach-key-points">
            <button
                type="button"
                class="teach-btn teach-key-points-toggle"
                data-teach-key-points-toggle
                disabled
            >
                "Show key points"
            </button>
            <div
                class="teach-key-points-body note-markdown"
                data-teach-key-points-body
                hidden
            ></div>
        </div>
        <p class="teach-error" data-teach-error hidden></p>
    };

    // Deliberately no data-config: the config may carry the hidden key points,
    // which must never appear in the DOM. The JS reads everything it needs
    // from data-widget-id and the body's dataset.
    Ok(widget_shell("teach", &w.id, None, inner.into_any()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "teach".into(),
            id: "t.w".into(),
            config,
        }
    }

    #[test]
    fn prompt_is_required() {
        assert!(render(&instance(Some(json!({})))).is_err());
        assert!(render(&instance(Some(json!({ "prompt": "  " })))).is_err());
        assert!(render(&instance(Some(json!({ "prompt": "p" })))).is_ok());
    }

    #[test]
    fn key_points_is_optional_but_validated() {
        assert!(render(&instance(Some(json!({ "prompt": "p" })))).is_ok());
        assert!(
            render(&instance(Some(json!({ "prompt": "p", "key_points": 4 })))).is_err(),
            "non-string key_points is an invalid config"
        );
        assert!(render(&instance(Some(json!({ "prompt": "p", "key_points": "k" })))).is_ok());
    }

    #[test]
    fn page_html_never_contains_the_key_points() {
        let w = instance(Some(json!({
            "prompt": "Explain what $\\bar{x}$ estimates.",
            "key_points": "SECRET: the expectation of $\\bar{x}$ is $\\theta$",
            "hint": "Start from the likelihood.",
        })));
        let html = crate::render::render_html(move || render(&w).unwrap());
        assert!(!html.contains("SECRET"), "{html}");
        assert!(!html.contains("the expectation of"), "{html}");
        assert!(!html.contains("data-config="), "{html}");
        assert!(html.contains(r#"data-tex="\bar{x}""#), "{html}");
        assert!(html.contains("teach-hint"), "{html}");
        assert!(html.contains("Be the teacher"), "{html}");
    }
}
