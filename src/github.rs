use std::borrow::Cow;

fn default_bool_false() -> bool {
    false
}

#[derive(serde::Deserialize)]
pub struct User<'s> {
    #[serde(borrow = "'s")]
    pub login: Cow<'s, str>,
    pub id: u64,
    #[serde(borrow = "'s")]
    pub avatar_url: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
}
#[derive(serde::Deserialize)]
pub struct Label<'s> {
    #[serde(borrow = "'s")]
    pub name: &'s str,
    #[serde(borrow = "'s")]
    pub url: &'s str,
}
#[derive(serde::Deserialize)]
pub struct IssuePullRequestInfo<'s> {
    #[serde(borrow = "'s")]
    #[allow(dead_code)]
    html_url: Cow<'s, str>,
}

#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(serde::Deserialize)]
pub struct Issue<'s> {
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
    pub number: usize,
    #[serde(borrow = "'s")]
    pub title: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub user: User<'s>,
    #[serde(borrow = "'s")]
    pub labels: Vec<Label<'s>>,
    #[serde(borrow = "'s")]
    pub body: Option<Cow<'s, str>>,
    pub state: IssueState,
    #[serde(borrow = "'s")]
    pub pull_request: Option<IssuePullRequestInfo<'s>>,
}
impl<'s> Issue<'s> {
    #[inline(always)]
    pub fn is_pr(&self) -> bool {
        self.pull_request.is_some()
    }

    #[inline(always)]
    pub fn is_closed(&self) -> bool {
        self.state == IssueState::Closed
    }
}

#[derive(serde::Deserialize)]
pub struct Comment<'s> {
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub user: User<'s>,
    #[serde(borrow = "'s")]
    pub body: Cow<'s, str>,
}
#[derive(serde::Deserialize)]
pub struct Repository<'s> {
    #[serde(borrow = "'s")]
    pub full_name: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
}
#[derive(serde::Deserialize)]
pub struct RefExt<'s> {
    #[serde(borrow = "'s")]
    pub label: Cow<'s, str>,
}
#[derive(serde::Deserialize)]
pub struct PullRequest<'s> {
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
    pub number: usize,
    #[serde(borrow = "'s")]
    pub title: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub user: User<'s>,
    #[serde(borrow = "'s")]
    pub body: Option<Cow<'s, str>>,
    #[serde(borrow = "'s")]
    pub head: RefExt<'s>,
    #[serde(borrow = "'s")]
    pub base: RefExt<'s>,
    pub merged: Option<bool>,
    #[serde(default = "default_bool_false")]
    pub draft: bool,
    #[serde(borrow = "'s")]
    pub labels: Vec<Label<'s>>,
}
#[derive(serde::Deserialize)]
pub struct PullRequestFlags {
    pub merged: bool,
    #[serde(default = "default_bool_false")]
    pub draft: bool,
}

#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DiscussionState {
    Open,
    Closed,
}

#[derive(serde::Deserialize)]
pub struct DiscussionCategory<'s> {
    pub emoji: &'s str,
    #[serde(borrow = "'s")]
    pub name: Cow<'s, str>,
    pub is_answerable: bool,
}

#[derive(serde::Deserialize)]
pub struct Discussion<'s> {
    #[serde(borrow = "'s")]
    pub category: Option<DiscussionCategory<'s>>,
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
    pub number: usize,
    #[serde(borrow = "'s")]
    pub title: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub user: User<'s>,
    pub state: DiscussionState,
    #[serde(borrow = "'s")]
    pub body: Option<Cow<'s, str>>,
}
impl Discussion<'_> {
    #[inline(always)]
    pub fn is_closed(&self) -> bool {
        self.state == DiscussionState::Closed
    }
}

#[derive(serde::Deserialize)]
pub struct WorkflowJob<'s> {
    pub run_url: &'s str,
    pub workflow_name: &'s str,
    pub name: &'s str,
    pub head_sha: &'s str,
    pub head_branch: &'s str,
    pub run_id: u64,
}
impl WorkflowJob<'_> {
    pub async fn run_details(&self) -> reqwest::Result<WorkflowRun> {
        ApiClient::unauthorized_get_request(self.run_url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .await?
            .json()
            .await
    }
}

pub fn workflow_run_html_url(job: &WorkflowJob, repository: &Repository) -> String {
    format!(
        "https://github.com/{}/actions/runs/{}",
        repository.full_name, job.run_id
    )
}

pub fn commit_html_url(repository: &Repository, sha: &str) -> String {
    format!("https://github.com/{}/commit/{}", repository.full_name, sha)
}

#[derive(serde::Deserialize)]
pub struct DeploymentInfo<'s> {
    pub url: &'s str,
    pub environment: &'s str,
}

