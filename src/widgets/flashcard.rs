use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let front = w.req_str("front")?;
    let back = w.req_str("back")?;

    let inner = view! {
        <div class="flashcard-inner">
            <div class="flashcard-face flashcard-front">{front}</div>
            <div class="flashcard-face flashcard-back">{back}</div>
        </div>
    };

    Ok(widget_shell("flashcard", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}
