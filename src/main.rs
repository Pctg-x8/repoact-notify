#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lambda_runtime as lambda;
use lambda::error::HandlerError;

fn main()
{
    lambda!(handler);
}

use std::collections::HashMap;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse
{
    status_code: usize, headers: HashMap<String, String>, body: String
}
#[derive(Deserialize)]
pub struct GatewayRequest { body: String }

mod github;
mod slack;

fn handler(e: GatewayRequest, ctx: lambda::Context) -> Result<GatewayResponse, HandlerError>
{
    let event: github::WebhookEvent = serde_json::from_str(&e.body)?;

    if let Some(iss) = event.issue
    {
        if let Some(cm) = event.comment { process_issue_comment(iss, cm, event.sender) }
        else { process_issue_event(event.action, iss, event.sender) }
    }
    else if let Some(pr) = event.pull_request
    {
        process_pull_request(event.action, pr, event.sender)
    }
    else { Ok("unprocessed".to_owned()) }.map(|d| GatewayResponse
    {
        status_code: 200, headers: HashMap::new(), body: d
    })
}

fn process_issue_event(action: &str, iss: github::Issue, sender: github::User) -> Result<String, HandlerError>
{
    let msg = match action
    {
        "opened" => format!(":issue-o: *{}さん* がissueを立てたよ！ :issue-o:", sender.login),
        "closed" => format!(":issue-c: *{}さん* がissueを閉じたよ :issue-c:", sender.login),
        "reopened" => format!(":issue-o: *{}さん* がissueをもう一回開いたよ :issue-o:", sender.login),
        _ => return Ok("unprocessed issue event".to_owned())
    };
    let issue_att_title = format!("#{}: {}", iss.number, iss.title);

    let mut att_fields = Vec::with_capacity(1);
    if !iss.labels.is_empty()
    {
        att_fields.push(slack::AttachmentField
        {
            title: "Labelled", short: false,
            value: iss.labels.into_iter().map(|l| l.name).collect::<Vec<_>>().join(",")
        });
    }

    slack::PostMessage
    {
        channel: slack::REPOACT_CHANNELID, as_user: true, unfurl_links: false, unfurl_media: false,
        text: &msg, attachments: vec![
            slack::Attachment
            {
                author_name: &iss.user.login, author_icon: &iss.user.avatar_url,
                author_link: &iss.user.html_url,
                title: Some(&issue_att_title), title_link: Some(&iss.html_url),
                text: &iss.body, fields: att_fields,
                color: match action
                {
                    "closed" => "#bd2c00",
                    _ => "#6cc644"
                }
            }
        ]
    }.post().map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
}

fn process_issue_comment(iss: github::Issue, cm: github::Comment, sender: github::User) -> Result<String, HandlerError>
{
    let tail_char = format!("{}{}",
        if rand::random() { "～" } else { "" },
        if rand::random() { "っ" } else { "" });
    let (issue_icon, color) = match (iss.is_pr(), &iss.state as &str)
    {
        (false, "closed") => (":issue-c:", "#bd2c00"),
        (true, "open") => (":pr:", "#4078c0"),
        (true, "closed") =>
        {
            let pr = github::query_pullrequest_is_merged(iss.number)
                .map_err(|e| failure::Error::from_boxed_compat(Box::new(e)))?;
            if pr { (":merge:", "#6e5494") } else { (":pr-closed:", "#bd2c00") }
        }
        _ => (":issue-o:", "#6cc644")
    };
    let msg = if rand::random()
    {
        format!("*{}さん* からの <{}|{icon}#{}({})> に向けた<{}|コメント>だよ{}",
            sender.login, iss.html_url, iss.number, iss.title, cm.html_url, tail_char, icon=issue_icon)
    }
    else
    {
        format!("*{}さん* が <{}|{icon}#{}({})> に<{}|コメント>したよ{}{}",
            sender.login, iss.html_url, iss.number, iss.title, cm.html_url, tail_char,
            if rand::random() { "！" } else { "" }, icon=issue_icon)
    };
    
    slack::PostMessage
    {
        channel: slack::REPOACT_CHANNELID, as_user: true, unfurl_links: false, unfurl_media: false,
        text: &msg, attachments: vec![
            slack::Attachment
            {
                author_name: &sender.login, author_icon: &sender.avatar_url, author_link: &sender.html_url,
                title: None, title_link: None, text: &cm.body, fields: Vec::new(), color
            }
        ]
    }.post().map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
}

fn process_pull_request(action: &str, pr: github::PullRequest, sender: github::User) -> Result<String, HandlerError>
{
    let msg = match (action, pr.merged)
    {
        ("opened", _) => format!(":pr: *{}さん* がPullRequestを作成したよ！ :pr:", sender.login),
        ("closed", true) => format!(":merge: *{}さん* がPullRequestをマージしたよ！ :merge:", sender.login),
        ("closed", false) => format!("*{}さん* がPullRequestを閉じたよ", sender.login),
        _ => return Ok("unprocessed pull request event".to_owned())
    };
    let att_title = format!("#{}: {}", pr.number, pr.title);

    let mut att_fields = vec![
        slack::AttachmentField
        {
            title: "Branch Flow", short: false,
            value: format!("{} => {}", pr.head.label, pr.base.label)
        }
    ];
    if !pr.labels.is_empty()
    {
        att_fields.push(slack::AttachmentField
        {
            title: "Labelled", short: false,
            value: pr.labels.into_iter().map(|l| l.name).collect::<Vec<_>>().join(",")
        });
    }

    slack::PostMessage
    {
        channel: slack::REPOACT_CHANNELID, as_user: true, unfurl_links: false, unfurl_media: false,
        text: &msg, attachments: vec![
            slack::Attachment
            {
                author_name: &pr.user.login, author_icon: &pr.user.avatar_url,
                author_link: &pr.user.html_url,
                title: Some(&att_title), title_link: Some(&pr.html_url),
                text: &pr.body, fields: att_fields,
                color: match (action, pr.merged)
                {
                    ("closed", true) => "#6e5494",
                    ("closed", false) => "#bd2c00",
                    _ => "#4078c0"
                }
            }
        ]
    }.post().map_err(|e| failure::Error::from_boxed_compat(Box::new(e)).into())
}
