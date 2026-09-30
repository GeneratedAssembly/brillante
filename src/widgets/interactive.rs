use std::collections::BTreeMap;

use leptos::prelude::*;
use serde_json::{json, Value};

use crate::widgets::expr;
use crate::widgets::template;
use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

fn parse_domain(w: &WidgetInstance, key: &str) -> Result<(f64, f64), WidgetError> {
    let arr = w.req_arr(key)?;
    if arr.len() != 2 {
        return Err(WidgetError::Invalid(format!(
            "'{key}' must be a [min, max] pair"
        )));
    }
    let min = arr[0]
        .as_f64()
        .ok_or_else(|| WidgetError::Invalid(format!("{key}[0] must be a number")))?;
    let max = arr[1]
        .as_f64()
        .ok_or_else(|| WidgetError::Invalid(format!("{key}[1] must be a number")))?;
    if !min.is_finite() || !max.is_finite() || max <= min {
        return Err(WidgetError::Invalid(format!(
            "'{key}' must satisfy min < max, both finite"
        )));
    }
    Ok((min, max))
}

struct Param {
    name: String,
    label: String,
    min: f64,
    max: f64,
    step: f64,
    initial: f64,
    decimals: usize,
}

fn decimals_of(step: f64) -> usize {
    let s = format!("{step}");
    s.split('.').nth(1).map(|d| d.len()).unwrap_or(0)
}

/// A computed step (range/100) lands on values like 0.0030000000000000006;
/// rounding to 9 decimal places keeps the slider's displayed values sane.
fn clean_step(step: f64) -> f64 {
    (step * 1e9).round() / 1e9
}

fn parse_params(w: &WidgetInstance) -> Result<Vec<Param>, WidgetError> {
    let raw = w.opt_arr("parameters")?.unwrap_or_default();
    let mut params = Vec::with_capacity(raw.len());
    for (i, entry) in raw.iter().enumerate() {
        let obj = entry
            .as_object()
            .ok_or_else(|| WidgetError::Invalid(format!("parameters[{i}] must be an object")))?;
        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                WidgetError::Invalid(format!("parameters[{i}].name must be a string"))
            })?
            .to_string();
        let label = obj
            .get("label")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| name.clone());
        let val = |key: &str| -> Result<f64, WidgetError> {
            obj.get(key)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| {
                    WidgetError::Invalid(format!("parameters[{i}].{key} must be a number"))
                })
        };
        let min = val("min")?;
        let max = val("max")?;
        if !min.is_finite() || !max.is_finite() || max <= min {
            return Err(WidgetError::Invalid(format!(
                "parameters[{i}]: 'min' < 'max' required"
            )));
        }
        let step = clean_step(match obj.get("step") {
            None | Some(Value::Null) => (max - min) / 100.0,
            Some(v) => v
                .as_f64()
                .filter(|s| *s > 0.0)
                .ok_or_else(|| {
                    WidgetError::Invalid(format!("parameters[{i}].step must be positive"))
                })?,
        });
        let initial = match obj.get("initial") {
            None | Some(Value::Null) => min + (max - min) / 2.0,
            Some(v) => v
                .as_f64()
                .map(|x| x.clamp(min, max))
                .ok_or_else(|| {
                    WidgetError::Invalid(format!("parameters[{i}].initial must be a number"))
                })?,
        };
        params.push(Param {
            name,
            label,
            min,
            max,
            step,
            initial,
            decimals: decimals_of(step),
        });
    }
    Ok(params)
}

fn readout_references_xy(template_text: &str) -> bool {
    template::placeholders(template_text)
        .iter()
        .any(|ph| ph.name == "x" || ph.name == "y")
}

