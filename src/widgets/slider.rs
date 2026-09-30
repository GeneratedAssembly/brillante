use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

fn decimals_of(step: f64) -> usize {
    let s = format!("{step}");
    s.split('.').nth(1).map(|d| d.len()).unwrap_or(0)
}

pub fn format_value(value: f64, decimals: usize, unit: Option<&str>, template: Option<&str>) -> String {
    let formatted = if decimals == 0 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.decimals$}")
    };
    match template {
        Some(t) => t.replace("{value}", &formatted),
        None => match unit {
            Some(u) => format!("{formatted} {u}"),
            None => formatted,
        },
    }
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let label = w.opt_str("label")?.unwrap_or_else(|| "Value".into());
    let min = w.req_f64("min")?;
    let max = w.req_f64("max")?;
    if !min.is_finite() || !max.is_finite() {
        return Err(WidgetError::Invalid(
            "'min' and 'max' must be finite numbers".into(),
        ));
    }
    if max <= min {
        return Err(WidgetError::Invalid(
            "'max' must be greater than 'min'".into(),
        ));
    }
    let step = w.opt_f64("step")?.unwrap_or(1.0);
    if step <= 0.0 {
        return Err(WidgetError::Invalid("'step' must be positive".into()));
    }
    let initial = w.opt_f64("initial")?.unwrap_or(min).clamp(min, max);
    let unit = w.opt_str("unit")?;
    let display = w.opt_str("display")?;
    let decimals = decimals_of(step);
    let value_text = format_value(initial, decimals, unit.as_deref(), display.as_deref());

    let inner = view! {
        <div class="slider-head">
            <span class="slider-label">{label.clone()}</span>
            <output class="slider-value">{value_text}</output>
        </div>
        <input
            type="range"
            class="slider-input"
            min={min.to_string()}
            max={max.to_string()}
            step={step.to_string()}
            value={initial.to_string()}
            aria-label={label.clone()}
        />
    };

    Ok(widget_shell("slider", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}

#[cfg(test)]
mod tests {
    use super::format_value;

    #[test]
    fn formats_integers_and_units() {
        assert_eq!(format_value(180.0, 0, Some("°C"), None), "180 °C");
        assert_eq!(format_value(42.0, 0, None, None), "42");
    }

    #[test]
    fn formats_decimals() {
        assert_eq!(format_value(2.5, 1, None, None), "2.5");
        assert_eq!(format_value(2.0, 2, None, None), "2.00");
    }

    #[test]
    fn applies_display_template() {
        assert_eq!(
            format_value(2.5, 1, Some("mol/s"), Some("{value} mol/s — {value}")),
            "2.5 mol/s — 2.5"
        );
    }
}
