//! Domain errors.

use thiserror::Error;

/// Why a string is not a valid `owner/repo` project.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectParseError {
    /// The entry is not exactly two non-empty parts separated by one `/`.
    #[error("expected the format owner/repo")]
    InvalidFormat,
}

/// Why the pull requests of a project could not be retrieved.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    /// The repository does not exist or is not visible to the caller.
    #[error("repository not found")]
    NotFound,
    /// The credentials were rejected or lack permission.
    #[error("unauthorized")]
    Unauthorized,
    /// The source's rate limit has been reached.
    #[error("rate limited")]
    RateLimited,
    /// The source could not be reached or answered with a failure.
    #[error("unavailable: {0}")]
    Unavailable(String),
    /// The source answered with content that could not be understood.
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}
