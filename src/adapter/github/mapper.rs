//! Maps wire types to the domain.

use crate::adapter::github::dto::PullRequestDto;
use crate::domain::model::{PrNumber, PrState, PrTitle, PullRequest};

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(json: &str) -> PullRequestDto {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn draft_true_maps_to_draft() {
        let dto = decode(r#"{"number":1,"title":"t","state":"open","draft":true}"#);
        assert_eq!(to_domain(dto).state, PrState::Draft);
    }

    #[test]
    fn missing_draft_maps_to_open() {
        let dto = decode(r#"{"number":2,"title":"t","state":"open"}"#);
        let pull_request = to_domain(dto);
        assert_eq!(pull_request.state, PrState::Open);
        assert_eq!(pull_request.number, PrNumber(2));
        assert_eq!(pull_request.title, PrTitle("t".to_owned()));
    }
}
