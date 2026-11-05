use leptos::prelude::*;

use crate::widgets::{WidgetError, WidgetInstance, widget_shell};

/// One piece of a prompt/hint string: prose renders as escaped text, math
/// renders as `data-tex` elements for `math.js` (`$…$` inline, `$$…$$`
/// display). Full markdown is deliberately not supported in these fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    Text(String),
    InlineMath(String),
    DisplayMath(String),
}

/// Split prose + `$…$`/`$$…$$` into text and math segments. `\$` is an
/// escaped literal dollar sign; unpaired or empty delimiters stay literal.
/// Byte scanning is UTF-8 safe: only ASCII `$`/`\` bytes are inspected and
/// every slice boundary lands on an ASCII character.
pub(crate) fn split_math_segments(src: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut text_start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            i += 2;
            continue;
        }
        if b[i] == b'$' {
            let display = i + 1 < b.len() && b[i + 1] == b'$';
            let open = if display { 2 } else { 1 };
            let body_start = i + open;
            // pulldown-cmark's inline-math rule: no whitespace right after
            // the opening delimiter (keeps "cost is $ 5" literal).
            if !display && body_start < b.len() && b[body_start].is_ascii_whitespace() {
                i += 1;
                continue;
            }
            let mut j = body_start;
            let mut close = None;
            while j < b.len() {
                if b[j] == b'\\' && j + 1 < b.len() {
                    j += 2;
                    continue;
                }
                if b[j] == b'$' {
                    if display && !(j + 1 < b.len() && b[j + 1] == b'$') {
                        j += 1;
                        continue;
                    }
                    // and no whitespace right before the closing delimiter.
                    if !display && j > body_start && b[j - 1].is_ascii_whitespace() {
                        j += 1;
                        continue;
                    }
                    close = Some(j);
                    break;
                }
                j += 1;
            }
            match close {
                Some(end) if !src[body_start..end].trim().is_empty() => {
                    if text_start < i {
                        out.push(Segment::Text(src[text_start..i].to_string()));
                    }
                    out.push(if display {
                        Segment::DisplayMath(src[body_start..end].to_string())
                    } else {
                        Segment::InlineMath(src[body_start..end].to_string())
                    });
                    i = end + open;
                    text_start = i;
                }
                _ => {
                    i += open;
                }
            }
            continue;
        }
        i += 1;
    }
    if text_start < src.len() {
        out.push(Segment::Text(src[text_start..].to_string()));
    }
    out
}

/// Render segments as escaped text spans and `data-tex` elements. Escaped
/// dollar signs collapse to a literal `$` in text output.
pub(crate) fn segment_views(segments: &[Segment]) -> Vec<AnyView> {
    segments
        .iter()
        .map(|seg| match seg {
            Segment::Text(t) => {
                let t = t.replace("\\$", "$");
                view! { <span class="assignment-text">{t}</span> }.into_any()
            }
            Segment::InlineMath(tex) => {
                view! { <span class="math-inline" data-tex={tex.clone()}></span> }.into_any()
            }
            Segment::DisplayMath(tex) => {
                view! { <div class="math-display" data-tex={tex.clone()}></div> }.into_any()
            }
        })
        .collect()
}

pub(crate) fn required_nonempty(w: &WidgetInstance, key: &str) -> Result<String, WidgetError> {
    let value = w.req_str(key)?;
    if value.trim().is_empty() {
        return Err(WidgetError::Invalid(format!(
            "config key '{key}' must not be empty"
        )));
    }
    Ok(value)
}

