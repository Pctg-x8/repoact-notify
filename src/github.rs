
use std::borrow::Cow;

#[derive(Deserialize)]
pub struct User<'s>
{
    #[serde(borrow = "'s")]
    pub login: Cow<'s, str>,
    pub id: u64,
    #[serde(borrow = "'s")]
    pub avatar_url: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>
}
#[derive(Deserialize)]
pub struct Label<'s>
{
    #[serde(borrow = "'s")]
    pub name: &'s str,
    #[serde(borrow = "'s")]
    pub url: &'s str
}
#[derive(Deserialize)]
pub struct IssuePullRequestInfo<'s>
{
    #[serde(borrow = "'s")] #[allow(dead_code)]
    html_url: Cow<'s, str>
}
#[derive(Deserialize)]
pub struct Issue<'s>
{
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
    pub body: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub state: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub pull_request: Option<IssuePullRequestInfo<'s>>
}
impl<'s> Issue<'s>
{
    pub fn is_pr(&self) -> bool { self.pull_request.is_some() }
}
#[derive(Deserialize)]
pub struct Comment<'s>
{
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub user: User<'s>,
    #[serde(borrow = "'s")]
    pub body: Cow<'s, str>
}
#[derive(Deserialize)]
pub struct Repository<'s>
{
    #[serde(borrow = "'s")]
    pub full_name: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>
}
#[derive(Deserialize)]
pub struct RefExt<'s>
{
    #[serde(borrow = "'s")]
    pub label: Cow<'s, str>
}
#[derive(Deserialize)]
pub struct PullRequest<'s>
{
    #[serde(borrow = "'s")]
    pub html_url: Cow<'s, str>,
    pub number: usize,
    #[serde(borrow = "'s")]
    pub title: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub user: User<'s>,
    #[serde(borrow = "'s")]
    pub body: Cow<'s, str>,
    #[serde(borrow = "'s")]
    pub head: RefExt<'s>,
    #[serde(borrow = "'s")]
    pub base: RefExt<'s>,
    pub merged: bool,
    #[serde(borrow = "'s")]
    pub labels: Vec<Label<'s>>
}
#[derive(Deserialize)]
struct PullRequestIsMerged { merged: bool }

#[derive(Deserialize)]
pub struct WebhookEvent<'s>
{
    pub action: &'s str,
    #[serde(borrow = "'s")]
    pub sender: User<'s>,
    #[serde(borrow = "'s")]
    pub issue: Option<Issue<'s>>,
    #[serde(borrow = "'s")]
    pub comment: Option<Comment<'s>>,
    #[serde(borrow = "'s")]
    pub pull_request: Option<PullRequest<'s>>,
    #[serde(borrow = "'s")]
    pub repository: Repository<'s>
}

fn api_key() -> String { std::env::var("GITHUB_API_TOKEN").expect("GitHub API Token not setted") }
pub fn query_pullrequest_is_merged(number: usize) -> reqwest::Result<bool>
{
    reqwest::Client::new().get(&format!("https://api.github.com/repos/Pctg-x8/peridot/pulls/{}", number))
        .header(reqwest::header::AUTHORIZATION, format!("token {}", api_key()))
        .send()?.json::<PullRequestIsMerged>().map(|m| m.merged)
}
