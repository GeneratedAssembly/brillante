use serde::Deserialize;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::content::frontmatter::{self, CourseMeta};
use crate::diag::Diagnostic;

#[derive(Debug, Clone)]
pub struct Lesson {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub order: f64,
    pub file_name: String,
    pub section: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Course {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub accent: Option<String>,
    pub dir: PathBuf,
    pub lessons: Vec<Lesson>,
    pub sections: Vec<Section>,
    /// ids of `acknowledge`-type widget tags found in lesson bodies (load-time
    /// auto-detection), in first-appearance order, deduplicated.
    pub ack_widget_ids: Vec<String>,
    /// slug of the first lesson containing an acknowledge widget, if any.
    pub ack_lesson_slug: Option<String>,
    widget_config: Option<toml::Table>,
}

impl Course {
    pub fn lesson(&self, slug: &str) -> Option<&Lesson> {
        self.lessons.iter().find(|l| l.slug == slug)
    }

    pub fn lesson_index(&self, slug: &str) -> Option<usize> {
        self.lessons.iter().position(|l| l.slug == slug)
    }

    /// Outline grouping: ungrouped lessons first (no section), then declared
    /// sections in `course.toml` order. Lessons keep their global sort order
    /// inside their group.
    pub fn outline_groups(&self) -> Vec<(Option<&Section>, Vec<&Lesson>)> {
        let mut groups: Vec<(Option<&Section>, Vec<&Lesson>)> = vec![(None, Vec::new())];
        for sec in &self.sections {
            groups.push((Some(sec), Vec::new()));
        }
        for lesson in &self.lessons {
            let idx = match &lesson.section {
                None => 0,
                Some(slug) => self
                    .sections
                    .iter()
                    .position(|s| &s.slug == slug)
                    .map(|p| p + 1)
                    .unwrap_or(0),
            };
            groups[idx].1.push(lesson);
        }
        groups
    }

    pub fn widget_config(&self, id: &str) -> Option<serde_json::Value> {
        let table = self.widget_config.as_ref()?;
        let value = table
            .get("widgets")
            .and_then(|w| w.get(id))
            .or_else(|| table.get(id))?;
        serde_json::to_value(value).ok()
    }

    pub fn lesson_slug_for_file(&self, file_name: &str) -> Option<&str> {
        self.lessons
            .iter()
            .find(|l| l.file_name == file_name)
            .map(|l| l.slug.as_str())
    }

    #[cfg(test)]
    pub fn test_stub(
        slug: String,
        title: String,
        description: Option<String>,
        lessons: Vec<Lesson>,
        dir: std::path::PathBuf,
    ) -> Course {
        Course {
            slug,
            title,
            description,
            accent: None,
            dir,
            lessons,
            sections: Vec::new(),
            ack_widget_ids: Vec::new(),
            ack_lesson_slug: None,
            widget_config: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct WidgetConfigFile {
    #[serde(default)]
    widgets: toml::Table,
}

pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = true;
    for ch in input.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("unnamed");
    }
    out
}

fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

fn natural_key(name: &str) -> Vec<(u64, String)> {
    let mut key = Vec::new();
    let mut num = String::new();
    let mut word = String::new();
    for ch in name.chars() {
        if ch.is_ascii_digit() {
            if !word.is_empty() {
                key.push((u64::MAX, std::mem::take(&mut word)));
            }
            num.push(ch);
        } else {
            if !num.is_empty() {
                key.push((num.parse().unwrap_or(0), String::new()));
                num.clear();
            }
            word.push(ch.to_ascii_lowercase());
        }
    }
    if !num.is_empty() {
        key.push((num.parse().unwrap_or(0), String::new()));
    }
    if !word.is_empty() {
        key.push((u64::MAX, word));
    }
    key
}

pub fn load_course_dir(dir: &Path, diags: &mut Vec<Diagnostic>) -> Option<Course> {
    let mut meta = CourseMeta::default();
    let course_toml = dir.join("course.toml");
    if course_toml.is_file() {
        match std::fs::read_to_string(&course_toml) {
            Ok(raw) => match toml::from_str::<CourseMeta>(&raw) {
                Ok(m) => meta = m,
                Err(e) => diags.push(Diagnostic::error(&course_toml, format!("invalid course.toml: {e}"))),
            },
            Err(e) => diags.push(Diagnostic::error(&course_toml, format!("cannot read course.toml: {e}"))),
        }
    }

    let mut widget_config = None;
    let config_toml = dir.join("config.toml");
    if config_toml.is_file() {
        match std::fs::read_to_string(&config_toml) {
            Ok(raw) => match toml::from_str::<toml::Table>(&raw) {
                Ok(t) => {
                    if let Err(e) = toml::from_str::<WidgetConfigFile>(&raw) {
                        diags.push(Diagnostic::warning(
                            &config_toml,
                            format!("config.toml does not follow the expected [widgets.\"<id>\"] shape: {e}"),
                        ));
                    }
                    widget_config = Some(t);
                }
                Err(e) => diags.push(Diagnostic::error(&config_toml, format!("invalid config.toml: {e}"))),
            },
            Err(e) => diags.push(Diagnostic::error(&config_toml, format!("cannot read config.toml: {e}"))),
        }
    }

    let mut md_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        diags.push(Diagnostic::error(dir, "cannot read course directory"));
        return None;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            md_files.push(path);
        }
    }
    md_files.sort_by(|a, b| {
        let ka = natural_key(&file_stem(a));
        let kb = natural_key(&file_stem(b));
        ka.cmp(&kb).then_with(|| a.cmp(b))
    });

