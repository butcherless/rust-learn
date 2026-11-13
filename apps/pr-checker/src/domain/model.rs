//! Domain model.

use std::fmt;
use std::str::FromStr;

use crate::domain::error::ProjectParseError;

/// Repository owner. Only constructed by parsing a [`Project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner(String);

/// Repository name. Only constructed by parsing a [`Project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoName(String);

impl Owner {
    /// The owner as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RepoName {
    /// The repository name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A GitHub repository, validated as `owner/repo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    owner: Owner,
    repo: RepoName,
}

impl Project {
    /// The repository owner.
    pub fn owner(&self) -> &Owner {
        &self.owner
    }

    /// The repository name.
    pub fn repo(&self) -> &RepoName {
        &self.repo
    }
}

impl FromStr for Project {
    type Err = ProjectParseError;

    fn from_str(entry: &str) -> Result<Self, Self::Err> {
        entry
            .trim()
            .split_once('/')
            .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
            .map(|(owner, repo)| Project {
                owner: Owner(owner.to_owned()),
                repo: RepoName(repo.to_owned()),
            })
            .ok_or(ProjectParseError::InvalidFormat)
    }
}

impl fmt::Display for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner.0, self.repo.0)
    }
}

/// A pull request number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrNumber(u64);

impl From<u64> for PrNumber {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for PrNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A pull request title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrTitle(String);

impl From<String> for PrTitle {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for PrTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The web address of a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrUrl(String);

impl From<String> for PrUrl {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl PrUrl {
    /// The address as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PrUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The state of an open pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    /// Open and ready for review.
    Open,
    /// Open and marked as draft.
    Draft,
}

impl fmt::Display for PrState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PrState::Open => "open",
            PrState::Draft => "draft",
        })
    }
}

/// Whether an open pull request can be merged without conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mergeable {
    /// It merges cleanly into its base branch.
    Yes,
    /// It has conflicts with its base branch.
    No,
    /// GitHub has not finished computing it yet.
    Unknown,
}

impl From<Option<bool>> for Mergeable {
    fn from(mergeable: Option<bool>) -> Self {
        match mergeable {
            Some(true) => Mergeable::Yes,
            Some(false) => Mergeable::No,
            None => Mergeable::Unknown,
        }
    }
}

impl fmt::Display for Mergeable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Mergeable::Yes => "yes",
            Mergeable::No => "no",
            Mergeable::Unknown => "unknown",
        })
    }
}

/// An open pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// The pull request number.
    pub number: PrNumber,
    /// The pull request title.
    pub title: PrTitle,
    /// The pull request state.
    pub state: PrState,
    /// Whether it can be merged without conflicts.
    pub mergeable: Mergeable,
    /// The pull request's page on the web.
    pub url: PrUrl,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_project() {
        let project: Project = "rust-lang/rust".parse().unwrap();
        assert_eq!(project.owner().as_str(), "rust-lang");
        assert_eq!(project.repo().as_str(), "rust");
    }

    #[test]
    fn rejects_invalid_projects() {
        ["", "owner", "/repo", "owner/", "a/b/c"]
            .iter()
            .for_each(|entry| {
                assert_eq!(
                    entry.parse::<Project>(),
                    Err(ProjectParseError::InvalidFormat),
                    "entry: {entry:?}"
                );
            });
    }

    #[test]
    fn trims_whitespace() {
        let project: Project = "  tokio-rs/tokio \n".parse().unwrap();
        assert_eq!(project.to_string(), "tokio-rs/tokio");
    }

    #[test]
    fn display_round_trips() {
        let project: Project = "seanmonstar/reqwest".parse().unwrap();
        assert_eq!(project.to_string().parse::<Project>(), Ok(project));
    }

    #[test]
    fn state_displays_lowercase() {
        assert_eq!(PrState::Open.to_string(), "open");
        assert_eq!(PrState::Draft.to_string(), "draft");
    }

    #[test]
    fn mergeable_comes_from_the_api_flag_and_displays_lowercase() {
        [
            (Some(true), Mergeable::Yes, "yes"),
            (Some(false), Mergeable::No, "no"),
            (None, Mergeable::Unknown, "unknown"),
        ]
        .into_iter()
        .for_each(|(flag, mergeable, text)| {
            assert_eq!(Mergeable::from(flag), mergeable, "flag {flag:?}");
            assert_eq!(mergeable.to_string(), text, "flag {flag:?}");
        });
    }
}
