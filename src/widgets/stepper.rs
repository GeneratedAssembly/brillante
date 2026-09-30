use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

struct Step {
    title: Option<String>,
    tex: Option<String>,
    text: Option<String>,
    note: Option<String>,
}

fn parse_steps(w: &WidgetInstance) -> Result<Vec<Step>, WidgetError> {
    let raw = w.req_arr("steps")?;
    if raw.len() < 2 {
        return Err(WidgetError::Invalid(
            "'steps' must contain at least 2 entries".into(),
        ));
    }
    let mut steps = Vec::with_capacity(raw.len());
    for (i, entry) in raw.iter().enumerate() {
        let obj = entry
            .as_object()
            .ok_or_else(|| WidgetError::Invalid(format!("steps[{i}] must be an object")))?;
        let get_str = |key: &str| -> Result<Option<String>, WidgetError> {
            match obj.get(key) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(WidgetError::Invalid(format!(
                    "steps[{i}].{key} must be a string"
                ))),
            }
        };
        let step = Step {
            title: get_str("title")?,
            tex: get_str("tex")?,
            text: get_str("text")?,
            note: get_str("note")?,
        };
        if step.tex.is_none() && step.text.is_none() {
            return Err(WidgetError::Invalid(format!(
                "steps[{i}]: must have 'tex' or 'text'"
            )));
        }
        steps.push(step);
    }
    Ok(steps)
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let title = w.opt_str("label")?;
    let steps = parse_steps(w)?;

    let mut step_views: Vec<AnyView> = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        let title_view: AnyView = match &s.title {
            Some(t) => view! { <div class="step-title">{t.clone()}</div> }.into_any(),
            None => view! { <div class="step-title" hidden></div> }.into_any(),
        };
        let tex_view: AnyView = match &s.tex {
            Some(tex) => view! { <div class="step-tex" data-tex={tex.clone()}></div> }.into_any(),
            None => view! { <div class="step-tex" hidden></div> }.into_any(),
        };
        let text_view: AnyView = match &s.text {
            Some(t) => view! { <div class="step-text">{t.clone()}</div> }.into_any(),
            None => view! { <div class="step-text" hidden></div> }.into_any(),
        };
        let note_view: AnyView = match &s.note {
            Some(t) => view! { <div class="step-note">{t.clone()}</div> }.into_any(),
            None => view! { <div class="step-note" hidden></div> }.into_any(),
        };
        step_views.push(
            view! {
                <div class="stepper-step" data-index={(i as u32).to_string()}>
                    {title_view}
                    {tex_view}
                    {text_view}
                    {note_view}
                </div>
            }
            .into_any(),
        );
    }

    let title_view: AnyView = match title {
        Some(t) => view! { <div class="stepper-label">{t}</div> }.into_any(),
        None => view! { <div class="stepper-label" hidden></div> }.into_any(),
    };

    let inner = view! {
        {title_view}
        <div class="stepper-steps">{step_views}</div>
        <div class="stepper-controls">
            <button type="button" class="stepper-btn stepper-prev">"← Prev"</button>
            <span class="stepper-counter"></span>
            <button type="button" class="stepper-btn stepper-next">"Next →"</button>
        </div>
    };

    Ok(widget_shell("stepper", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "stepper".into(),
            id: "t.w".into(),
            config,
        }
    }

    #[test]
    fn requires_two_steps() {
        let w = instance(Some(json!({ "steps": [{ "text": "one" }] })));
        assert!(parse_steps(&w).is_err());
    }

    #[test]
    fn steps_need_tex_or_text() {
        let w = instance(Some(json!({
            "steps": [{ "title": "empty" }, { "text": "ok" }]
        })));
        assert!(parse_steps(&w).is_err());
        let w = instance(Some(json!({
            "steps": [{ "tex": "x = 1", "note": "n" }, { "text": "ok" }]
        })));
        assert!(parse_steps(&w).is_ok());
    }
}
