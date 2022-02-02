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

fn github_api_token() -> String {
    std::env::var("GITHUB_API_TOKEN").expect("no GITHUB_API_TOKEN set")
}

fn webhook_verification_key() -> String {
    std::env::var("GITHUB_WEBHOOK_VERIFICATION_KEY").expect("no GITHUB_WEBHOOK_VERIFICATION_KEY")
}

pub async fn query_pullrequest_flags(
    repo_fullname: &str,
    number: usize,
) -> reqwest::Result<PullRequestFlags> {
    reqwest::Client::new()
        .get(&format!(
            "https://api.github.com/repos/{}/pulls/{}",
            repo_fullname, number
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("token {}", github_api_token()),
        )
        .header(
            reqwest::header::ACCEPT,
            "application/vnd.github.shadow-cat-preview+json",
        )
        .send()
        .await?
        .json()
        .await
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
