//! Outbound adapter: the GitHub REST API.

pub mod client;
pub mod dto;
pub mod mapper;

pub use client::{DEFAULT_BASE_URL, GithubClient, GithubClientError, GithubToken};
