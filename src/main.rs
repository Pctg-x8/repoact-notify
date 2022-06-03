use lambda_runtime::{service_fn, Error};
use log;
use rand::seq::SliceRandom;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    lambda_runtime::run(service_fn(handler)).await
}

use std::collections::HashMap;

use crate::{route::Route, secrets::Secrets};
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse {
    status_code: usize,
    headers: HashMap<String, String>,
    body: String,
}
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequest {
    headers: GitHubWebhookHeaderValues,
    body: String,
    path_parameters: PathParameters,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct GitHubWebhookHeaderValues {
    x_hub_signature_256: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct PathParameters {
    identifiers: String,
}

mod github;
mod route;
mod secrets;
mod slack;

#[derive(Debug)]
enum ProcessError {
    InvalidWebhookSignature,
    WebhookEventParsingFailed(serde_json::Error),
    RouteNotFound(String),
}
impl std::error::Error for ProcessError {}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidWebhookSignature => write!(fmt, "Invalid Webhook signature"),
            Self::WebhookEventParsingFailed(e) => write!(fmt, "Webhook event parsing failed! {e}"),
            Self::RouteNotFound(r) => write!(fmt, "Route {r:?} is not found"),
        }
    }
}

async fn post_message(msg: slack::PostMessage<'_>, bot_token: &str) -> Result<(), Error> {
    let resp = msg.post(bot_token).await?;
    log::trace!("Post Successful! {resp:?}");
    Ok(())
}

struct ExecutionContext {
    secrets: Secrets,
    route: Route,
}
impl ExecutionContext {
    pub fn post_message<'s>(
        &'s self,
        msg: &'s str,
        modifier: impl FnOnce(slack::PostMessage<'s>) -> slack::PostMessage<'s>,
    ) -> impl std::future::Future<Output = Result<(), Error>> + 's {
        post_message(
            modifier(slack::PostMessage::new(&self.route.channel_id, msg)),
            &self.secrets.slack_bot_token,
        )
    }

    pub fn connect_github<'s>(
        &'s self,
        repo_fullpath: &'s str,
    ) -> impl std::future::Future<Output = reqwest::Result<github::ApiClient>> + 's {
        github::ApiClient::new(
            &self.secrets.github_app_id,
            &self.secrets.github_app_installation_id,
            &self.secrets.github_app_pem,
            repo_fullpath,
        )
    }
}

async fn handler(e: lambda_runtime::LambdaEvent<GatewayRequest>) -> Result<GatewayResponse, Error> {
    let secrets = Secrets::load().await?;

    if !github::verify_request(
        &e.payload.body,
        &e.payload.headers.x_hub_signature_256,
        &secrets.github_webhook_verification_secret,
    ) {
        return Err(ProcessError::InvalidWebhookSignature.into());
    }

    log::trace!("Incoming Event: {:?}", e.payload);

    let event: github::WebhookEvent =
        serde_json::from_str(&e.payload.body).map_err(ProcessError::WebhookEventParsingFailed)?;
    let route = Route::get(&e.payload.path_parameters.identifiers)
        .await?
        .ok_or_else(|| ProcessError::RouteNotFound(e.payload.path_parameters.identifiers))?;

    let ctx = ExecutionContext { secrets, route };

    if let Some(iss) = event.issue {
        if let Some(cm) = event.comment {
            process_issue_comment(ctx, iss, cm, event.repository, event.sender).await?;
        } else {
            process_issue_event(ctx, event.action, iss, event.repository, event.sender).await?;
        }
    } else if let Some(pr) = event.pull_request {
        process_pull_request(ctx, event.action, pr, event.repository, event.sender).await?;
    } else if let Some(d) = event.discussion {
        if let Some(cm) = event.comment {
            process_discussion_comment(ctx, d, cm, event.sender).await?;
        } else {
            process_discussion_event(ctx, event.action, d, event.repository, event.sender).await?;
        }
    } else {
        log::trace!("unprocessed message: {:?}", e.payload.body);
    }

    Ok(GatewayResponse {
        status_code: 200,
        headers: HashMap::new(),
        body: String::new(),
    })
}

