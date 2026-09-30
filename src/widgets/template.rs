//! Shared `{placeholder}` template helpers for widget readouts.
//!
//! A placeholder is `{name}` or `{name:spec}` where `spec` carries an optional
//! width and a decimal precision (`{x:0.2f}`, `{y:.3}`, `{d:1}` — the trailing
//! `f` is the Python-style format suffix and is ignored). Placeholders that a
//! renderer does not resolve stay literal in the output.

pub struct Placeholder<'a> {
    pub name: &'a str,
    pub spec: Option<&'a str>,
}

fn split_inner(inner: &str) -> (&str, Option<&str>) {
    match inner.split_once(':') {
        Some((name, spec)) => (name.trim(), Some(spec.trim())),
        None => (inner.trim(), None),
    }
}

/// List every placeholder in `template`, left to right.
pub fn placeholders(template: &str) -> Vec<Placeholder<'_>> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        let (name, spec) = split_inner(&rest[start + 1..start + end]);
        out.push(Placeholder { name, spec });
        rest = &rest[start + end + 1..];
    }
    out
}

/// Decimal places implied by a format spec: the digits after the last dot of
/// `"0.2f"`/`".2"`/`"2"`. `None` when the spec carries no usable precision.
pub fn spec_decimals(spec: &str) -> Option<usize> {
    let s = spec.strip_suffix('f').unwrap_or(spec);
    let digits = s.rsplit('.').next().unwrap_or("");
    if digits.is_empty() {
        return None;
    }
    digits.parse::<usize>().ok()
}

/// Rebuild `template`, replacing each placeholder with `resolve(name, decimals)`.
/// A `None` from `resolve` leaves that placeholder literal in the output.
pub fn render(
    template: &str,
    resolve: &mut dyn FnMut(&str, Option<usize>) -> Option<String>,
) -> String {
    let mut out = String::new();
    let mut rest = template;
    loop {
        match rest.find('{') {
            None => {
                out.push_str(rest);
                return out;
            }
            Some(start) => {
                let Some(end) = rest[start..].find('}') else {
                    out.push_str(rest);
                    return out;
                };
                out.push_str(&rest[..start]);
                let (name, spec) = split_inner(&rest[start + 1..start + end]);
                match resolve(name, spec.and_then(spec_decimals)) {
                    Some(text) => out.push_str(&text),
                    None => out.push_str(&rest[start..=start + end]),
                }
                rest = &rest[start + end + 1..];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_with(
        template: &str,
        table: &[(&str, f64)],
        defaults: &[(&str, usize)],
    ) -> String {
        render(template, &mut |name, dec| {
            let v = table
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)?;
            let dec = dec.unwrap_or_else(|| {
                defaults
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, d)| *d)
                    .unwrap_or(3)
            });
            Some(format!("{v:.dec$}"))
        })
    }

    #[test]
    fn substitutes_names_and_specs() {
        let table = [("x", 0.45), ("y", 1.52228)];
        assert_eq!(
            render_with("n({x:0.2f} μm) = {y:0.4f}", &table, &[]),
            "n(0.45 μm) = 1.5223"
        );
        assert_eq!(render_with("{x:.1}→{x}", &table, &[("x", 3)]), "0.5→0.450");
    }

    #[test]
    fn unknown_placeholders_stay_literal() {
        let table = [("x", 2.0)];
        assert_eq!(render_with("a {x} b {nope} c", &table, &[]), "a 2.000 b {nope} c");
        assert_eq!(render_with("dangling {brace", &table, &[]), "dangling {brace");
    }

    #[test]
    fn spec_decimals_rules() {
        assert_eq!(spec_decimals("0.2f"), Some(2));
        assert_eq!(spec_decimals("0.0f"), Some(0));
        assert_eq!(spec_decimals(".3f"), Some(3));
        assert_eq!(spec_decimals("1"), Some(1));
        assert_eq!(spec_decimals("f"), None);
        assert_eq!(spec_decimals("0.xf"), None);
    }

    #[test]
    fn lists_placeholders() {
        let phs = placeholders("a {x:0.2f} b {y} c {nope:9} d { half }");
        assert_eq!(phs.len(), 4);
        assert_eq!(phs[0].name, "x");
        assert_eq!(phs[0].spec, Some("0.2f"));
        assert_eq!(phs[1].name, "y");
        assert!(phs[1].spec.is_none());
        assert_eq!(phs[3].name, "half");
    }
}