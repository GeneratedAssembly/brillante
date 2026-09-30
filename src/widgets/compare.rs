use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

fn panel(w: &WidgetInstance, key: &str, fallback_title: &str, css_class: &str) -> Result<AnyView, WidgetError> {
    let title = w
        .opt_table(key)?
        .and_then(|t| t.get("title").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| fallback_title.to_string());
    let mut lines: Vec<String> = Vec::new();
    let mut note: Option<String> = None;
    if let Some(table) = w.opt_table(key)? {
        match table.get("tex") {
            Some(serde_json::Value::String(tex)) => lines.push(tex.clone()),
            Some(serde_json::Value::Array(arr)) => {
                for item in arr {
                    match item {
                        serde_json::Value::String(tex) => lines.push(tex.clone()),
                        _ => {
                            return Err(WidgetError::Invalid(format!(
                                "{key}.tex entries must all be strings"
                            )))
                        }
                    }
                }
            }
            Some(_) => {
                return Err(WidgetError::Invalid(format!(
                    "{key}.tex must be a string or an array of strings"
                )))
            }
            None => {
                return Err(WidgetError::Invalid(format!(
                    "missing required config key '{key}.tex'"
                )))
            }
        }
        note = table
            .get("note")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    let eqs: Vec<AnyView> = lines
        .iter()
        .map(|tex| view! { <div class="compare-eq" data-tex={tex.clone()}></div> }.into_any())
        .collect();

    let note_view: AnyView = match note {
        Some(n) => view! { <div class="compare-note">{n}</div> }.into_any(),
        None => view! { <div class="compare-note" hidden></div> }.into_any(),
    };

    Ok(view! {
        <div class=css_class>
            <div class="compare-title">{title}</div>
            <div class="compare-eqs">{eqs}</div>
            {note_view}
        </div>
    }
    .into_any())
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let left = panel(w, "left", "Original", "compare-panel")?;
    let right = panel(w, "right", "Revised", "compare-panel compare-panel-right")?;

    let inner = view! {
        <div class="compare-cols">{left} {right}</div>
    };

    Ok(widget_shell("compare", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}