#[derive(Debug)]
struct UnhandledDiscussionActionError(github::Action);
impl std::error::Error for UnhandledDiscussionActionError {}
impl std::fmt::Display for UnhandledDiscussionActionError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Unhandled Discussion Action: {:?}", self.0)
    }
}

const COLOR_OPEN: &'static str = "#6cc644";
const COLOR_CLOSED: &'static str = "#bd2c00";
const COLOR_DRAFT_PR: &'static str = "#6c737c";
const COLOR_OPEN_PR: &'static str = "#4078c0";
const COLOR_MERGED_PR: &'static str = "#6e5494";

async fn process_discussion_event<'s>(
    ctx: ExecutionContext,
    action: github::Action,
    d: github::Discussion<'s>,
    repo: github::Repository<'s>,
    sender: github::User<'s>,
) -> Result<(), Error> {
    let msg = match action {
        github::Action::Created => format!("*{}さん* がDiscussionを開いたよ！", sender.login),
        github::Action::Closed => format!("*{}さん* がDiscussionを閉じたよ", sender.login),
        github::Action::Reopened => format!("*{}さん* がDiscussionを再開したよ", sender.login),
        _ => return Err(UnhandledDiscussionActionError(action).into()),
    };
    let a_title = format!("[{}]#{}: {}", repo.full_name, d.number, d.title);

    let main_attachment = slack::Attachment::new(d.body.as_deref().unwrap_or(""))
        .author(&d.user.login, &d.user.html_url, &d.user.avatar_url)
        .title(&a_title, &d.html_url)
        .color(if d.state == github::DiscussionState::Closed {
            COLOR_CLOSED
        } else {
            COLOR_OPEN
        });

    ctx.post_message(&msg, |x| x.as_user().attachments(vec![main_attachment]))
        .await
}
async fn process_discussion_comment<'s>(
    ctx: ExecutionContext,
    d: github::Discussion<'s>,
    cm: github::Comment<'s>,
    sender: github::User<'s>,
) -> Result<(), Error> {
    let tail_char = format!(
        "{}{}",
        if rand::random() { "～" } else { "" },
        if rand::random() { "っ" } else { "" }
    );
    // todo: あとでアイコン変える
    let (issue_icon, color) = match d.state {
        github::DiscussionState::Closed => (":issue-c:", COLOR_CLOSED),
        github::DiscussionState::Open => (":issue-o:", COLOR_OPEN),
    };
    let msg = if rand::random() {
        format!(
            "*{}さん* からの <{}|{icon}#{}({})> に向けた<{}|コメント>だよ{}",
            sender.login,
            d.html_url,
            d.number,
            d.title,
            cm.html_url,
            tail_char,
            icon = issue_icon
        )
    } else {
        format!(
            "*{}さん* が <{}|{icon}#{}({})> に<{}|コメント>したよ{}{}",
            sender.login,
            d.html_url,
            d.number,
            d.title,
            cm.html_url,
            tail_char,
            if rand::random() { "！" } else { "" },
            icon = issue_icon
        )
    };

    let attachment = slack::Attachment::new(&cm.body)
        .author(&sender.login, &sender.html_url, &sender.avatar_url)
        .color(color);

    ctx.post_message(&msg, |x| x.as_user().attachments(vec![attachment]))
        .await
}

#[derive(Debug)]
pub struct UnhandledIssueActionError(github::Action);
impl std::error::Error for UnhandledIssueActionError {}
impl std::fmt::Display for UnhandledIssueActionError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Unhandled issue action: {:?}", self.0)
    }
}

