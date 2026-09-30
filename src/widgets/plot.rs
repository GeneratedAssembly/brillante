use serde_json::Value;
use leptos::prelude::*;

use crate::widgets::{serialize_config, widget_shell, WidgetError, WidgetInstance};

fn is_hex_color(s: &str) -> bool {
    let body = s.strip_prefix('#').unwrap_or(s);
    (3..=8).contains(&body.len()) && body.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_points(raw: &Value, index: usize) -> Result<Vec<(f64, f64)>, WidgetError> {
    let arr = raw
        .as_array()
        .ok_or_else(|| WidgetError::Invalid(format!("series {index}: 'points' must be an array")))?;
    let mut points = Vec::with_capacity(arr.len());
    for p in arr {
        let pair = p.as_array().ok_or_else(|| {
            WidgetError::Invalid(format!("series {index}: each point must be [x, y]"))
        })?;
        if pair.len() != 2 {
            return Err(WidgetError::Invalid(format!(
                "series {index}: each point must have exactly two numbers"
            )));
        }
        let x = pair[0].as_f64().ok_or_else(|| {
            WidgetError::Invalid(format!("series {index}: point x must be a number"))
        })?;
        let y = pair[1].as_f64().ok_or_else(|| {
            WidgetError::Invalid(format!("series {index}: point y must be a number"))
        })?;
        points.push((x, y));
    }
    Ok(points)
}

type Series = (String, String, Vec<(f64, f64)>);

fn parse_series(w: &WidgetInstance) -> Result<Vec<Series>, WidgetError> {
    const PALETTE: [&str; 5] = ["#6366f1", "#10b981", "#f59e0b", "#ef4444", "#06b6d4"];
    let cfg = w.cfg()?;
    let series_raw = if let Some(points) = cfg.get("points") {
        vec![serde_json::json!({ "points": points })]
    } else {
        w.req_arr("series")?
    };
    let mut series = Vec::with_capacity(series_raw.len());
    for (i, entry) in series_raw.iter().enumerate() {
        let name = entry
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("series {}", i + 1));
        let color = entry
            .get("color")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| PALETTE[i % PALETTE.len()].to_string());
        if !is_hex_color(&color) {
            return Err(WidgetError::Invalid(format!(
                "series {index}: color '{color}' is not a hex color",
                index = i
            )));
        }
        let points_raw = entry.get("points").ok_or_else(|| {
            WidgetError::Invalid(format!("series {}: missing 'points'", i))
        })?;
        let points = parse_points(points_raw, i)?;
        if points.is_empty() {
            return Err(WidgetError::Invalid(format!(
                "series {}: 'points' must not be empty",
                i
            )));
        }
        series.push((name, color, points));
    }
    if series.is_empty() {
        return Err(WidgetError::Invalid(
            "config must define 'points' or a non-empty 'series' array".into(),
        ));
    }
    Ok(series)
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let title = w.opt_str("title")?;
    let xlabel = w.opt_str("xlabel")?;
    let ylabel = w.opt_str("ylabel")?;
    let series = parse_series(w)?;

    let mut legend: Vec<AnyView> = Vec::new();
    for (name, color, _) in &series {
        legend.push(
            view! {
                <span class="plot-legend-item">
                    <span class="plot-swatch" style={format!("background: {color}")}></span>
                    {name.clone()}
                </span>
            }
            .into_any(),
        );
    }

    let title_view: AnyView = match title {
        Some(t) => view! { <div class="plot-title">{t}</div> }.into_any(),
        None => view! { <div class="plot-title" hidden></div> }.into_any(),
    };
    let xlabel_view: AnyView = match xlabel {
        Some(x) => view! { <div class="plot-xlabel">{x}</div> }.into_any(),
        None => view! { <div class="plot-xlabel" hidden></div> }.into_any(),
    };
    let ylabel_view: AnyView = match ylabel {
        Some(y) => view! { <div class="plot-ylabel">{y}</div> }.into_any(),
        None => view! { <div class="plot-ylabel" hidden></div> }.into_any(),
    };

    let inner = view! {
        {title_view}
        <div class="plot-body">
            {ylabel_view}
            <canvas class="plot-canvas" width="640" height="360"></canvas>
            {xlabel_view}
        </div>
        <div class="plot-legend">{legend}</div>
    };

    Ok(widget_shell("plot", &w.id, serialize_config(w.config.as_ref()), inner.into_any()))
}
