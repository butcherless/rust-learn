//! Inbound adapter: reads the project list from a TOML file.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::domain::error::ProjectParseError;
use crate::domain::model::Project;

/// Why the project list could not be loaded.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file could not be read.
    #[error("cannot read {}", path.display())]
    Io {
        /// The file that was being read.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file is not valid TOML or lacks the expected shape.
    #[error("invalid TOML")]
    Toml(#[from] toml::de::Error),
    /// An entry is not a valid `owner/repo`.
    #[error("invalid project {entry:?}")]
    InvalidProject {
        /// The offending entry.
        entry: String,
        /// Why it was rejected.
        source: ProjectParseError,
    },
}

#[derive(Deserialize)]
struct ProjectsFile {
    projects: Vec<String>,
}

/// Loads the project list from the file at `path`.
pub fn load_projects(path: &Path) -> Result<Vec<Project>, ConfigError> {
    fs::read_to_string(path)
        .map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })
        .and_then(|content| parse_projects(&content))
}

/// Parses the content of a projects file.
pub fn parse_projects(content: &str) -> Result<Vec<Project>, ConfigError> {
    let file: ProjectsFile = toml::from_str(content)?;
    file.projects
        .into_iter()
        .map(|entry| {
            entry
                .parse()
                .map_err(|source| ConfigError::InvalidProject { entry, source })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_file() {
        let projects = parse_projects(r#"projects = ["a/b", " c/d "]"#).unwrap();
        let names: Vec<String> = projects.iter().map(ToString::to_string).collect();
        assert_eq!(names, ["a/b", "c/d"]);
    }

    #[test]
    fn empty_list_is_valid() {
        assert!(parse_projects("projects = []").unwrap().is_empty());
    }

    #[test]
    fn invalid_entry_is_named() {
        let error = parse_projects(r#"projects = ["a/b", "nope"]"#).unwrap_err();
        assert!(matches!(error, ConfigError::InvalidProject { .. }));
        assert!(error.to_string().contains("nope"), "message: {error}");
    }

    #[test]
    fn malformed_toml_is_rejected() {
        let error = parse_projects("projects = [").unwrap_err();
        assert!(matches!(error, ConfigError::Toml(_)));
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let error = load_projects(Path::new("does-not-exist.toml")).unwrap_err();
        assert!(matches!(error, ConfigError::Io { .. }));
    }
}
