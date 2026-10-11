use crate::config::ApprovalGroup;
use crate::diff::{DiffError, DiffProvider};
use crate::domain::ChangedPath;
use crate::gates::{ApprovalCheck, ApprovalChecker, GateError};
use crate::report::replace_marked_comment;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("GitHub request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("GitHub returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("invalid repository '{0}', expected owner/name")]
    Repository(String),
    #[error("invalid team '{0}', expected org/team")]
    Team(String),
    #[error("GitHub response was invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("GitHub file pagination repeated page {0}")]
    RepeatedPage(usize),
}

#[derive(Debug, Clone)]
pub struct GithubApi {
    client: Client,
    base_url: String,
    repository: String,
    pull_number: u64,
}

impl GithubApi {
    pub fn new(
        token: &str,
        repository: impl Into<String>,
        pull_number: u64,
    ) -> Result<Self, GithubError> {
        let repository = repository.into();
        validate_repository(&repository)?;
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("risk-gate"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        let authorization =
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| GithubError::Http {
                status: 0,
                body: "invalid token header".into(),
            })?;
        headers.insert(AUTHORIZATION, authorization);
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()?;
        Ok(Self {
            client,
            base_url: "https://api.github.com".into(),
            repository,
            pull_number,
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_owned();
        self
    }

    pub fn changed_paths(&self) -> Result<Vec<ChangedPath>, GithubError> {
        let mut page = 1;
        let mut seen_pages = BTreeSet::new();
        let mut result = Vec::new();
        loop {
            if !seen_pages.insert(page) {
                return Err(GithubError::RepeatedPage(page));
            }
            let url = self.endpoint(&format!(
                "/repos/{}/pulls/{}/files?per_page=100&page={page}",
                self.repository, self.pull_number
            ));
            let files: Vec<PullFile> = self.send(self.client.get(url))?;
            let count = files.len();
            result.extend(files.into_iter().map(|file| ChangedPath {
                path: file.filename,
                old_path: if file.status == "renamed" {
                    file.previous_filename
                } else {
                    None
                },
                is_new: file.status == "added",
            }));
            if count < 100 {
                return Ok(result);
            }
            page += 1;
        }
    }

    fn reviews(&self) -> Result<Vec<Review>, GithubError> {
        let mut page = 1;
        let mut result = Vec::new();
        loop {
            let url = self.endpoint(&format!(
                "/repos/{}/pulls/{}/reviews?per_page=100&page={page}",
                self.repository, self.pull_number
            ));
            let reviews: Vec<Review> = self.send(self.client.get(url))?;
            let count = reviews.len();
            result.extend(reviews);
            if count < 100 {
                return Ok(result);
            }
            page += 1;
        }
    }

    pub fn team_members(&self, team: &str) -> Result<BTreeSet<String>, GithubError> {
        let (org, slug) = team
            .split_once('/')
            .ok_or_else(|| GithubError::Team(team.to_owned()))?;
        if org.is_empty() || slug.is_empty() {
            return Err(GithubError::Team(team.to_owned()));
        }
        let mut page = 1;
        let mut members = Vec::new();
        loop {
            let url = self.endpoint(&format!(
                "/orgs/{org}/teams/{slug}/members?per_page=100&page={page}"
            ));
            let page_members: Vec<User> = self.send(self.client.get(url))?;
            let count = page_members.len();
            members.extend(page_members);
            if count < 100 {
                return Ok(members.into_iter().map(|member| member.login).collect());
            }
            page += 1;
        }
    }

    pub fn has_approval(&self, group: &ApprovalGroup) -> Result<ApprovalResult, GithubError> {
        let allowed = if let Some(team) = &group.team {
            self.team_members(team)?
        } else {
            group.users.iter().cloned().collect()
        };
        let approved_by = self
            .reviews()?
            .into_iter()
            .filter(|review| {
                review.state.eq_ignore_ascii_case("approved")
                    && allowed.contains(&review.user.login)
            })
            .map(|review| review.user.login)
            .collect::<BTreeSet<_>>();
        Ok(ApprovalResult {
            satisfied: !approved_by.is_empty(),
            approved_by,
            required: allowed,
        })
    }

    pub fn upsert_comment(&self, body: &str) -> Result<CommentResult, GithubError> {
        let mut page = 1;
        let mut comments = Vec::new();
        loop {
            let url = self.endpoint(&format!(
                "/repos/{}/issues/{}/comments?per_page=100&page={page}",
                self.repository, self.pull_number
            ));
            let page_comments: Vec<Comment> = self.send(self.client.get(url))?;
            let count = page_comments.len();
            comments.extend(page_comments);
            if count < 100 {
                break;
            }
            page += 1;
        }
        let bodies = comments
            .iter()
            .map(|comment| comment.body.clone())
            .collect::<Vec<_>>();
        if let Some((index, _)) = replace_marked_comment(&bodies, body) {
            let comment = &comments[index as usize];
            let url = self.endpoint(&format!(
                "/repos/{}/issues/comments/{}",
                self.repository, comment.id
            ));
            let response: Comment = self.send(self.client.patch(url).json(&CommentBody {
                body: body.to_owned(),
            }))?;
            return Ok(CommentResult {
                id: response.id,
                updated: true,
                url: response.html_url,
            });
        }
        let url = self.endpoint(&format!(
            "/repos/{}/issues/{}/comments",
            self.repository, self.pull_number
        ));
        let response: Comment = self.send(self.client.post(url).json(&CommentBody {
            body: body.to_owned(),
        }))?;
        Ok(CommentResult {
            id: response.id,
            updated: false,
            url: response.html_url,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn send<T: for<'de> Deserialize<'de>>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, GithubError> {
        let response = request.send()?;
        let status = response.status();
        let body = response.text()?;
        if !status.is_success() {
            return Err(GithubError::Http {
                status: status.as_u16(),
                body: truncate(&body),
            });
        }
        Ok(serde_json::from_str(&body)?)
    }
}

impl ApprovalChecker for GithubApi {
    fn check(&self, group_name: &str, group: &ApprovalGroup) -> Result<ApprovalCheck, GateError> {
        let result = self
            .has_approval(group)
            .map_err(|error| GateError::ApprovalUnavailable(format!("{group_name}: {error}")))?;
        Ok(ApprovalCheck {
            group: group_name.to_owned(),
            satisfied: result.satisfied,
            approved_by: result.approved_by.into_iter().collect(),
            required: result.required.into_iter().collect(),
        })
    }
}

impl DiffProvider for GithubApi {
    fn changed_paths(&self) -> Result<Vec<ChangedPath>, DiffError> {
        self.changed_paths()
            .map_err(|error| DiffError::Git(error.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResult {
    pub satisfied: bool,
    pub approved_by: BTreeSet<String>,
    pub required: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentResult {
    pub id: u64,
    pub updated: bool,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PullFile {
    filename: String,
    status: String,
    previous_filename: Option<String>,
}
#[derive(Debug, Deserialize)]
struct Review {
    user: User,
    state: String,
}
#[derive(Debug, Deserialize)]
struct User {
    login: String,
}
#[derive(Debug, Deserialize)]
struct Comment {
    id: u64,
    body: String,
    html_url: Option<String>,
}
#[derive(Debug, Serialize)]
struct CommentBody {
    body: String,
}

fn validate_repository(repository: &str) -> Result<(), GithubError> {
    let (owner, name) = repository.split_once('/').unwrap_or(("", ""));
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return Err(GithubError::Repository(repository.to_owned()));
    }
    Ok(())
}

fn truncate(value: &str) -> String {
    value.chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::REPORT_MARKER;

    #[test]
    fn validates_repository_and_team_shapes() {
        assert!(validate_repository("owner/repo").is_ok());
        assert!(validate_repository("owner").is_err());
        assert!(validate_repository("owner/repo/extra").is_err());
        assert!(validate_repository("/repo").is_err());
        assert!(
            GithubError::Team("architects".into())
                .to_string()
                .contains("org/team")
        );
        let api = GithubApi::new("token", "owner/repo", 1).unwrap();
        assert!(matches!(
            api.team_members("org/"),
            Err(GithubError::Team(_))
        ));
    }

    #[test]
    fn marker_is_preserved_for_comment_updates() {
        assert!(crate::report::REPORT_MARKER.starts_with("<!--"));
        let comments = vec![format!("{REPORT_MARKER}\nold")];
        assert_eq!(replace_marked_comment(&comments, "new").unwrap().0, 0);
    }

    #[test]
    fn reads_files_reviews_team_members_and_creates_comment() {
        let mut server = mockito::Server::new();
        let files = server
            .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"filename":"src/a.py","status":"modified"},{"filename":"src/added.py","status":"added"},{"filename":"src/new.py","status":"renamed","previous_filename":"src/old.py"}]"#)
            .create();
        let reviews = server
            .mock("GET", "/repos/o/r/pulls/1/reviews?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"user":{"login":"alice"},"state":"APPROVED"}]"#)
            .create();
        let members = server
            .mock(
                "GET",
                "/orgs/o/teams/architects/members?per_page=100&page=1",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"login":"alice"}]"#)
            .create();
        let comments = server
            .mock("GET", "/repos/o/r/issues/1/comments?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create();
        let create = server
            .mock("POST", "/repos/o/r/issues/1/comments")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":9,"body":"new","html_url":"https://example.test/c/9"}"#)
            .create();
        let api = GithubApi::new("token", "o/r", 1)
            .unwrap()
            .with_base_url(server.url());
        let paths = api.changed_paths().unwrap();
        assert_eq!(paths[0].path, "src/a.py");
        assert!(!paths[0].is_new);
        assert!(paths[1].is_new);
        assert_eq!(paths[2].old_path.as_deref(), Some("src/old.py"));
        let approval = api
            .has_approval(&ApprovalGroup {
                team: Some("o/architects".into()),
                users: vec![],
            })
            .unwrap();
        assert!(approval.satisfied);
        assert_eq!(api.upsert_comment("new").unwrap().id, 9);
        files.assert();
        reviews.assert();
        members.assert();
        comments.assert();
        create.assert();

        let mut unauthorized_server = mockito::Server::new();
        let unauthorized_reviews = unauthorized_server
            .mock("GET", "/repos/o/r/pulls/1/reviews?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"user":{"login":"bob"},"state":"APPROVED"}]"#)
            .create();
        let unauthorized_api = GithubApi::new("token", "o/r", 1)
            .unwrap()
            .with_base_url(unauthorized_server.url());
        let unauthorized = unauthorized_api
            .has_approval(&ApprovalGroup {
                team: None,
                users: vec!["alice".into()],
            })
            .unwrap();
        assert!(!unauthorized.satisfied);
        unauthorized_reviews.assert();
    }

    #[test]
    fn paginates_reviews_and_team_members() {
        let mut server = mockito::Server::new();
        let reviews_body = format!(
            "[{}]",
            (0..100)
                .map(|i| format!(r#"{{"user":{{"login":"user{i}"}},"state":"COMMENTED"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let reviews_page_one = server
            .mock("GET", "/repos/o/r/pulls/1/reviews?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&reviews_body)
            .create();
        let reviews_page_two = server
            .mock("GET", "/repos/o/r/pulls/1/reviews?per_page=100&page=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"user":{"login":"alice"},"state":"APPROVED"}]"#)
            .create();
        let members_body = format!(
            "[{}]",
            (0..100)
                .map(|i| format!(r#"{{"login":"member{i}"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let members_page_one = server
            .mock(
                "GET",
                "/orgs/o/teams/architects/members?per_page=100&page=1",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&members_body)
            .create();
        let members_page_two = server
            .mock(
                "GET",
                "/orgs/o/teams/architects/members?per_page=100&page=2",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"login":"alice"}]"#)
            .create();
        let api = GithubApi::new("token", "o/r", 1)
            .unwrap()
            .with_base_url(server.url());
        let approval = api
            .has_approval(&ApprovalGroup {
                team: Some("o/architects".into()),
                users: vec![],
            })
            .unwrap();
        assert!(approval.satisfied);
        reviews_page_one.assert();
        reviews_page_two.assert();
        members_page_one.assert();
        members_page_two.assert();
    }

    #[test]
    fn finds_existing_report_comment_on_a_later_page() {
        let mut server = mockito::Server::new();
        let first_page = format!(
            "[{}]",
            (0..100)
                .map(|id| format!(r#"{{"id":{id},"body":"other","html_url":null}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let page_one = server
            .mock("GET", "/repos/o/r/issues/1/comments?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(&first_page)
            .create();
        let page_two = server
            .mock("GET", "/repos/o/r/issues/1/comments?per_page=100&page=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"[{{"id":7,"body":"{REPORT_MARKER}","html_url":null}}]"#
            ))
            .create();
        let update = server
            .mock("PATCH", "/repos/o/r/issues/comments/7")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":7,"body":"new","html_url":null}"#)
            .create();
        let api = GithubApi::new("token", "o/r", 1)
            .unwrap()
            .with_base_url(server.url());
        let result = api.upsert_comment("new").unwrap();
        assert!(result.updated);
        page_one.assert();
        page_two.assert();
        update.assert();
    }

    #[test]
    fn updates_existing_comment_and_reports_http_failure() {
        let mut server = mockito::Server::new();
        let comments = server
            .mock("GET", "/repos/o/r/issues/1/comments?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":7,"body":"<!-- risk-gate:report --> old","html_url":null}]"#)
            .create();
        let update = server
            .mock("PATCH", "/repos/o/r/issues/comments/7")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":7,"body":"new","html_url":null}"#)
            .create();
        let api = GithubApi::new("token", "o/r", 1)
            .unwrap()
            .with_base_url(server.url());
        assert!(api.upsert_comment("new").unwrap().updated);
        comments.assert();
        update.assert();
        let error = server
            .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=1")
            .with_status(403)
            .with_body("forbidden")
            .create();
        assert!(matches!(
            api.changed_paths(),
            Err(GithubError::Http { status: 403, .. })
        ));
        error.assert();

        let long_body = "x".repeat(600);
        let long_error = server
            .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=1")
            .with_status(500)
            .with_body(long_body)
            .create();
        if let Err(GithubError::Http { body, .. }) = api.changed_paths() {
            assert_eq!(body.chars().count(), 512);
        } else {
            panic!("expected HTTP error");
        }
        long_error.assert();
    }

    #[test]
    fn paginates_pull_request_files() {
        let mut server = mockito::Server::new();
        let first_body = format!(
            "[{}]",
            (0..100)
                .map(|index| format!(r#"{{"filename":"src/{index}.py","status":"modified"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let first = server
            .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(first_body)
            .create();
        let second = server
            .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"filename":"src/last.py","status":"modified"}]"#)
            .create();
        let api = GithubApi::new("token", "o/r", 1)
            .unwrap()
            .with_base_url(server.url());
        assert_eq!(api.changed_paths().unwrap().len(), 101);
        first.assert();
        second.assert();
    }
}
