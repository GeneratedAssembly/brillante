use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let statement = w.req_str("statement")?;
    let button_label = w
        .opt_str("button_label")?
        .unwrap_or_else(|| "I acknowledge".to_string());
    let done_message = w
        .opt_str("done_message")?
        .unwrap_or_else(|| "Acknowledged".to_string());

    let inner = view! {
        <div class="ack-inner">
            <p class="ack-statement">{statement}</p>
            <label class="ack-row">
                <input type="checkbox" class="ack-check" />
                <span class="ack-check-label">"I have read this."</span>
            </label>
            <button type="button" class="ack-button button button-primary" disabled>
                {button_label}
            </button>
            <p class="ack-done" hidden>{done_message}</p>
        </div>
    };

    Ok(widget_shell(
        "acknowledge",
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
            kind: "acknowledge".into(),
            id: "t.ack".into(),
            config,
        }
    }

    fn html(config: Option<serde_json::Value>) -> String {
        crate::render::render_html(move || render(&instance(config)).unwrap())
    }

    #[test]
    fn renders_statement_and_defaults() {
        let html = html(Some(json!({ "statement": "Be nice." })));
        assert!(html.contains("Be nice."));
        assert!(html.contains("I acknowledge"));
        assert!(html.contains(r#"data-widget-type="acknowledge""#));
        assert!(html.contains(r#"data-widget-id="t.ack""#));
    }

    #[test]
    fn custom_labels_render() {
        let html = html(Some(json!({
            "statement": "s",
            "button_label": "Agreed!",
            "done_message": "Thanks!"
        })));
        assert!(html.contains("Agreed!"));
    }

    #[test]
    fn missing_statement_is_invalid() {
        let result = render(&instance(Some(json!({}))));
        assert!(matches!(result, Err(WidgetError::Invalid(msg)) if msg.contains("statement")));
        assert!(render(&instance(None)).is_err());
    }

    #[test]
    fn non_string_statement_is_invalid() {
        assert!(render(&instance(Some(json!({ "statement": 42 })))).is_err());
    }
}