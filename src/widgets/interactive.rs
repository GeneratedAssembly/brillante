use leptos::prelude::*;
use serde_json::{json, Value};

use crate::widgets::expr;
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
        let step = match obj.get("step") {
            None | Some(Value::Null) => (max - min) / 100.0,
            Some(v) => v
                .as_f64()
                .filter(|s| *s > 0.0)
                .ok_or_else(|| {
                    WidgetError::Invalid(format!("parameters[{i}].step must be positive"))
                })?,
        };
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

    // Client config: the parsed AST replaces the source expression.
    let mut client_config = w.cfg()?.clone();
    if let Some(obj) = client_config.as_object_mut() {
        obj.insert("ast".into(), ast);
        obj.insert("domain".into(), json!([xmin, xmax]));
        obj.insert("samples".into(), json!(samples));
        if let Some(yd) = ydomain {
            obj.insert("ydomain".into(), json!(yd));
        }
    }

    let mut param_views: Vec<AnyView> = Vec::new();
    for p in &params {
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
    let readout_view: AnyView = match readout {
        Some(t) => view! { <div class="interactive-readout">{t}</div> }.into_any(),
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