    if md_files.is_empty() {
        diags.push(Diagnostic::warning(dir, "course directory contains no markdown lessons"));
    }

    let dir_title = fallback_title(&file_name(dir));
    let course_slug = slugify(meta.slug.as_deref().unwrap_or(&file_name(dir)));
    let mut sections: Vec<Section> = Vec::new();
    let mut used_section_slugs: Vec<String> = Vec::new();
    for sm in meta.sections.unwrap_or_default() {
        let Some(title) = sm
            .title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
        else {
            diags.push(Diagnostic::warning(
                &course_toml,
                "section in course.toml has no title; ignored",
            ));
            continue;
        };
        let base_input = sm.slug.unwrap_or_else(|| title.clone());
        let base_slug = slugify(&base_input);
        let mut slug = base_slug.clone();
        let mut n = 2;
        while used_section_slugs.contains(&slug) {
            diags.push(Diagnostic::warning(
                &course_toml,
                format!("duplicate section slug '{slug}'; renamed to '{slug}-{n}'"),
            ));
            slug = format!("{base_slug}-{n}");
            n += 1;
        }
        used_section_slugs.push(slug.clone());
        sections.push(Section {
            title,
            slug,
            description: sm.description,
        });
    }
    let mut lessons: Vec<Lesson> = Vec::new();
    let mut used_slugs: Vec<String> = Vec::new();

