use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::widgets::expr;
use crate::widgets::template;
use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

fn decimals_of(step: f64) -> usize {
    let s = format!("{step}");
    s.split('.').nth(1).map(|d| d.len()).unwrap_or(0)
}

fn format_fixed(value: f64, decimals: usize) -> String {
    if decimals == 0 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.decimals$}")
    }
}

fn display_text(
    display: Option<&str>,
    value: f64,
    decimals: usize,
    unit: Option<&str>,
    result: Option<f64>,
) -> String {
    match display {
        Some(t) => template::render(t, &mut |name, dec| match name {
            "value" => Some(format_fixed(value, dec.unwrap_or(decimals))),
            "result" => Some(match result {
                Some(r) if r.is_finite() => format_fixed(r, dec.unwrap_or(2)),
                _ => "—".to_string(),
            }),
            _ => None,
        }),
        None => {
            let formatted = format_fixed(value, decimals);
            match unit {
                Some(u) => format!("{formatted} {u}"),
                None => formatted,
            }
        }
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

    let readout = w.opt_str("readout")?;
    let ast = match &readout {
        Some(src) => Some(expr::parse(src, &[])?),
        None => None,
    };
    let mut uses_result = false;
    if let Some(t) = &display {
        for ph in template::placeholders(t) {
            let shown = format!("{{{}}}", ph.name);
            match ph.name {
                "value" | "result" => {}
                other => {
                    return Err(WidgetError::Invalid(format!(
                        "'display' placeholder '{{{other}}}' must be '{{value}}' or '{{result}}'"
                    )))
                }
            }
            if let Some(spec) = ph.spec && template::spec_decimals(spec).is_none() {
                return Err(WidgetError::Invalid(format!(
                    "'display' format spec '{spec}' in placeholder {shown} has no usable precision; write e.g. '{{value:0.2f}}'"
                )));
            }
            if ph.name == "result" {
                uses_result = true;
            }
        }
    }
    match (&readout, uses_result) {
        (None, true) => {
            return Err(WidgetError::Invalid(
                "'display' uses {result} but there is no 'readout' expression; add e.g. readout = \"(650/x)^4\""
                    .into(),
            ))
        }
        (Some(_), false) => {
            return Err(WidgetError::Invalid(
                "'readout' has no effect unless 'display' contains a {result} placeholder".into(),
            ))
        }
        _ => {}
    }

    let result = ast.as_ref().map(|a| {
        let mut env = BTreeMap::new();
        env.insert("x".to_string(), initial);
        expr::eval_ast(a, &env)
    });
    let value_text = display_text(display.as_deref(), initial, decimals_of(step), unit.as_deref(), result);

    let mut client_config = w.cfg()?.clone();
    if let Some(a) = &ast
        && let Some(obj) = client_config.as_object_mut()
    {
        obj.insert("readoutAst".into(), a.clone());
        obj.remove("readout");
    }

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

    Ok(widget_shell(
        "slider",
        &w.id,
        serialize_config(Some(&client_config)),
        inner.into_any(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "slider".into(),
            id: "t.s".into(),
            config,
        }
    }

    fn html(config: Option<serde_json::Value>) -> String {
        crate::render::render_html(move || render(&instance(config)).unwrap())
    }

    #[test]
    fn formats_integers_and_units() {
        assert_eq!(display_text(None, 180.0, 0, Some("°C"), None), "180 °C");
        assert_eq!(display_text(None, 42.0, 0, None, None), "42");
    }

    #[test]
    fn formats_decimals() {
        assert_eq!(display_text(None, 2.5, 1, None, None), "2.5");
        assert_eq!(display_text(None, 2.0, 2, None, None), "2.00");
    }

    #[test]
    fn applies_display_template() {
        assert_eq!(
            display_text(Some("{value} mol/s — {value}"), 2.5, 1, None, None),
            "2.5 mol/s — 2.5"
        );
    }

    #[test]
    fn display_templates_support_precision_specs() {
        assert_eq!(
            display_text(Some("sensitivity ∝ {value:.2} × (B/(B+1)²)"), 0.5, 1, None, None),
            "sensitivity ∝ 0.50 × (B/(B+1)²)"
        );
    }

    #[test]
    fn display_templates_substitute_computed_result() {
        assert_eq!(
            display_text(
                Some("{value} nm — {result:.1}× the scattering of 650 nm red"),
                450.0,
                0,
                None,
                Some((650.0f64 / 450.0).powi(4)),
            ),
            "450 nm — 4.4× the scattering of 650 nm red"
        );
        assert_eq!(
            display_text(Some("x = {value}, f(x) = {result}"), 3.0, 0, None, Some(f64::NAN)),
            "x = 3, f(x) = —"
        );
    }

    #[test]
    fn renders_value_and_result_from_config() {
        let html = html(Some(json!({
            "label": "Wavelength",
            "min": 380, "max": 750, "step": 5, "initial": 450,
            "display": "{value} nm — {result:.1}× vs 650 nm",
            "readout": "(650/x)^4"
        })));
        assert!(html.contains("450 nm — 4.4× vs 650 nm"), "{html}");
        assert!(html.contains(r#"data-config"#), "{html}");
        // The client receives the parsed AST, not the source expression.
        assert!(html.contains("readoutAst"), "{html}");
        assert!(!html.contains("(650/x)^4"), "{html}");
    }

    #[test]
    fn result_without_readout_is_invalid() {
        let config = json!({ "min": 0, "max": 10, "display": "{result}" });
        assert!(matches!(
            render(&instance(Some(config))),
            Err(WidgetError::Invalid(msg)) if msg.contains("no 'readout'")
        ));
    }

    #[test]
    fn readout_without_result_placeholder_is_invalid() {
        let config = json!({ "min": 0, "max": 10, "display": "{value} nm", "readout": "x^2" });
        assert!(matches!(
            render(&instance(Some(config))),
            Err(WidgetError::Invalid(msg)) if msg.contains("{result}")
        ));
    }

    #[test]
    fn unknown_display_placeholder_is_invalid() {
        let config = json!({ "min": 0, "max": 10, "display": "{nope}" });
        assert!(matches!(
            render(&instance(Some(config))),
            Err(WidgetError::Invalid(msg)) if msg.contains("{nope}")
        ));
    }

    #[test]
    fn bad_readout_expression_is_invalid() {
        let config = json!({
            "min": 0, "max": 10, "display": "{result}", "readout": "y + 1"
        });
        assert!(matches!(
            render(&instance(Some(config))),
            Err(WidgetError::Invalid(msg)) if msg.contains("unknown identifier")
        ));
    }
}