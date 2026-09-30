use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::Path;

use crate::diag::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontmatterFormat {
    Yaml,
    Toml,
}

#[derive(Debug, Default, Deserialize)]
pub struct LessonFrontmatter {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub order: Option<OrderValue>,
    #[serde(default)]
    pub draft: Option<bool>,
    #[serde(default)]
    pub section: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OrderValue {
    Int(i64),
    Float(f64),
}

impl OrderValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            OrderValue::Int(i) => *i as f64,
            OrderValue::Float(f) => *f,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct SectionMeta {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CourseMeta {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub sections: Option<Vec<SectionMeta>>,
}

pub enum Split {
    With {
        format: FrontmatterFormat,
        text: String,
        body: String,
    },
    Without(String),
}

pub fn split(src: &str) -> Result<Split, String> {
    let trimmed_start = src.trim_start_matches('\u{feff}');
    let mut lines = trimmed_start.lines();
    let first = match lines.next() {
        Some(l) => l.trim_end(),
        None => return Ok(Split::Without(String::new())),
    };
    let (format, delim) = if first == "---" {
        (FrontmatterFormat::Yaml, "---")
    } else if first == "+++" {
        (FrontmatterFormat::Toml, "+++")
    } else {
        return Ok(Split::Without(trimmed_start.to_string()));
    };

    let rest = trimmed_start.split_once('\n').map(|x| x.1)
        .unwrap_or("");
    let mut closing = None;
    let mut body_start = 0usize;
    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']).trim_end();
        if bare == delim {
            closing = Some(offset);
            body_start = offset + line.len();
            break;
        }
        offset += line.len();
    }
    let closing = closing.ok_or_else(|| {
        format!("frontmatter opened with '{delim}' but never closed")
    })?;

    let text = rest[..closing].trim().to_string();
    let body = rest[body_start..].to_string();
    Ok(Split::With { format, text, body })
}

pub fn parse<T: DeserializeOwned>(
    format: FrontmatterFormat,
    text: &str,
) -> Result<T, String> {
    match format {
        FrontmatterFormat::Yaml => serde_yaml::from_str(text)
            .map_err(|e| format!("invalid YAML frontmatter: {e}")),
        FrontmatterFormat::Toml => toml::from_str(text)
            .map_err(|e| format!("invalid TOML frontmatter: {e}")),
    }
}

pub fn parse_lesson_frontmatter(
    src: &str,
    path: &Path,
    diags: &mut Vec<Diagnostic>,
) -> Option<(LessonFrontmatter, String)> {
    match split(src) {
        Err(e) => {
            diags.push(Diagnostic::error(path, e));
            None
        }
        Ok(Split::Without(body)) => Some((LessonFrontmatter::default(), body)),
        Ok(Split::With { format, text, body }) => match parse::<LessonFrontmatter>(format, &text) {
            Ok(fm) => Some((fm, body)),
            Err(e) => {
                diags.push(Diagnostic::error(path, e));
                None
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_yaml_frontmatter() {
        let src = "---\ntitle: Hi\norder: 2\n---\n\nBody here\n";
        match split(src).unwrap() {
            Split::With { format, text, body } => {
                assert_eq!(format, FrontmatterFormat::Yaml);
                assert_eq!(text, "title: Hi\norder: 2");
                assert!(body.trim_start().starts_with("Body"));
            }
            _ => panic!("expected frontmatter"),
        }
    }

    #[test]
    fn splits_toml_frontmatter() {
        let src = "+++\ntitle = \"Hi\"\ndraft = true\n+++\nBody";
        match split(src).unwrap() {
            Split::With { format, text, .. } => {
                assert_eq!(format, FrontmatterFormat::Toml);
                assert!(text.contains("draft = true"));
            }
            _ => panic!("expected frontmatter"),
        }
    }

    #[test]
    fn body_without_frontmatter() {
        match split("just text\n").unwrap() {
            Split::Without(body) => assert_eq!(body, "just text\n"),
            _ => panic!("expected no frontmatter"),
        }
    }

    #[test]
    fn unclosed_frontmatter_is_error() {
        assert!(split("---\ntitle: Hi\nnope").is_err());
        assert!(split("+++\ntitle = 'Hi'").is_err());
    }

    #[test]
    fn parses_yaml_lesson_fields() {
        let fm: LessonFrontmatter = parse(
            FrontmatterFormat::Yaml,
            "title: Loops\ndescription: d\norder: 3.5\ndraft: false",
        )
        .unwrap();
        assert_eq!(fm.title.as_deref(), Some("Loops"));
        assert_eq!(fm.order.unwrap().as_f64(), 3.5);
        assert!(!fm.draft.unwrap());
    }

    #[test]
    fn parses_toml_lesson_fields() {
        let fm: LessonFrontmatter = parse(
            FrontmatterFormat::Toml,
            "title = 'Loops'\nslug = 'custom-slug'\norder = 2\ndraft = true",
        )
        .unwrap();
        assert_eq!(fm.slug.as_deref(), Some("custom-slug"));
        assert_eq!(fm.order.unwrap().as_f64(), 2.0);
        assert!(fm.draft.unwrap());
    }

    #[test]
    fn empty_frontmatter_is_default() {
        let fm: LessonFrontmatter = parse(FrontmatterFormat::Yaml, "").unwrap();
        assert!(fm.title.is_none());
        assert!(fm.order.is_none());
        assert!(fm.section.is_none());
    }

    #[test]
    fn parses_section_key() {
        let fm: LessonFrontmatter = parse(FrontmatterFormat::Yaml, "title: X\nsection: rays").unwrap();
        assert_eq!(fm.section.as_deref(), Some("rays"));
        let meta: CourseMeta = parse(
            FrontmatterFormat::Toml,
            "title = 'T'\n[[sections]]\ntitle = 'Rays'\nslug = 'rays'\ndescription = 'd'",
        )
        .unwrap();
        let sections = meta.sections.unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title.as_deref(), Some("Rays"));
        assert_eq!(sections[0].slug.as_deref(), Some("rays"));
    }

    #[test]
    fn wrong_types_are_errors() {
        let result = parse::<LessonFrontmatter>(FrontmatterFormat::Toml, "order = 'not a number'");
        assert!(result.is_err());
    }
}