    for file in &md_files {
        let name = file_name(file);
        let src = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                diags.push(Diagnostic::error(file, format!("cannot read lesson: {e}")));
                continue;
            }
        };
        let Some((fm, body)) = frontmatter::parse_lesson_frontmatter(&src, file, diags) else {
            continue;
        };
        if fm.draft.unwrap_or(false) {
            continue;
        }
        let title = fm.title.clone().unwrap_or_else(|| {
            diags.push(Diagnostic::warning(
                file,
                "lesson has no title in frontmatter; using file name",
            ));
            title_from_stem(&file_stem(file))
        });
        let base_input = match fm.slug.as_deref() {
            Some(s) => s.to_string(),
            None => strip_order_prefix(&file_stem(file)).to_string(),
        };
        let base_slug = slugify(&base_input);
        if !is_valid_slug(&base_slug) {
            diags.push(Diagnostic::warning(file, format!("slug '{base_slug}' is not URL-safe")));
        }
        let mut slug = base_slug.clone();
        let mut n = 2;
        while used_slugs.contains(&slug) {
            diags.push(Diagnostic::warning(
                file,
                format!("duplicate lesson slug '{slug}'; renamed to '{slug}-{n}'"),
            ));
            slug = format!("{base_slug}-{n}");
            n += 1;
        }
        used_slugs.push(slug.clone());
        let section = match fm.section.as_deref() {
            None => None,
            Some(raw) => {
                let s = raw.trim();
                if s.is_empty() {
                    None
                } else if sections.iter().any(|sec| sec.slug == s) {
                    Some(s.to_string())
                } else {
                    diags.push(Diagnostic::warning(
                        file,
                        format!("lesson references unknown section '{s}'; rendering ungrouped"),
                    ));
                    None
                }
            }
        };
        lessons.push(Lesson {
            slug,
            title,
            description: fm.description,
            order: fm.order.as_ref().map(|o| o.as_f64()).unwrap_or(0.0),
            file_name: name,
            section,
            body,
        });
    }

    lessons.sort_by(|a, b| {
        a.order
            .partial_cmp(&b.order)
            .unwrap_or(Ordering::Equal)
            .then_with(|| natural_key(&a.file_name).cmp(&natural_key(&b.file_name)))
    });

    let accent = meta.accent.and_then(|a| {
        if is_hex_color(&a) {
            Some(a)
        } else {
            diags.push(Diagnostic::warning(
                &course_toml,
                format!("accent '{a}' is not a hex color; ignored"),
            ));
            None
        }
    });

    for sec in &sections {
        if !lessons
            .iter()
            .any(|l| l.section.as_deref() == Some(sec.slug.as_str()))
        {
            diags.push(Diagnostic::warning(
                &course_toml,
                format!("section '{}' has no lessons", sec.slug),
            ));
        }
    }

    let mut ack_widget_ids: Vec<String> = Vec::new();
    let mut ack_lesson_slug: Option<String> = None;
    for lesson in &lessons {
        let path = dir.join(&lesson.file_name);
        for tag in crate::content::markdown::scan_widget_tags_in_markdown(&lesson.body, &path) {
            if tag.kind == "acknowledge" && !ack_widget_ids.contains(&tag.id) {
                if ack_lesson_slug.is_none() {
                    ack_lesson_slug = Some(lesson.slug.clone());
                }
                ack_widget_ids.push(tag.id);
            }
        }
    }

    Some(Course {
        slug: course_slug,
        title: meta.title.unwrap_or(dir_title),
        description: meta.description,
        accent,
        dir: dir.to_path_buf(),
        lessons,
        sections,
        ack_widget_ids,
        ack_lesson_slug,
        widget_config,
    })
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn fallback_title(dir_name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in dir_name.chars().enumerate() {
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_order_prefix(stem: &str) -> &str {
    stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_')
}

fn title_from_stem(stem: &str) -> String {
    let cleaned = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_');
    let cleaned = cleaned.trim_start_matches(['-', '_']);
    if cleaned.is_empty() {
        fallback_title(stem)
    } else {
        fallback_title(cleaned)
    }
}

fn is_hex_color(s: &str) -> bool {
    let body = s.strip_prefix('#').unwrap_or(s);
    (body.len() == 3 || body.len() == 6 || body.len() == 8)
        && body.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn slugifies_names() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  --Weird__name--  "), "weird-name");
        assert_eq!(slugify("123"), "123");
        assert_eq!(slugify("***"), "unnamed");
    }

    #[test]
    fn strips_order_prefixes_for_slugs() {
        assert_eq!(strip_order_prefix("01-welcome"), "welcome");
        assert_eq!(strip_order_prefix("12_steel"), "steel");
        assert_eq!(strip_order_prefix("007-x"), "x");
        assert_eq!(strip_order_prefix("loops2"), "loops2");
    }

    #[test]
    fn natural_order_sorts_numerically() {
        let mut names = vec!["10-x.md", "2-x.md", "1-x.md"];
        names.sort_by_key(|n| natural_key(n));
        assert_eq!(names, vec!["1-x.md", "2-x.md", "10-x.md"]);
    }

    #[test]
    fn loads_course_with_order_and_slug_collision() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("course.toml"),
            "title = \"T\"\ndescription = \"D\"\naccent = \"#123abc\"",
        )
        .unwrap();
        fs::write(dir.path().join("a.md"), "---\ntitle: Same\norder: 2\n---\nbody").unwrap();
        fs::write(dir.path().join("b.md"), "---\ntitle: Same\norder: 1\n---\nbody").unwrap();
        fs::write(dir.path().join("c-draft.md"), "---\ntitle: X\ndraft: true\n---\n").unwrap();

        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert_eq!(course.title, "T");
        assert_eq!(course.accent.as_deref(), Some("#123abc"));
        assert_eq!(course.lessons.len(), 2);
        assert_eq!(course.lessons[0].title, "Same");
        assert_eq!(course.lessons[0].slug, "b");
        assert_eq!(course.lessons[1].slug, "a");
        assert!(!course.lessons.iter().any(|l| l.slug == "c-draft"));
    }

    #[test]
    fn duplicate_slugs_get_suffixed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.md"), "---\ntitle: One\n---\n").unwrap();
        fs::write(dir.path().join("x-copy.md"), "---\ntitle: Two\nslug: x\n---\n").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        let slugs: Vec<&str> = course.lessons.iter().map(|l| l.slug.as_str()).collect();
        assert_eq!(slugs, vec!["x-2", "x"]);
    }

    #[test]
    fn falls_back_without_course_toml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lesson.md"), "no frontmatter at all").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert_eq!(course.lessons.len(), 1);
        assert!(!course.lessons[0].title.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("no title")));
    }

    #[test]
    fn widget_config_lookup() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("l.md"), "---\ntitle: L\n---\n").unwrap();
        fs::write(
            dir.path().join("config.toml"),
            "[widgets.\"com.x.q\"]\nanswer = 1\n\n[\"bare.id\"]\nanswer = 2\n",
        )
        .unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        let q = course.widget_config("com.x.q").unwrap();
        assert_eq!(q["answer"], 1);
        let bare = course.widget_config("bare.id").unwrap();
        assert_eq!(bare["answer"], 2);
        assert!(course.widget_config("missing").is_none());
    }

    #[test]
    fn invalid_accent_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("course.toml"), "accent = \"red\"").unwrap();
        fs::write(dir.path().join("l.md"), "---\ntitle: L\n---\n").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert!(course.accent.is_none());
        assert!(diags.iter().any(|d| d.message.contains("hex color")));
    }

    #[test]
    fn sections_group_lessons() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("course.toml"),
            "[[sections]]\ntitle = \"Rays\"\nslug = \"rays\"\n\n[[sections]]\ntitle = \"Waves\"",
        )
        .unwrap();
        fs::write(dir.path().join("01-a.md"), "---\ntitle: A\nsection: waves\n---\n").unwrap();
        fs::write(dir.path().join("02-b.md"), "---\ntitle: B\nsection: rays\n---\n").unwrap();
        fs::write(dir.path().join("03-c.md"), "---\ntitle: C\n---\n").unwrap();
        fs::write(dir.path().join("04-d.md"), "---\ntitle: D\nsection: rays\n---\n").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert_eq!(course.sections.len(), 2);
        assert_eq!(course.sections[0].slug, "rays");
        assert_eq!(course.sections[1].slug, "waves");
        let groups = course.outline_groups();
        assert_eq!(groups.len(), 3);
        assert!(groups[0].0.is_none());
        assert_eq!(
            groups[0].1.iter().map(|l| l.slug.as_str()).collect::<Vec<_>>(),
            vec!["c"]
        );
        assert_eq!(groups[1].0.unwrap().slug, "rays");
        assert_eq!(
            groups[1].1.iter().map(|l| l.slug.as_str()).collect::<Vec<_>>(),
            vec!["b", "d"]
        );
        assert_eq!(
            groups[2].1.iter().map(|l| l.slug.as_str()).collect::<Vec<_>>(),
            vec!["a"]
        );
        assert!(!diags.iter().any(|d| d.message.contains("has no lessons")));
        assert!(!diags.iter().any(|d| d.message.contains("unknown section")));
    }

    #[test]
    fn duplicate_section_slugs_get_suffixed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("course.toml"),
            "[[sections]]\ntitle = \"Rays\"\n\n[[sections]]\ntitle = \"Rays\"\nslug = \"RAYS!\"",
        )
        .unwrap();
        fs::write(dir.path().join("a.md"), "---\ntitle: A\nsection: rays\n---\n").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert_eq!(course.sections[0].slug, "rays");
        assert_eq!(course.sections[1].slug, "rays-2");
        assert!(diags.iter().any(|d| d.message.contains("duplicate section slug")));
        // the first declared section still owns the bare slug
        assert!(course.lessons[0].section.as_deref() == Some("rays"));
    }

    #[test]
    fn unknown_section_slug_warns_and_ungroups() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("course.toml"),
            "[[sections]]\ntitle = \"Rays\"\nslug = \"rays\"",
        )
        .unwrap();
        fs::write(dir.path().join("a.md"), "---\ntitle: A\nsection: nope\n---\n").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert!(course.lessons[0].section.is_none());
        assert!(diags.iter().any(|d| d.message.contains("unknown section")));
        assert!(diags.iter().any(|d| d.message.contains("has no lessons")));
    }

    #[test]
    fn titleless_section_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("course.toml"), "[[sections]]\ndescription = \"d\"").unwrap();
        fs::write(dir.path().join("a.md"), "---\ntitle: A\n---\n").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert!(course.sections.is_empty());
        assert!(diags.iter().any(|d| d.message.contains("no title")));
    }

    #[test]
    fn detects_acknowledge_widgets_in_bodies() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("01-a.md"),
            "---\ntitle: A\n---\n<Widget type=\"acknowledge\" id=\"x.ack\" />\n\n```html\n<Widget type=\"acknowledge\" id=\"ignored.incode\" />\n```\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("02-b.md"),
            "---\ntitle: B\n---\n<widget TYPE='Acknowledge' id=\"y.ack\" /><Widget type=\"acknowledge\" id=\"x.ack\" />",
        )
        .unwrap();
        fs::write(dir.path().join("03-c.md"), "---\ntitle: C\n---\nno widgets").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        assert_eq!(course.ack_widget_ids, vec!["x.ack", "y.ack"]);
        assert_eq!(course.ack_lesson_slug.as_deref(), Some("a"));
    }
}
