
use std::borrow::Cow;

fn default_bool_false() -> bool { false }

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
    pub body: Option<Cow<'s, str>>,
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
    pub body: Option<Cow<'s, str>>,
    #[serde(borrow = "'s")]
    pub head: RefExt<'s>,
    #[serde(borrow = "'s")]
    pub base: RefExt<'s>,
    pub merged: Option<bool>,
    #[serde(default = "default_bool_false")]
    pub draft: bool,
    #[serde(borrow = "'s")]
    pub labels: Vec<Label<'s>>
}
#[derive(Deserialize)]
pub struct PullRequestFlags
{
    pub merged: bool,
    #[serde(default = "default_bool_false")]
    pub draft: bool
}

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

pub fn query_pullrequest_flags(number: usize) -> reqwest::Result<PullRequestFlags>
{
    reqwest::Client::new().get(&format!("https://api.github.com/repos/Pctg-x8/peridot/pulls/{}", number))
        .header(reqwest::header::AUTHORIZATION, concat!("token ", env!("GITHUB_API_TOKEN")))
        .header(reqwest::header::ACCEPT, "application/vnd.github.shadow-cat-preview+json")
        .send()?.json()
}