/// Initial readout text: placeholders resolved against the environment of
/// parameter initials plus the probe point `x`; `y` is the expression value
/// there. Mirrors `formatReadout` in interactive.js.
fn initial_readout(
    template_text: &str,
    ast: &Value,
    env: &BTreeMap<String, f64>,
) -> String {
    template::render(template_text, &mut |name, dec| {
        let v = if name == "y" {
            expr::eval_ast(ast, env)
        } else {
            env.get(name).copied().unwrap_or(f64::NAN)
        };
        let dec = dec.unwrap_or(3);
        Some(if v.is_finite() {
            format!("{v:.dec$}")
        } else {
            "—".to_string()
        })
    })
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let title = w.opt_str("title")?;
    let xlabel = w.opt_str("xlabel")?;
    let ylabel = w.opt_str("ylabel")?;
    let readout = w.opt_str("readout")?;
    let (xmin, xmax) = parse_domain(w, "domain")?;
    let params = parse_params(w)?;
    let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    let expression = w.req_str("expression")?;
    let ast = expr::parse(&expression, &param_names)?;
    let samples = w.opt_f64("samples")?.unwrap_or(240.0).clamp(16.0, 2000.0) as u32;
    let ydomain: Option<Vec<Value>> = if w.cfg()?.get("ydomain").is_some() {
        let (ymin, ymax) = parse_domain(w, "ydomain")?;
        Some(vec![json!(ymin), json!(ymax)])
    } else {
        None
    };

    if let Some(t) = &readout {
        for ph in template::placeholders(t) {
            let shown = format!("{{{}}}", ph.name);
            let allowed =
                ph.name == "x" || ph.name == "y" || params.iter().any(|p| p.name == ph.name);
            if !allowed {
                return Err(WidgetError::Invalid(format!(
                    "'readout' placeholder {shown} is not plottable; allowed: x, y, or a parameter name"
                )));
            }
            if let Some(spec) = ph.spec && template::spec_decimals(spec).is_none() {
                return Err(WidgetError::Invalid(format!(
                    "'readout' format spec '{spec}' in placeholder {shown} has no usable precision; write e.g. '{{x:0.2f}}'"
                )));
            }
        }
    }

    // An x slider keeps the independent variable adjustable so `{x}`/`{y}`
    // readouts can react to something; it defaults on when the readout needs it.
    let xslider = match w.cfg()?.get("xslider") {
        None | Some(Value::Null) => readout.as_deref().is_some_and(readout_references_xy),
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(WidgetError::Invalid("'xslider' must be true or false".into())),
    };
    let xparam = xslider.then(|| {
        let step = clean_step((xmax - xmin) / 100.0);
        Param {
            name: "x".into(),
            label: xlabel.clone().unwrap_or_else(|| "x".into()),
            min: xmin,
            max: xmax,
            step,
            initial: xmin + (xmax - xmin) / 2.0,
            decimals: decimals_of(step),
        }
    });

    let initial_x = xparam
        .as_ref()
        .map(|p| p.initial)
        .unwrap_or_else(|| xmin + (xmax - xmin) / 2.0);
    let mut env: BTreeMap<String, f64> = params
        .iter()
        .map(|p| (p.name.clone(), p.initial))
        .collect();
    env.insert("x".to_string(), initial_x);

    // Client config: the parsed AST replaces the source expression.
    let mut client_config = w.cfg()?.clone();
    if let Some(obj) = client_config.as_object_mut() {
        obj.insert("ast".into(), ast.clone());
        obj.insert("domain".into(), json!([xmin, xmax]));
        obj.insert("samples".into(), json!(samples));
        obj.insert("xslider".into(), json!(xslider));
        if let Some(yd) = ydomain {
            obj.insert("ydomain".into(), json!(yd));
        }
        obj.remove("expression");
    }

    let mut param_views: Vec<AnyView> = Vec::new();
    for p in xparam.iter().chain(params.iter()) {
        param_views.push(
            view! {
                <div class="interactive-param">
                    <div class="interactive-param-head">
                        <label class="interactive-param-label" for={format!("{}-{}", w.id, p.name)}>
                            {p.label.clone()}
                        </label>
                        <output class="interactive-param-value" data-param={p.name.clone()}>
                            {format!("{:.*}", p.decimals, p.initial)}
                        </output>
                    </div>
                    <input
                        type="range"
                        class="interactive-param-input"
                        id={format!("{}-{}", w.id, p.name)}
                        data-param={p.name.clone()}
                        min={p.min.to_string()}
                        max={p.max.to_string()}
                        step={p.step.to_string()}
                        value={p.initial.to_string()}
                        aria-label={p.label.clone()}
                    />
                </div>
            }
            .into_any(),
        );
    }

    let title_view: AnyView = match title {
        Some(t) => view! { <div class="interactive-title">{t}</div> }.into_any(),
        None => view! { <div class="interactive-title" hidden></div> }.into_any(),
    };
    let xlabel_view: AnyView = match xlabel {
        Some(x) => view! { <div class="interactive-xlabel">{x}</div> }.into_any(),
        None => view! { <div class="interactive-xlabel" hidden></div> }.into_any(),
    };
    let ylabel_view: AnyView = match ylabel {
        Some(y) => view! { <div class="interactive-ylabel">{y}</div> }.into_any(),
        None => view! { <div class="interactive-ylabel" hidden></div> }.into_any(),
    };
    let readout_view: AnyView = match &readout {
        Some(t) => {
            view! { <div class="interactive-readout">{initial_readout(t, &ast, &env)}</div> }
                .into_any()
        }
        None => view! { <div class="interactive-readout" hidden></div> }.into_any(),
    };

    let inner = view! {
        {title_view}
        <div class="interactive-columns">
            <div class="interactive-plot">
                <div class="interactive-body">
                    {ylabel_view}
                    <canvas class="interactive-canvas" width="640" height="360"></canvas>
                    {xlabel_view}
                </div>
            </div>
            <div class="interactive-controls">{param_views}</div>
        </div>
        {readout_view}
    };

    Ok(widget_shell(
        "interactive",
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
            kind: "interactive".into(),
            id: "t.i".into(),
            config,
        }
    }

    fn html(config: Option<serde_json::Value>) -> String {
        crate::render::render_html(move || render(&instance(config)).unwrap())
    }

    fn base_config() -> serde_json::Value {
        json!({
            "expression": "1.50 + 0.0045 / (x * x)",
            "domain": [0.4, 0.7],
            "xlabel": "wavelength λ (μm)",
            "readout": "n({x:0.2f} μm) = {y:0.4f}"
        })
    }

    #[test]
    fn readout_placeholders_render_initial_values() {
        let html = html(Some(base_config()));
        assert!(html.contains("n(0.55 μm) = 1.5149"), "{html}");
    }

    #[test]
    fn readout_referencing_xy_adds_an_x_slider() {
        let html = html(Some(base_config()));
        assert!(
            html.contains(r#"data-param="x""#),
            "readout referencing x/y must render an x slider: {html}"
        );
    }

    #[test]
    fn xslider_can_be_forced_off() {
        let mut cfg = base_config();
        cfg["xslider"] = json!(false);
        let html = html(Some(cfg));
        assert!(!html.contains(r#"data-param="x""#), "{html}");
        // the readout still evaluates at the domain midpoint
        assert!(html.contains("n(0.55 μm) = 1.5149"), "{html}");
    }

    #[test]
    fn xslider_explicit_on_without_readout() {
        let mut cfg = json!({
            "expression": "x^2",
            "domain": [0.0, 10.0],
            "xslider": true
        });
        cfg.as_object_mut().unwrap().remove("readout");
        let html = html(Some(cfg));
        assert!(html.contains(r#"data-param="x""#), "{html}");
    }

    #[test]
    fn parameters_render_and_substitute_into_readout() {
        let cfg = json!({
            "expression": "y0 + th * x",
            "domain": [0, 40],
            "parameters": [
                { "name": "y0", "min": -5, "max": 5, "step": 0.1, "initial": 2 },
                { "name": "th", "min": -0.2, "max": 0.2, "step": 0.005, "initial": 0.1 }
            ],
            "readout": "y({x:0.1f} cm) = {y0:0.2f} + {th:0.3f}·{x:0.1f} = {y:0.2f} cm",
            "xslider": true
        });
        let html = html(Some(cfg));
        assert!(html.contains("y(20.0 cm) = 2.00 + 0.100·20.0 = 4.00 cm"), "{html}");
        assert!(html.contains(r#"data-param="y0""#), "{html}");
        assert!(html.contains(r#"data-param="th""#), "{html}");
    }

    #[test]
    fn unknown_readout_placeholder_is_invalid() {
        let mut cfg = base_config();
        cfg["readout"] = json!("n = {nope}");
        assert!(matches!(
            render(&instance(Some(cfg))),
            Err(WidgetError::Invalid(msg)) if msg.contains("{nope}")
        ));
    }

    #[test]
    fn bad_readout_spec_is_invalid() {
        let mut cfg = base_config();
        cfg["readout"] = json!("n = {x:zz}");
        assert!(matches!(
            render(&instance(Some(cfg))),
            Err(WidgetError::Invalid(msg)) if msg.contains("precision")
        ));
    }

    #[test]
    fn non_bool_xslider_is_invalid() {
        let mut cfg = base_config();
        cfg["xslider"] = json!("yes");
        assert!(matches!(
            render(&instance(Some(cfg))),
            Err(WidgetError::Invalid(msg)) if msg.contains("'xslider' must be true or false")
        ));
    }

    #[test]
    fn client_config_drops_source_expression() {
        let html = html(Some(base_config()));
        assert!(html.contains("xslider"), "{html}");
        assert!(!html.contains("(x * x)"), "{html}");
        assert!(!html.contains("expression"), "{html}");
    }
}