use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub path: PathBuf,
    pub message: String,
}

impl Diagnostic {
    pub fn warning(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn error(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.path.display(), self.message)
    }
}

pub fn log_diagnostics(diags: &[Diagnostic]) {
    for d in diags {
        match d.severity {
            Severity::Warning => tracing::warn!("{d}"),
            Severity::Error => tracing::error!("{d}"),
        }
    }
}