#[derive(serde::Deserialize)]
pub struct WebhookEvent<'s> {
    pub action: Action,
    #[serde(borrow = "'s")]
    pub sender: User<'s>,
    #[serde(borrow = "'s")]
    pub issue: Option<Issue<'s>>,
    #[serde(borrow = "'s")]
    pub comment: Option<Comment<'s>>,
    #[serde(borrow = "'s")]
    pub pull_request: Option<PullRequest<'s>>,
    #[serde(borrow = "'s")]
    pub discussion: Option<Discussion<'s>>,
    #[serde(borrow = "'s")]
    pub repository: Repository<'s>,
    pub workflow_job: Option<WorkflowJob<'s>>,
    pub deployment: Option<DeploymentInfo<'s>>,
}

#[derive(serde::Deserialize)]
pub struct WorkflowRun {
    pub run_number: u64,
}

#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Opened,
    Closed,
    Reopened,
    Created,
    ReadyForReview,
    Waiting,
}

pub struct ApiClient<'s> {
    token: String,
    repo_fullname: &'s str,
}
impl<'s> ApiClient<'s> {
    pub async fn new(
        app_id_str: &str,
        installation_id_str: &str,
        private_key_pem: &str,
        repo_fullname: &'s str,
    ) -> reqwest::Result<ApiClient<'s>> {
        #[derive(serde::Serialize)]
        struct Payload<'s> {
            iat: usize,
            exp: usize,
            iss: &'s str,
        }
        #[derive(serde::Serialize)]
        struct BodyParameters<'s> {
            repository: &'s str,
        }
        #[derive(serde::Deserialize)]
        struct Response {
            token: String,
        }

        let key =
            jsonwebtoken::EncodingKey::from_rsa_pem(private_key_pem.as_bytes()).expect("Failed to load github pkey");
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let nowtime = time::OffsetDateTime::now_utc().unix_timestamp() as usize;
        let payload = Payload {
            iat: nowtime - 60,
            exp: nowtime + 10 * 60,
            iss: app_id_str,
        };
        let token = jsonwebtoken::encode(&header, &payload, &key).expect("Failed to encode jwt");

        let Response { token } = reqwest::Client::new()
            .post(&format!(
                "https://api.github.com/app/installations/{installation_id_str}/access_tokens"
            ))
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .header(reqwest::header::USER_AGENT, "koyuki/repoact-notify")
            .json(&BodyParameters {
                repository: repo_fullname,
            })
            .send()
            .await?
            .json()
            .await?;

        Ok(Self { token, repo_fullname })
    }

    fn unauthorized_get_request(url: impl reqwest::IntoUrl) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .get(url)
            .header(reqwest::header::USER_AGENT, "koyuki/repoact-notify")
    }

    fn authorized_get_request(&self, url: impl reqwest::IntoUrl) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .get(url)
            .header(reqwest::header::AUTHORIZATION, format!("token {}", self.token))
            .header(reqwest::header::USER_AGENT, "koyuki/repoact-notify")
    }

    fn authorized_post_request(&self, url: impl reqwest::IntoUrl) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .post(url)
            .header(reqwest::header::AUTHORIZATION, format!("bearer {}", self.token))
            .header(reqwest::header::USER_AGENT, "koyuki/repoact-notify")
    }

    pub async fn query_pullrequest_flags(&self, number: usize) -> reqwest::Result<PullRequestFlags> {
        let url = format!("https://api.github.com/repos/{}/pulls/{number}", self.repo_fullname);

        self.authorized_get_request(url)
            .header(
                reqwest::header::ACCEPT,
                "application/vnd.github.shadow-cat-preview+json",
            )
            .send()
            .await?
            .json()
            .await
    }
}

pub fn verify_request(payload: &str, signature: &str, key: &str) -> bool {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key.as_bytes());
    let signature_decoded = signature.as_bytes()[b"sha256=".len()..]
        .chunks_exact(2)
        .map(|cs| {
            let h = match cs[0] {
                c @ b'0'..=b'9' => c - b'0',
                c @ b'a'..=b'f' => (c - b'a') + 0x0a,
                c @ b'A'..=b'F' => (c - b'A') + 0x0a,
                _ => unreachable!("invalid input signature"),
            };
            let l = match cs[1] {
                c @ b'0'..=b'9' => c - b'0',
                c @ b'a'..=b'f' => (c - b'a') + 0x0a,
                c @ b'A'..=b'F' => (c - b'A') + 0x0a,
                _ => unreachable!("invalid input signature"),
            };

            (h << 4) | l
        })
        .collect::<Vec<_>>();

    ring::hmac::verify(&key, payload.as_bytes(), &signature_decoded).is_ok()
}

pub mod graphql;
