use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let question = w.req_str("question")?;
    let options = w.req_arr("options")?;
    let answer = w.opt_usize("answer")?.unwrap_or(0);
    if options.is_empty() {
        return Err(WidgetError::Invalid("'options' must not be empty".into()));
    }
    if answer >= options.len() {
        return Err(WidgetError::Invalid(format!(
            "'answer' index {answer} is out of range for {} options",
            options.len()
        )));
    }
    let texts: Vec<String> = options
        .iter()
        .map(|o| match o {
            serde_json::Value::String(s) => Ok(s.clone()),
            _ => Err(WidgetError::Invalid(
                "'options' entries must all be strings".into(),
            )),
        })
        .collect::<Result<_, _>>()?;
    let explanation = w.opt_str("explanation")?.unwrap_or_default();

    let mut option_views: Vec<AnyView> = Vec::new();
    for (i, text) in texts.iter().enumerate() {
        let idx = (i as u32).to_string();
        let letter = char::from(b'A' + i as u8).to_string();
        option_views.push(
            view! {
                <button type="button" class="quiz-option" data-index=idx>
                    <span class="quiz-option-letter">{letter}</span>
                    <span class="quiz-option-text">{text.clone()}</span>
                </button>
            }
            .into_any(),
        );
    }

    let inner = view! {
        <div class="quiz-question">{question}</div>
        <div class="quiz-options">{option_views}</div>
        <div class="quiz-explanation" hidden>{explanation}</div>
    };

    Ok(widget_shell("quiz", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}