pub fn render(w: &WidgetInstance) -> Result<AnyView, WidgetError> {
    let prompt = required_nonempty(w, "prompt")?;
    // Validated here so a missing/empty reference is a visible error card at
    // authoring time; the value itself is NEVER rendered or serialized into
    // the page (it only leaves the server inside a graded attempt response).
    let _reference = required_nonempty(w, "reference")?;
    let hint = w.opt_str("hint")?.filter(|h| !h.trim().is_empty());
    let label = w.opt_str("label")?.filter(|l| !l.trim().is_empty());
    let button_label = w
        .opt_str("button_label")?
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_else(|| "Submit for grading".to_string());
    let placeholder = w
        .opt_str("placeholder")?
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| {
            "Write your solution in markdown. Use $...$ for inline math and $$...$$ for display math."
                .to_string()
        });

    let label_view: AnyView = match label {
        Some(l) => view! { <span class="assignment-label">{l}</span> }.into_any(),
        None => view! { <span class="assignment-label">"Assignment"</span> }.into_any(),
    };
    let hint_view: AnyView = match hint {
        Some(h) => {
            let body = segment_views(&split_math_segments(&h));
            view! {
                <details class="assignment-hint">
                    <summary>"Hint"</summary>
                    <div class="assignment-hint-body">{body}</div>
                </details>
            }
            .into_any()
        }
        None => view! { <span hidden></span> }.into_any(),
    };
    let prompt_views = segment_views(&split_math_segments(&prompt));

    let inner = view! {
        <div class="assignment-head">
            {label_view}
            <span class="assignment-attempts" data-assignment-attempts hidden></span>
        </div>
        <div class="assignment-prompt">{prompt_views}</div>
        {hint_view}
        <div class="assignment-editor">
            <textarea
                class="assignment-input"
                data-assignment-input
                rows="8"
                maxlength="8000"
                placeholder={placeholder}
                spellcheck="false"
            ></textarea>
            <div class="assignment-editor-bar">
                <span class="assignment-count" data-assignment-count>"0 / 8000"</span>
                <button
                    type="button"
                    class="assignment-btn assignment-preview-toggle"
                    data-assignment-preview
                >
                    "Preview"
                </button>
                <button
                    type="button"
                    class="assignment-btn assignment-submit"
                    data-assignment-submit
                >
                    {button_label}
                </button>
            </div>
        </div>
        <div class="assignment-preview-out note-markdown" data-assignment-preview-out hidden></div>
        <div class="assignment-result" data-assignment-result hidden>
            <span class="assignment-verdict" data-assignment-verdict></span>
            <div class="assignment-feedback note-markdown" data-assignment-feedback></div>
        </div>
        <div class="assignment-reference">
            <button
                type="button"
                class="assignment-btn assignment-reference-toggle"
                data-assignment-reference-toggle
                disabled
            >
                "Show reference solution"
            </button>
            <div
                class="assignment-reference-body note-markdown"
                data-assignment-reference-body
                hidden
            ></div>
        </div>
        <p class="assignment-error" data-assignment-error hidden></p>
    };

    // Deliberately no data-config: the assignment config carries the hidden
    // reference solution, which must never appear in the DOM. The JS reads
    // everything it needs from data-widget-id and the body's dataset.
    Ok(widget_shell("assignment", &w.id, None, inner.into_any()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn instance(config: Option<serde_json::Value>) -> WidgetInstance {
        WidgetInstance {
            kind: "assignment".into(),
            id: "t.w".into(),
            config,
        }
    }

    #[test]
    fn splits_text_inline_and_display_math() {
        let segs = split_math_segments(
            "Given $x \\sim \\mathcal{N}(\\theta, 1)$, find $\\hat\\theta$.\n\n$$\\hat\\theta = \\bar{x}$$\n\nDone.",
        );
        assert_eq!(
            segs,
            vec![
                Segment::Text("Given ".into()),
                Segment::InlineMath("x \\sim \\mathcal{N}(\\theta, 1)".into()),
                Segment::Text(", find ".into()),
                Segment::InlineMath("\\hat\\theta".into()),
                Segment::Text(".\n\n".into()),
                Segment::DisplayMath("\\hat\\theta = \\bar{x}".into()),
                Segment::Text("\n\nDone.".into()),
            ]
        );
    }

    #[test]
    fn unpaired_and_empty_delimiters_stay_literal() {
        assert_eq!(
            split_math_segments("cost is \\$5 flat, then $$$$ empty"),
            vec![Segment::Text("cost is \\$5 flat, then $$$$ empty".into())]
        );
        assert_eq!(
            split_math_segments("a $ b spaced $ c"),
            vec![Segment::Text("a $ b spaced $ c".into())]
        );
    }

    #[test]
    fn escaped_dollar_is_not_a_delimiter() {
        let segs = split_math_segments("price \\$10 with $x^2$ inline");
        assert_eq!(
            segs,
            vec![
                Segment::Text("price \\$10 with ".into()),
                Segment::InlineMath("x^2".into()),
                Segment::Text(" inline".into()),
            ]
        );
    }

    #[test]
    fn utf8_text_round_trips() {
        let segs = split_math_segments("θ ~ π·σ² with $e^{iπ}$ — done");
        let text: String = segs
            .iter()
            .filter_map(|s| match s {
                Segment::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(text.contains("θ ~ π·σ²"));
        assert!(text.contains("— done"));
    }

    #[test]
    fn prompt_and_reference_are_required() {
        assert!(render(&instance(Some(json!({ "reference": "r" })))).is_err());
        assert!(render(&instance(Some(json!({ "prompt": "p" })))).is_err());
        assert!(render(&instance(Some(json!({ "prompt": "  ", "reference": "r" })))).is_err());
        assert!(render(&instance(Some(json!({ "prompt": "p", "reference": " " })))).is_err());
        assert!(render(&instance(Some(json!({ "prompt": "p", "reference": "r" })))).is_ok());
    }

    #[test]
    fn page_html_never_contains_the_reference() {
        let w = instance(Some(json!({
            "prompt": "Find $\\hat\\theta$.",
            "reference": "SECRET: $\\hat\\theta = \\bar{x}$",
            "hint": "Start from the likelihood.",
            "label": "Kay I · A1",
        })));
        let html = crate::render::render_html(move || render(&w).unwrap());
        assert!(!html.contains("SECRET"), "{html}");
        assert!(!html.contains("bar{x}"), "{html}");
        assert!(!html.contains("data-config="), "{html}");
        assert!(html.contains(r#"data-tex="\hat\theta""#), "{html}");
        assert!(html.contains("assignment-hint"), "{html}");
        assert!(html.contains("Kay I · A1"), "{html}");
    }
}
