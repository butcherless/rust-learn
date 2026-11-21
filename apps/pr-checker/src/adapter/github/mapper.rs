//! Maps wire types to the domain.

use crate::adapter::github::dto::{PullRequestDto, WorkflowRunDto};
use crate::domain::model::{
    Mergeable, PrNumber, PrState, PrTitle, PrUrl, PullRequest, RunStatus, WorkflowRun,
};

/// What a run without a name or a branch shows instead.
const MISSING: &str = "—";

/// Converts a pull request DTO into its domain form.
pub fn to_domain(dto: PullRequestDto) -> PullRequest {
    PullRequest {
        number: PrNumber::from(dto.number),
        title: PrTitle::from(dto.title),
        state: if dto.draft {
            PrState::Draft
        } else {
            PrState::Open
        },
        mergeable: Mergeable::from(dto.mergeable),
        url: PrUrl::from(dto.html_url),
    }
}

/// Converts a workflow run DTO into its domain form, or `None` once it has finished or when its
/// status is not one GitHub documents.
pub fn to_running(dto: WorkflowRunDto) -> Option<WorkflowRun> {
    let status = match dto.status.as_deref()? {
        "queued" => RunStatus::Queued,
        "in_progress" => RunStatus::InProgress,
        "requested" | "waiting" | "pending" => RunStatus::Waiting,
        _ => return None,
    };
    Some(WorkflowRun {
        name: dto.name.unwrap_or_else(|| MISSING.to_owned()),
        branch: dto.head_branch.unwrap_or_else(|| MISSING.to_owned()),
        event: dto.event,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://github.com/o/r/pull/2";

    /// Decodes an API pull request whose JSON ends with `fields`.
    fn decode(fields: &str) -> PullRequestDto {
        serde_json::from_str(&format!(
            r#"{{"number":2,"title":"t","state":"open","html_url":"{URL}"{fields}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn maps_every_field_the_draft_flag_and_the_mergeable_flag() {
        [
            (r#","draft":true"#, PrState::Draft, Mergeable::Unknown),
            (r#","draft":false"#, PrState::Open, Mergeable::Unknown),
            ("", PrState::Open, Mergeable::Unknown),
            (r#","mergeable":true"#, PrState::Open, Mergeable::Yes),
            (r#","mergeable":false"#, PrState::Open, Mergeable::No),
            (r#","mergeable":null"#, PrState::Open, Mergeable::Unknown),
        ]
        .into_iter()
        .for_each(|(fields, state, mergeable)| {
            assert_eq!(
                to_domain(decode(fields)),
                PullRequest {
                    number: PrNumber::from(2),
                    title: PrTitle::from("t".to_owned()),
                    state,
                    mergeable,
                    url: PrUrl::from(URL.to_owned()),
                },
                "fields: {fields:?}"
            );
        });
    }

    #[test]
    fn keeps_only_unfinished_runs() {
        let run = |status: Option<&str>| WorkflowRunDto {
            name: Some("CI".to_owned()),
            head_branch: Some("main".to_owned()),
            event: "push".to_owned(),
            status: status.map(str::to_owned),
        };
        [
            (Some("queued"), Some(RunStatus::Queued)),
            (Some("in_progress"), Some(RunStatus::InProgress)),
            (Some("requested"), Some(RunStatus::Waiting)),
            (Some("waiting"), Some(RunStatus::Waiting)),
            (Some("pending"), Some(RunStatus::Waiting)),
            (Some("completed"), None),
            (Some("something new"), None),
            (None, None),
        ]
        .into_iter()
        .for_each(|(status, expected)| {
            assert_eq!(
                to_running(run(status)).map(|run| run.status),
                expected,
                "{status:?}"
            );
        });
        let unnamed = WorkflowRunDto {
            name: None,
            head_branch: None,
            ..run(Some("queued"))
        };
        assert_eq!(
            to_running(unnamed),
            Some(WorkflowRun {
                name: MISSING.to_owned(),
                branch: MISSING.to_owned(),
                event: "push".to_owned(),
                status: RunStatus::Queued,
            })
        );
    }
}
