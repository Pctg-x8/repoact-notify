use lambda_runtime::{handler_fn, Context, Error};
use rand::seq::SliceRandom;
#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    lambda_runtime::run(handler_fn(handler)).await
}

use std::collections::HashMap;
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse {
    status_code: usize,
    headers: HashMap<String, String>,
    body: String,
}
#[derive(serde::Deserialize)]
pub struct GatewayRequest {
    body: String,
}

mod github;
mod slack;

async fn handler(e: GatewayRequest, _ctx: Context) -> Result<GatewayResponse, Error> {
    let event: github::WebhookEvent = serde_json::from_str(&e.body)?;

    if let Some(iss) = event.issue {
        if let Some(cm) = event.comment {
            process_issue_comment(iss, cm, event.sender)
        } else {
            process_issue_event(event.action, iss, event.repository, event.sender)
        }
    } else if let Some(pr) = event.pull_request {
        process_pull_request(event.action, pr, event.repository, event.sender)
    } else if let Some(d) = event.discussion {
        if let Some(cm) = event.comment {
            process_discussion_comment(d, cm, event.sender)
        } else {
            process_discussion_event(event.action, d, event.repository, event.sender)
        }
    } else {
        Ok("unprocessed".to_owned())
    }
    .map(|d| {
        info!("Delivering Successful! Response: {}", d);
        GatewayResponse {
            status_code: 200,
            headers: HashMap::new(),
            body: d,
        }
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

fn process_discussion_event(
    action: github::Action,
    d: github::Discussion,
    repo: github::Repository,
    sender: github::User,
) -> Result<String, Error> {
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
        .color(match action {
            github::Action::Closed => COLOR_CLOSED,
            _ => COLOR_OPEN,
        });
    slack::PostMessage::new(slack::REPOACT_CHANNELID, &msg)
        .as_user()
        .attachments(vec![main_attachment])
        .post()
        .map_err(From::from)
}
fn process_discussion_comment(
    d: github::Discussion,
    cm: github::Comment,
    sender: github::User,
) -> Result<String, Error> {
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
    slack::PostMessage::new(slack::REPOACT_CHANNELID, &msg)
        .as_user()
        .attachments(vec![attachment])
        .post()
        .map_err(From::from)
}

fn process_issue_event(
    action: github::Action,
    iss: github::Issue,
    repo: github::Repository,
    sender: github::User,
) -> Result<String, Error> {
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
        _ => return Ok("unprocessed issue event".to_owned()),
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
        .color(match action {
            github::Action::Closed => COLOR_CLOSED,
            _ => COLOR_OPEN,
        })
        .fields(att_fields);
    slack::PostMessage::new(slack::REPOACT_CHANNELID, &msg)
        .as_user()
        .attachments(vec![attachment])
        .post()
        .map_err(From::from)
}

fn process_issue_comment(
    iss: github::Issue,
    cm: github::Comment,
    sender: github::User,
) -> Result<String, Error> {
    let tail_char = format!(
        "{}{}",
        if rand::random() { "～" } else { "" },
        if rand::random() { "っ" } else { "" }
    );
    let (issue_icon, color) = match (iss.is_pr(), &iss.state as &str) {
        (false, "closed") => (":issue-c:", COLOR_CLOSED),
        (true, "open") => {
            let pr = github::query_pullrequest_flags(iss.number)
                .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))?;
            if pr.draft {
                (":pr-draft:", COLOR_DRAFT_PR)
            } else {
                (":pr:", COLOR_OPEN_PR)
            }
        }
        (true, "closed") => {
            let pr = github::query_pullrequest_flags(iss.number)
                .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))?;
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
    slack::PostMessage::new(slack::REPOACT_CHANNELID, &msg)
        .as_user()
        .attachments(vec![attachment])
        .post()
        .map_err(From::from)
}

fn process_pull_request(
    action: github::Action,
    pr: github::PullRequest,
    repo: github::Repository,
    sender: github::User,
) -> Result<String, Error> {
    let merged = pr.merged.map(Ok).unwrap_or_else(|| {
        github::query_pullrequest_flags(pr.number)
            .map(|s| s.merged)
            .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))
    })?;
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
        _ => return Ok("unprocessed pull request event".to_owned())
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
            (github::Action::Closed, false, _) => COLOR_CLOSED, // unmerged but closed pr
            (_, _, true) => COLOR_DRAFT_PR,                      // draft pr
            _ => COLOR_OPEN_PR,                                 // opened pr
        });
    slack::PostMessage::new(slack::REPOACT_CHANNELID, &msg)
        .as_user()
        .attachments(vec![attachment])
        .post()
        .map_err(From::from)
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
