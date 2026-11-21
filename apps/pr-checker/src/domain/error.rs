//! Domain errors.

use thiserror::Error;

/// Why a string is not a valid `owner/repo` project.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectParseError {
    /// The entry is not exactly two non-empty parts separated by one `/`.
    #[error("expected the format owner/repo")]
    InvalidFormat,
}

/// Why a request to the source failed: retrieving the pull requests of a project, or merging one.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    /// The repository does not exist or is not visible to the caller.
    #[error("repository not found")]
    NotFound,
    /// The credentials were rejected or lack permission.
    #[error("unauthorized")]
    Unauthorized,
    /// The source's rate limit has been reached. It holds how many seconds the source asked to
    /// wait before trying again, when it said.
    #[error("rate limited{}", retry_text(*.0))]
    RateLimited(Option<u64>),
    /// The source could not be reached or answered with a failure.
    #[error("unavailable: {0}")]
    Unavailable(String),
    /// The source answered with content that could not be understood.
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    /// The pull request cannot be merged: it has conflicts, is a draft, or fails a branch rule.
    #[error("not mergeable")]
    NotMergeable,
}

/// `, retry in N s` when the wait is known, and nothing otherwise.
fn retry_text(seconds: Option<u64>) -> String {
    seconds
        .map(|seconds| format!(", retry in {seconds} s"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_limit_says_how_long_to_wait_when_known() {
        assert_eq!(SourceError::RateLimited(None).to_string(), "rate limited");
        assert_eq!(
            SourceError::RateLimited(Some(42)).to_string(),
            "rate limited, retry in 42 s"
        );
    }
}
