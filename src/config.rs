use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub courses: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    server: Option<ServerSection>,
    courses: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct ServerSection {
    host: Option<String>,
    port: Option<u16>,
    /// Tolerated misplacement: TOML scopes any `courses` key written after
    /// `[server]` into that table, where it would otherwise be silently
    /// ignored. Honored with a warning instead.
    #[serde(default)]
    courses: Option<Vec<String>>,
}

impl AppConfig {
    pub fn load(
        config_path: Option<&Path>,
        extra_courses: &[PathBuf],
        host_override: Option<String>,
        port_override: Option<u16>,
    ) -> std::io::Result<AppConfig> {
        let mut warnings = Vec::new();
        let mut courses: Vec<PathBuf> = Vec::new();

        if let Some(path) = config_path {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| std::io::Error::other(format!("reading {}: {e}", path.display())))?;
            let cfg: FileConfig = toml::from_str(&raw)
                .map_err(|e| std::io::Error::other(format!("parsing {}: {e}", path.display())))?;
            let base = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let server = cfg.server.unwrap_or_default();
            let host = host_override.clone().unwrap_or_else(|| server.host.unwrap_or_else(|| "127.0.0.1".into()));
            let port = port_override.unwrap_or_else(|| server.port.unwrap_or(3000));
            if let Some(list) = cfg.courses {
                for entry in list {
                    courses.push(resolve(&base, &entry));
                }
            } else if let Some(list) = server.courses {
                warnings.push(
                    "brillante.toml: 'courses' sits after [server], so TOML scopes it into the \
                     server table; move it above [server] to make the placement explicit. \
                     Honoring it anyway."
                        .to_string(),
                );
                for entry in list {
                    courses.push(resolve(&base, &entry));
                }
            }
            if extra_courses.is_empty() && courses.is_empty() {
                let mut discovered = AppConfig { host: host.clone(), port, courses: Vec::new(), warnings: Vec::new() };
                discover_under(&base.join("content"), &mut discovered);
                courses.extend(discovered.courses);
                warnings.extend(discovered.warnings);
            }
            let mut app = AppConfig { host, port, courses, warnings };
            let mut extra_warnings = Vec::new();
            app.register(extra_courses.to_vec(), &mut extra_warnings);
            app.warnings.extend(extra_warnings);
            return Ok(app);
        }

        let host = host_override.unwrap_or_else(|| "127.0.0.1".into());
        let port = port_override.unwrap_or(3000);
        let mut app = AppConfig { host, port, courses: Vec::new(), warnings: Vec::new() };
        discover_under(Path::new("content"), &mut app);
        let mut extra_warnings = Vec::new();
        app.register(extra_courses.to_vec(), &mut extra_warnings);
        app.warnings.extend(extra_warnings);
        Ok(app)
    }

    fn register(&mut self, extra: Vec<PathBuf>, warnings: &mut Vec<String>) {
        for path in extra {
            if !path.is_dir() {
                warnings.push(format!("registered course directory not found: {}", path.display()));
                continue;
            }
            self.courses.push(path);
        }
    }
}

fn discover_under(root: &Path, app: &mut AppConfig) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        if has_course_files(&dir) {
            app.courses.push(dir);
        } else {
            app.warnings.push(format!(
                "auto-discovery skipped {} (no markdown or course.toml)",
                dir.display()
            ));
        }
    }
}

fn has_course_files(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_md = path.extension().is_some_and(|e| e == "md");
        let is_course = path.file_name().is_some_and(|n| n == "course.toml");
        if is_md || is_course {
            return true;
        }
    }
    false
}

fn resolve(base: &Path, entry: &str) -> PathBuf {
    let p = Path::new(entry);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_top_level_courses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(
            &path,
            "courses = [\"content/a\", \"/abs/b\"]\n[server]\nhost = \"127.0.0.1\"\nport = 3000\n",
        )
        .unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        assert_eq!(app.courses, vec![dir.path().join("content/a"), PathBuf::from("/abs/b")]);
        assert!(app.warnings.is_empty());
    }

    #[test]
    fn tolerates_courses_scoped_into_server_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(
            &path,
            "[server]\nhost = \"127.0.0.1\"\nport = 3000\ncourses = [\"content/a\"]\n",
        )
        .unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        assert_eq!(app.courses, vec![dir.path().join("content/a")]);
        assert!(app
            .warnings
            .iter()
            .any(|w| w.contains("'courses' sits after [server]")));
    }

    #[test]
    fn discovers_when_no_courses_listed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("content/my-course")).unwrap();
        fs::write(dir.path().join("content/my-course/l.md"), "---\ntitle: L\n---\n").unwrap();
        fs::create_dir_all(dir.path().join("content/not-a-course")).unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(&path, "[server]\nhost = \"127.0.0.1\"\nport = 3000\n").unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        assert_eq!(app.courses, vec![dir.path().join("content/my-course")]);
        assert!(app
            .warnings
            .iter()
            .any(|w| w.contains("auto-discovery skipped")));
    }
}