async fn process_issue_event<'s>(
    ctx: ExecutionContext,
    action: github::Action,
    iss: github::Issue<'s>,
    repo: github::Repository<'s>,
    sender: github::User<'s>,
) -> Result<(), Error> {
    let msg = match action {
        github::Action::Opened => format!(
            ":issue-o: *{}さん* がissueを立てたよ！ :issue-o:",
            sender.login
        ),
        github::Action::Closed => format!(
            ":issue-c: *{}さん* がissueを閉じたよ :issue-c:",
            sender.login
        ),
        github::Action::Reopened => format!(
            ":issue-o: *{}さん* がissueをもう一回開いたよ :issue-o:",
            sender.login
        ),
        _ => return Err(UnhandledIssueActionError(action).into()),
    };
    let issue_att_title = format!("[{}]#{}: {}", repo.full_name, iss.number, iss.title);

    let mut att_fields = Vec::with_capacity(1);
    if !iss.labels.is_empty() {
        att_fields.push(slack::AttachmentField {
            title: "Labelled",
            short: false,
            value: iss
                .labels
                .into_iter()
                .map(|l| l.name)
                .collect::<Vec<_>>()
                .join(","),
        });
    }
    let attachment = slack::Attachment::new(iss.body.as_deref().unwrap_or(""))
        .author(&iss.user.login, &iss.user.html_url, &iss.user.avatar_url)
        .title(&issue_att_title, &iss.html_url)
        .color(if iss.state == github::IssueState::Closed {
            COLOR_CLOSED
        } else {
            COLOR_OPEN
        })
        .fields(att_fields);

    ctx.post_message(&msg, |x| x.as_user().attachments(vec![attachment]))
        .await
}

async fn process_issue_comment<'s>(
    ctx: ExecutionContext,
    iss: github::Issue<'s>,
    cm: github::Comment<'s>,
    repo: github::Repository<'s>,
    sender: github::User<'s>,
) -> Result<(), Error> {
    let tail_char = format!(
        "{}{}",
        if rand::random() { "～" } else { "" },
        if rand::random() { "っ" } else { "" }
    );
    let (issue_icon, color) = match (iss.is_pr(), iss.state) {
        (false, github::IssueState::Closed) => (":issue-c:", COLOR_CLOSED),
        (true, github::IssueState::Open) => {
            let pr = ctx
                .connect_github(&repo.full_name)
                .await?
                .query_pullrequest_flags(iss.number)
                .await?;
            if pr.draft {
                (":pr-draft:", COLOR_DRAFT_PR)
            } else {
                (":pr:", COLOR_OPEN_PR)
            }
        }
        (true, github::IssueState::Closed) => {
            let pr = ctx
                .connect_github(&repo.full_name)
                .await?
                .query_pullrequest_flags(iss.number)
                .await?;
            if pr.merged {
                (":merge:", COLOR_MERGED_PR)
            } else {
                (":pr-closed:", COLOR_CLOSED)
            }
        }
        _ => (":issue-o:", COLOR_OPEN),
    };
    let msg = if rand::random() {
        format!(
            "*{}さん* からの <{}|{icon}#{}({})> に向けた<{}|コメント>だよ{}",
            sender.login,
            iss.html_url,
            iss.number,
            iss.title,
            cm.html_url,
            tail_char,
            icon = issue_icon
        )
    } else {
        format!(
            "*{}さん* が <{}|{icon}#{}({})> に<{}|コメント>したよ{}{}",
            sender.login,
            iss.html_url,
            iss.number,
            iss.title,
            cm.html_url,
            tail_char,
            if rand::random() { "！" } else { "" },
            icon = issue_icon
        )
    };

    let attachment = slack::Attachment::new(&cm.body)
        .author(&sender.login, &sender.html_url, &sender.avatar_url)
        .color(color);

    ctx.post_message(&msg, |x| x.as_user().attachments(vec![attachment]))
        .await
}

#[derive(Debug)]
pub struct UnhandledPullRequestActionError(github::Action);
impl std::error::Error for UnhandledPullRequestActionError {}
impl std::fmt::Display for UnhandledPullRequestActionError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Unhandled issue action: {:?}", self.0)
    }
}

