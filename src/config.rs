use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub courses: Vec<PathBuf>,
    pub pocketbase: Option<PocketBaseConfig>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PocketBaseConfig {
    pub url: String,
    /// Set true when Brillante itself is served over HTTPS; marks the
    /// session cookie `Secure`.
    pub secure_cookies: bool,
    /// Browser-facing base URL of this server (no trailing slash). Used to
    /// build OAuth2 redirect URIs; defaults to `http://<host>:<port>`.
    pub public_url: String,
    /// Emails that should always hold the `admin` role. At startup the server
    /// promotes existing accounts via a PB superuser (credentials from the
    /// BRILLANTE_PB_SUPERUSER_EMAIL / BRILLANTE_PB_SUPERUSER_PASSWORD env
    /// vars). Emails without an account are warned about, never created.
    pub bootstrap_admins: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    server: Option<ServerSection>,
    courses: Option<Vec<String>>,
    pocketbase: Option<PocketBaseSection>,
}

#[derive(Debug, Default, Deserialize)]
struct PocketBaseSection {
    url: Option<String>,
    #[serde(default)]
    secure_cookies: Option<bool>,
    #[serde(default)]
    bootstrap_admins: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct ServerSection {
    host: Option<String>,
    port: Option<u16>,
    /// Browser-facing base URL of this server (e.g. "https://courses.example.com").
    /// When set, OAuth2 redirect URIs are built from it instead of the derived
    /// `http://<host>:<port>`; relevant when Brillante runs behind a proxy.
    #[serde(default)]
    public_url: Option<String>,
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
            let derived_url = derive_public_url(&host, port);
            let public_url = match server.public_url.as_deref().map(str::trim) {
                Some(raw) if !raw.is_empty() => match normalize_public_url(raw) {
                    Some(url) => url,
                    None => {
                        warnings.push(format!(
                            "brillante.toml: [server] public_url '{raw}' must start with http:// or https://; \
                             falling back to the derived {derived_url}"
                        ));
                        derived_url.clone()
                    }
                },
                _ => derived_url.clone(),
            };
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
                let mut discovered = AppConfig {
                    host: host.clone(),
                    port,
                    courses: Vec::new(),
                    pocketbase: None,
                    warnings: Vec::new(),
                };
                discover_under(&base.join("content"), &mut discovered);
                courses.extend(discovered.courses);
                warnings.extend(discovered.warnings);
            }
            let pocketbase = cfg.pocketbase.and_then(|pb| {
                build_pocketbase(
                    pb.url,
                    pb.secure_cookies.unwrap_or(false),
                    pb.bootstrap_admins.unwrap_or_default(),
                    public_url,
                    &mut warnings,
                )
            });
            let mut app = AppConfig {
                host,
                port,
                courses,
                pocketbase,
                warnings,
            };
            let mut extra_warnings = Vec::new();
            app.register(extra_courses.to_vec(), &mut extra_warnings);
            app.warnings.extend(extra_warnings);
            return Ok(app);
        }

        let host = host_override.unwrap_or_else(|| "127.0.0.1".into());
        let port = port_override.unwrap_or(3000);
        let mut app = AppConfig {
            host,
            port,
            courses: Vec::new(),
            pocketbase: None,
            warnings: Vec::new(),
        };
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

fn build_pocketbase(
    url: Option<String>,
    secure_cookies: bool,
    bootstrap_admins: Vec<String>,
    public_url: String,
    warnings: &mut Vec<String>,
) -> Option<PocketBaseConfig> {
    let raw = url?.trim().trim_end_matches('/').to_string();
    if raw.is_empty() {
        warnings.push("brillante.toml: [pocketbase] url is empty; auth disabled".to_string());
        return None;
    }
    if !(raw.starts_with("http://") || raw.starts_with("https://")) {
        warnings.push(format!(
            "brillante.toml: [pocketbase] url '{raw}' must start with http:// or https://; auth disabled"
        ));
        return None;
    }
    for email in &bootstrap_admins {
        if email.trim().is_empty() {
            warnings.push(
                "brillante.toml: [pocketbase] bootstrap_admins contains an empty entry".to_string(),
            );
        } else if !email.contains('@') {
            warnings.push(format!(
                "brillante.toml: [pocketbase] bootstrap_admins entry '{email}' does not look like an email"
            ));
        }
    }
    Some(PocketBaseConfig {
        url: raw,
        secure_cookies,
        public_url,
        bootstrap_admins,
    })
}

/// Browser-facing base URL derived from the bind address. Bind-all addresses
/// are replaced by their loopback counterpart so the derived URL is usable.
fn derive_public_url(host: &str, port: u16) -> String {
    let host = match host.trim() {
        "" | "0.0.0.0" | "::" => "127.0.0.1".to_string(),
        h if h.contains(':') && !h.starts_with('[') => format!("[{h}]"),
        h => h.to_string(),
    };
    format!("http://{host}:{port}")
}

fn normalize_public_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        .then(|| trimmed.to_string())
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
    fn derives_public_url_from_bind_address() {
        assert_eq!(derive_public_url("127.0.0.1", 3000), "http://127.0.0.1:3000");
        assert_eq!(derive_public_url("0.0.0.0", 8080), "http://127.0.0.1:8080");
        assert_eq!(derive_public_url("::", 3000), "http://127.0.0.1:3000");
        assert_eq!(derive_public_url("::1", 3000), "http://[::1]:3000");
        assert_eq!(derive_public_url("example.com", 3000), "http://example.com:3000");
    }

    #[test]
    fn parses_server_public_url_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(
            &path,
            "courses = [\"content/a\"]\n[server]\npublic_url = \"https://courses.example.com/\"\n[pocketbase]\nurl = \"http://127.0.0.1:8090\"\n",
        )
        .unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        let pb = app.pocketbase.expect("pocketbase config");
        assert_eq!(pb.public_url, "https://courses.example.com");
        assert!(app.warnings.is_empty());
    }

    #[test]
    fn invalid_public_url_falls_back_to_derived_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(
            &path,
            "courses = [\"content/a\"]\n[server]\npublic_url = \"courses.example.com\"\n[pocketbase]\nurl = \"http://127.0.0.1:8090\"\n",
        )
        .unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        let pb = app.pocketbase.expect("pocketbase config");
        assert_eq!(pb.public_url, "http://127.0.0.1:3000");
        assert!(app.warnings.iter().any(|w| w.contains("[server] public_url")));
    }

    #[test]
    fn parses_pocketbase_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(
            &path,
            "courses = [\"content/a\"]\n[pocketbase]\nurl = \"http://127.0.0.1:8090/\"\nsecure_cookies = true\n",
        )
        .unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        let pb = app.pocketbase.expect("pocketbase config");
        assert_eq!(pb.url, "http://127.0.0.1:8090");
        assert!(pb.secure_cookies);
        assert!(app.warnings.is_empty());
    }

    #[test]
    fn invalid_pocketbase_url_disables_auth_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brillante.toml");
        fs::write(&path, "courses = [\"content/a\"]\n[pocketbase]\nurl = \"ftp://nope\"\n").unwrap();
        let app = AppConfig::load(Some(&path), &[], None, None).unwrap();
        assert!(app.pocketbase.is_none());
        assert!(
            app.warnings
                .iter()
                .any(|w| w.contains("[pocketbase] url 'ftp://nope'"))
        );
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
