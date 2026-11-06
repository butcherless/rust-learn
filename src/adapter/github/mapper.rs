//! Maps wire types to the domain.

use crate::adapter::github::dto::PullRequestDto;
use crate::domain::model::{PrNumber, PrState, PrTitle, PrUrl, PullRequest};

/// Converts a pull request DTO into its domain form.
pub fn to_domain(dto: PullRequestDto) -> PullRequest {
    PullRequest {
        number: PrNumber(dto.number),
        title: PrTitle(dto.title),
        state: if dto.draft {
            PrState::Draft
        } else {
            PrState::Open
        },
        url: PrUrl(dto.html_url),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://github.com/o/r/pull/2";

    /// Decodes an API pull request whose JSON ends with `draft_field`.
    fn decode(draft_field: &str) -> PullRequestDto {
        serde_json::from_str(&format!(
            r#"{{"number":2,"title":"t","state":"open","html_url":"{URL}"{draft_field}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn maps_every_field_and_the_draft_flag() {
        [
            (r#","draft":true"#, PrState::Draft),
            (r#","draft":false"#, PrState::Open),
            ("", PrState::Open),
        ]
        .into_iter()
        .for_each(|(draft_field, state)| {
            assert_eq!(
                to_domain(decode(draft_field)),
                PullRequest {
                    number: PrNumber(2),
                    title: PrTitle("t".to_owned()),
                    state,
                    url: PrUrl(URL.to_owned()),
                },
                "draft field: {draft_field:?}"
            );
        });
    }
}