async fn process_pull_request<'s>(
    ctx: ExecutionContext,
    action: github::Action,
    pr: github::PullRequest<'s>,
    repo: github::Repository<'s>,
    sender: github::User<'s>,
) -> Result<(), Error> {
    let merged = match pr.merged {
        Some(m) => m,
        None => {
            ctx.connect_github(&repo.full_name)
                .await?
                .query_pullrequest_flags(pr.number)
                .await?
                .merged
        }
    };
    let msg_base = match (action, merged, pr.draft)
    {
        (github::Action::ReadyForReview, _, _) =>
            format!(":pr: *{}さん* の <{}|:pr-draft:#{}: {}> がレビューできるようになったよ！よろしくね！ :pr:", sender.login,
                pr.html_url, pr.number, pr.title),
        (github::Action::Opened, _, true) => format!(":pr-draft: *{}さん* がPullRequestを作成したよ！ :pr-draft:", sender.login),
        (github::Action::Reopened, _,true) => format!(":pr-draft: *{}さん* がPullRequestを開き直したよ！ :pr-draft:", sender.login),
        (github::Action::Opened, _, false) => format!(":pr: *{}さん* がPullRequestを作成したよ！ :pr:", sender.login),
        (github::Action::Reopened, _, false) => format!(":pr: *{}さん* がPullRequestを開き直したよ！ :pr:", sender.login),
        (github::Action::Closed, true, _) => format!(":merge: *{}さん* がPullRequestをマージしたよ！ :merge:", sender.login),
        (github::Action::Closed, false, _) => format!("*{}さん* がPullRequestを閉じたよ", sender.login),
        _ => return Err(UnhandledPullRequestActionError(action).into())
    };
    let att_title = format!("[{}]#{}: {}", repo.full_name, pr.number, pr.title);
    let draft_msg = if pr.draft && action == github::Action::Opened {
        [
            "\nこのPRはまだドラフト状態だよ！",
            "\nこのPRはまだドラフト状態みたい。",
            "\n作業中のPRだね！",
            "\nまだ作業中みたいだから、マージはもうちょっと待ってね。",
        ]
        .choose(&mut rand::thread_rng())
        .unwrap()
    } else {
        ""
    };
    let msg = format!("{}{}", msg_base, draft_msg);

    let branch_flow_name = detect_branch_flow(
        pr.head
            .label
            .splitn(2, ":")
            .nth(1)
            .unwrap_or(&pr.head.label),
        pr.base
            .label
            .splitn(2, ":")
            .nth(1)
            .unwrap_or(&pr.base.label),
    );
    let mut att_fields = vec![slack::AttachmentField {
        title: "Branch Flow",
        short: false,
        value: format!(
            "{} ({} => {})",
            branch_flow_name, pr.head.label, pr.base.label
        ),
    }];
    if !pr.labels.is_empty() {
        att_fields.push(slack::AttachmentField {
            title: "Labelled",
            short: false,
            value: pr
                .labels
                .into_iter()
                .map(|l| l.name)
                .collect::<Vec<_>>()
                .join(","),
        });
    }

    let attachment = slack::Attachment::new(pr.body.as_deref().unwrap_or(""))
        .author(&pr.user.login, &pr.user.html_url, &pr.user.avatar_url)
        .title(&att_title, &pr.html_url)
        .fields(att_fields)
        .color(match (action, merged, pr.draft) {
            (github::Action::Closed, true, _) => COLOR_MERGED_PR, // merged pr
            (github::Action::Closed, false, _) => COLOR_CLOSED,   // unmerged but closed pr
            (_, _, true) => COLOR_DRAFT_PR,                       // draft pr
            _ => COLOR_OPEN_PR,                                   // opened pr
        });

    ctx.post_message(&msg, |x| x.as_user().attachments(vec![attachment]))
        .await
}

fn detect_branch_flow(head: &str, base: &str) -> &'static str {
    if head.starts_with("ft-") {
        // feature merging flow
        match base {
            "dev" => "Stable Promotion",
            "master" => "<Illegal Flow>",
            b if b.starts_with("dev-") => "Stable Promotion",
            _ => "?",
        }
    } else if head.starts_with("fix-") {
        // hotfix merging flow
        match base {
            "dev" => "Fixes Promotion",
            "master" => "Emergent Patching",
            b if b.starts_with("dev-") => "Fixes Promotion",
            _ => "?",
        }
    } else if head == "dev" || head.starts_with("dev-") {
        // development merging flow
        match base {
            "master" => "Release Promotion",
            _ => "Delivering",
        }
    } else if head == "master" {
        // master merging flow
        if base == "dev" || base.starts_with("dev-") {
            "Delivering"
        } else {
            "<Illegal Flow>"
        }
    } else {
        "?"
    }
}
