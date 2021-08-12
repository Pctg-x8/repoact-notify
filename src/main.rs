#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lambda_runtime as lambda;
use lambda::error::HandlerError;
use rand::seq::SliceRandom;
#[macro_use]
extern crate log;

fn main() {
    simple_logger::init_with_level(log::Level::Info).expect("initializing simple_logger");
    lambda!(handler);
}

use std::collections::HashMap;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse {
    status_code: usize,
    headers: HashMap<String, String>,
    body: String,
}
#[derive(Deserialize)]
pub struct GatewayRequest {
    body: String,
}

mod github;
mod slack;

fn handler(e: GatewayRequest, _ctx: lambda::Context) -> Result<GatewayResponse, HandlerError> {
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
        process_discussion_event(event.action, d, event.repository, event.sender)
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

fn process_discussion_event(
    action: github::Action,
    d: github::Discussion,
    repo: github::Repository,
    sender: github::User,
) -> Result<String, HandlerError> {
    let msg = match action {
        github::Action::Created => format!("*{}さん* がDiscussionを開いたよ！", sender.login),
        github::Action::Closed => format!("*{}さん* がDiscussionを閉じたよ", sender.login),
        github::Action::Reopened => format!("*{}さん* がDiscussionを再開したよ", sender.login),
        _ => {
            return Err(failure::Error::from_boxed_compat(Box::new(
                UnhandledDiscussionActionError(action),
            ))
            .into())
        }
    };
    let a_title = format!("[{}]#{}: {}", repo.full_name, d.number, d.title);

    slack::PostMessage {
        channel: slack::REPOACT_CHANNELID,
        as_user: true,
        unfurl_links: false,
        unfurl_media: false,
        text: &msg,
        attachments: vec![slack::Attachment {
            author_name: &d.user.login,
            author_icon: &d.user.avatar_url,
            author_link: &d.user.html_url,
            title: Some(&a_title),
            title_link: Some(&d.html_url),
            text: d.body.as_deref().unwrap_or(""),
            fields: Vec::new(),
            color: match action {
                github::Action::Closed => "#bd2c00",
                _ => "#6cc644",
            },
        }],
    }
    .post()
    .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
}

fn process_issue_event(
    action: github::Action,
    iss: github::Issue,
    repo: github::Repository,
    sender: github::User,
) -> Result<String, HandlerError> {
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

    slack::PostMessage {
        channel: slack::REPOACT_CHANNELID,
        as_user: true,
        unfurl_links: false,
        unfurl_media: false,
        text: &msg,
        attachments: vec![slack::Attachment {
            author_name: &iss.user.login,
            author_icon: &iss.user.avatar_url,
            author_link: &iss.user.html_url,
            title: Some(&issue_att_title),
            title_link: Some(&iss.html_url),
            text: iss.body.as_ref().map(|s| s as &str).unwrap_or(""),
            fields: att_fields,
            color: match action {
                github::Action::Closed => "#bd2c00",
                _ => "#6cc644",
            },
        }],
    }
    .post()
    .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
}

fn process_issue_comment(
    iss: github::Issue,
    cm: github::Comment,
    sender: github::User,
) -> Result<String, HandlerError> {
    let tail_char = format!(
        "{}{}",
        if rand::random() { "～" } else { "" },
        if rand::random() { "っ" } else { "" }
    );
    let (issue_icon, color) = match (iss.is_pr(), &iss.state as &str) {
        (false, "closed") => (":issue-c:", "#bd2c00"),
        (true, "open") => {
            let pr = github::query_pullrequest_flags(iss.number)
                .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))?;
            if pr.draft {
                (":pr-draft:", "#6c737c")
            } else {
                (":pr:", "#4078c0")
            }
        }
        (true, "closed") => {
            let pr = github::query_pullrequest_flags(iss.number)
                .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))?;
            if pr.merged {
                (":merge:", "#6e5494")
            } else {
                (":pr-closed:", "#bd2c00")
            }
        }
        _ => (":issue-o:", "#6cc644"),
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

    slack::PostMessage {
        channel: slack::REPOACT_CHANNELID,
        as_user: true,
        unfurl_links: false,
        unfurl_media: false,
        text: &msg,
        attachments: vec![slack::Attachment {
            author_name: &sender.login,
            author_icon: &sender.avatar_url,
            author_link: &sender.html_url,
            title: None,
            title_link: None,
            text: &cm.body,
            fields: Vec::new(),
            color,
        }],
    }
    .post()
    .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
}

fn process_pull_request(
    action: github::Action,
    pr: github::PullRequest,
    repo: github::Repository,
    sender: github::User,
) -> Result<String, HandlerError> {
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

    slack::PostMessage {
        channel: slack::REPOACT_CHANNELID,
        as_user: true,
        unfurl_links: false,
        unfurl_media: false,
        text: &msg,
        attachments: vec![slack::Attachment {
            author_name: &pr.user.login,
            author_icon: &pr.user.avatar_url,
            author_link: &pr.user.html_url,
            title: Some(&att_title),
            title_link: Some(&pr.html_url),
            text: pr.body.as_ref().map(|s| s as &str).unwrap_or(""),
            fields: att_fields,
            color: match (action, merged, pr.draft) {
                (github::Action::Closed, true, _) => "#6e5494", // merged pr
                (github::Action::Closed, false, _) => "#bd2c00", // unmerged but closed pr
                (_, _, true) => "#6c737c",                      // draft pr
                _ => "#4078c0",                                 // opened pr
            },
        }],
    }
    .post()
    .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
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
