use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

pub struct Definition {
    symbol: String,
    meaning: String,
    link: Option<String>,
}

fn parse_definitions(w: &WidgetInstance) -> Result<Vec<Definition>, WidgetError> {
    let Some(raw) = w.opt_arr("definitions")? else {
        return Ok(Vec::new());
    };
    if raw.is_empty() {
        return Err(WidgetError::Invalid(
            "'definitions' must not be empty when present".into(),
        ));
    }
    let mut defs = Vec::with_capacity(raw.len());
    for (i, entry) in raw.iter().enumerate() {
        let obj = entry.as_object().ok_or_else(|| {
            WidgetError::Invalid(format!("definitions[{i}] must be an object"))
        })?;
        let symbol = obj
            .get("symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| WidgetError::Invalid(format!("definitions[{i}].symbol must be a string")))?
            .to_string();
        let meaning = obj
            .get("meaning")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                WidgetError::Invalid(format!("definitions[{i}].meaning must be a string"))
            })?
            .to_string();
        let link = match obj.get("link") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) => {
                if !s.starts_with('/') && !s.starts_with('#') {
                    return Err(WidgetError::Invalid(format!(
                        "definitions[{i}].link must be an in-app path starting with '/' or '#'"
                    )));
                }
                Some(s.clone())
            }
            Some(_) => {
                return Err(WidgetError::Invalid(format!(
                    "definitions[{i}].link must be a string"
                )))
            }
        };
        if symbol.is_empty() || meaning.is_empty() {
            return Err(WidgetError::Invalid(format!(
                "definitions[{i}]: 'symbol' and 'meaning' must be non-empty"
            )));
        }
        defs.push(Definition {
            symbol,
            meaning,
            link,
        });
    }
    Ok(defs)
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let tex = w.req_str("tex")?;
    let label = w.opt_str("label")?;
    let definitions = parse_definitions(w)?;

    let label_view: AnyView = match label {
        Some(l) => view! { <div class="math-label">{l}</div> }.into_any(),
        None => view! { <div class="math-label" hidden></div> }.into_any(),
    };

    let mut items: Vec<AnyView> = Vec::new();
    for d in &definitions {
        let meaning_view: AnyView = match &d.link {
            Some(href) => view! {
                <span class="math-def-meaning">
                    {d.meaning.clone()}
                    <a class="math-def-link" href={href.clone()}>" ↦ where defined"</a>
                </span>
            }
            .into_any(),
            None => view! { <span class="math-def-meaning">{d.meaning.clone()}</span> }.into_any(),
        };
        items.push(
            view! {
                <div class="math-def-row">
                    <span class="math-def-symbol">{d.symbol.clone()}</span>
                    {meaning_view}
                </div>
            }
            .into_any(),
        );
    }
    let defs_view: AnyView = if items.is_empty() {
        view! { <div></div> }.into_any()
    } else {
        view! {
            <details class="math-defs">
                <summary>"Definitions"</summary>
                <div class="math-defs-body">{items}</div>
            </details>
        }
        .into_any()
    };

    let inner = view! {
        {label_view}
        <div class="math-display" data-tex={tex}></div>
        {defs_view}
    };

    Ok(widget_shell("math", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "math".into(),
            id: "t.w".into(),
            config,
        }
    }

    #[test]
    fn no_definitions_is_accepted() {
        let w = instance(Some(json!({ "tex": "x" })));
        assert!(parse_definitions(&w).unwrap().is_empty());
        let w = instance(Some(json!({ "tex": "x", "definitions": null })));
        assert!(parse_definitions(&w).unwrap().is_empty());
    }

    #[test]
    fn definitions_parse_with_optional_link() {
        let w = instance(Some(json!({
            "tex": "x",
            "definitions": [
                { "symbol": "I(θ)", "meaning": "Fisher information", "link": "/crlb/03#fisher" },
                { "symbol": "V", "meaning": "the score" }
            ]
        })));
        let defs = parse_definitions(&w).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].link.as_deref(), Some("/crlb/03#fisher"));
        assert!(defs[1].link.is_none());
    }

    #[test]
    fn definitions_reject_bad_entries() {
        let w = instance(Some(json!({ "tex": "x", "definitions": [] })));
        assert!(parse_definitions(&w).is_err());
        let w = instance(Some(json!({
            "tex": "x",
            "definitions": [{ "symbol": "s", "meaning": "m", "link": "https://evil.example" }]
        })));
        assert!(parse_definitions(&w).is_err());
        let w = instance(Some(json!({
            "tex": "x",
            "definitions": [{ "symbol": "s" }]
        })));
        assert!(parse_definitions(&w).is_err());
    }
}
