pub mod course;
pub mod frontmatter;
pub mod markdown;

use std::path::PathBuf;

use crate::content::course::Course;
use crate::diag::{log_diagnostics, Diagnostic};

#[derive(Debug, Clone)]
pub struct ContentStore {
    pub registered: Vec<PathBuf>,
    pub courses: Vec<Course>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ContentStore {
    pub fn load(registered: &[PathBuf]) -> ContentStore {
        let mut diagnostics = Vec::new();
        let mut courses: Vec<Course> = Vec::new();
        let mut seen: Vec<String> = Vec::new();

        for dir in registered {
            let Some(course) = course::load_course_dir(dir, &mut diagnostics) else {
                continue;
            };
            if seen.contains(&course.slug) {
                diagnostics.push(Diagnostic::warning(
                    dir,
                    format!(
                        "course slug '{}' already registered; skipping duplicate",
                        course.slug
                    ),
                ));
                continue;
            }
            seen.push(course.slug.clone());
            courses.push(course);
        }

        courses.sort_by(|a, b| a.slug.cmp(&b.slug));
        ContentStore {
            registered: registered.to_vec(),
            courses,
            diagnostics,
        }
    }

    pub fn reload(&self) -> ContentStore {
        let store = ContentStore::load(&self.registered);
        log_diagnostics(&store.diagnostics);
        store
    }

    pub fn course(&self, slug: &str) -> Option<&Course> {
        self.courses.iter().find(|c| c.slug == slug)
    }
}
