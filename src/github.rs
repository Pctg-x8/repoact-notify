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
    pub fn is_pr(&self) -> bool {
        self.pull_request.is_some()
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
}

#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Opened,
    Closed,
    Reopened,
    Created,
    ReadyForReview,
}

fn webhook_verification_key() -> String {
    std::env::var("GITHUB_WEBHOOK_VERIFICATION_KEY").expect("no GITHUB_WEBHOOK_VERIFICATION_KEY")
}

pub fn app_id() -> usize {
    std::env::var("GITHUB_APP_ID")
        .expect("no GITHUB_APP_ID set")
        .parse()
        .expect("invalid app id")
}

pub fn installation_id() -> usize {
    std::env::var("GITHUB_APP_INSTALLATION_ID")
        .expect("no GITHUB_APP_INSTALLATION_ID set")
        .parse()
        .expect("invalid installation id")
}

const APP_PRIVATE_KEY: &'static [u8] = include_bytes!("../pkey.pem");

pub struct ApiClient<'s> {
    token: String,
    repo_fullname: &'s str,
}
impl<'s> ApiClient<'s> {
    pub async fn new(
        app_id: usize,
        installation_id: usize,
        repo_fullname: &'s str,
    ) -> reqwest::Result<ApiClient<'s>> {
        #[derive(serde::Serialize)]
        struct Payload {
            iat: usize,
            exp: usize,
            iss: String,
        }
        #[derive(serde::Serialize)]
        struct BodyParameters<'s> {
            repository: &'s str,
        }
        #[derive(serde::Deserialize)]
        struct Response {
            token: String,
        }

        let key = jsonwebtoken::EncodingKey::from_rsa_pem(APP_PRIVATE_KEY)
            .expect("Failed to load github pkey");
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let nowtime = time::OffsetDateTime::now_utc().unix_timestamp() as usize;
        let payload = Payload {
            iat: nowtime - 60,
            exp: nowtime + 10 * 60,
            iss: app_id.to_string(),
        };
        let token = jsonwebtoken::encode(&header, &payload, &key).expect("Failed to encode jwt");

        let resp = reqwest::Client::new()
            .post(&format!(
                "https://api.github.com/app/installations/{installation_id}/access_tokens"
            ))
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .header(reqwest::header::USER_AGENT, "koyuki/repoact-notify")
            .json(&BodyParameters {
                repository: repo_fullname,
            })
            .send()
            .await?
            .text()
            .await?;
        log::trace!("access tokens response: {resp}");
        let Response { token } = serde_json::from_str(&resp).expect("Failed to parse json");

        Ok(Self {
            token,
            repo_fullname,
        })
    }

    pub async fn query_pullrequest_flags(
        &self,
        number: usize,
    ) -> reqwest::Result<PullRequestFlags> {
        reqwest::Client::new()
            .get(&format!(
                "https://api.github.com/repos/{}/pulls/{number}",
                self.repo_fullname
            ))
            .header(
                reqwest::header::AUTHORIZATION,
                format!("token {}", self.token),
            )
            .header(
                reqwest::header::ACCEPT,
                "application/vnd.github.shadow-cat-preview+json",
            )
            .header(reqwest::header::USER_AGENT, "koyuki/repoact-notify")
            .send()
            .await?
            .json()
            .await
    }
}

pub fn verify_request(payload: &str, signature: &str) -> bool {
    let key = ring::hmac::Key::new(
        ring::hmac::HMAC_SHA256,
        webhook_verification_key().as_bytes(),
    );
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
