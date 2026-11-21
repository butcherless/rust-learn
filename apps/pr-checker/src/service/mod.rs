//! Application use cases.

pub mod list_open_prs;
pub mod running_workflows;

pub use list_open_prs::{
    MAX_CONCURRENCY, ProjectReport, has_failures, list_open_prs, stream_open_prs,
};
pub use running_workflows::{RunsReport, stream_running_workflows};
