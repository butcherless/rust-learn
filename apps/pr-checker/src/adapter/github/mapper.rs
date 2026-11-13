//! Maps wire types to the domain.

use crate::adapter::github::dto::PullRequestDto;
use crate::domain::model::{Mergeable, PrNumber, PrState, PrTitle, PrUrl, PullRequest};

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
        mergeable: Mergeable::from(dto.mergeable),
        url: PrUrl(dto.html_url),
    }
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
                    number: PrNumber(2),
                    title: PrTitle("t".to_owned()),
                    state,
                    mergeable,
                    url: PrUrl(URL.to_owned()),
                },
                "fields: {fields:?}"
            );
        });
    }
}
