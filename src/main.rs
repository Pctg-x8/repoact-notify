use futures::TryFutureExt;
use lambda_runtime::{service_fn, Error};
use rand::seq::SliceRandom;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().json())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    lambda_runtime::run(service_fn(handler)).await
}

use std::collections::HashMap;

use repoact_notify_common::{slack, Route};

use crate::secrets::Secrets;
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
mod secrets;

#[derive(Debug, thiserror::Error)]
enum ProcessError {
    #[error("Invalid Webhook signature")]
    InvalidWebhookSignature,
    #[error("Webhook event parsing failed! {0}")]
    WebhookEventParsingFailed(serde_json::Error),
    #[error("Route {0:?} is not found")]
    RouteNotFound(String),
    #[error("Field {0:?} is not contained in the payload")]
    RequireField(&'static str),
}

async fn post_message(msg: slack::PostMessage<'_>, bot_token: &str) -> Result<(), Error> {
    let resp = msg.post(bot_token).await?;
    tracing::trace!("Post Successful! {resp:?}");
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

#[tracing::instrument]
async fn handler(e: lambda_runtime::LambdaEvent<GatewayRequest>) -> Result<GatewayResponse, Error> {
    let sdk_config = aws_config::load_from_env().await;
    let secrets = Secrets::load(&sdk_config).await?;

    if !github::verify_request(
        &e.payload.body,
        &e.payload.headers.x_hub_signature_256,
        &secrets.github_webhook_verification_secret,
    ) {
        return Err(ProcessError::InvalidWebhookSignature.into());
    }

    let event: github::WebhookEvent =
        serde_json::from_str(&e.payload.body).map_err(ProcessError::WebhookEventParsingFailed)?;
    let Some(route) = Route::get(
        &aws_sdk_dynamodb::Client::new(&sdk_config),
        e.payload.path_parameters.identifiers.clone(),
    )
    .await?
    else {
        return Err(ProcessError::RouteNotFound(e.payload.path_parameters.identifiers).into());
    };

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
    } else if let Some(wj) = event.workflow_job {
        process_workflow_job_events(ctx, event.action, wj, event.deployment, event.repository).await?;
    } else {
        tracing::trace!("unprocessed message: {:?}", e.payload.body);
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
        .color(if d.is_closed() { COLOR_CLOSED } else { COLOR_OPEN });

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
            "*{}さん* からの <{}|{issue_icon}#{}({})> に向けた<{}|コメント>だよ{tail_char}",
            sender.login, d.html_url, d.number, d.title, cm.html_url
        )
    } else {
        format!(
            "*{}さん* が <{}|{issue_icon}#{}({})> に<{}|コメント>したよ{tail_char}{}",
            sender.login,
            d.html_url,
            d.number,
            d.title,
            cm.html_url,
            if rand::random() { "！" } else { "" }
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
        github::Action::Opened => format!(":issue-o: *{}さん* がissueを立てたよ！ :issue-o:", sender.login),
        github::Action::Closed => format!(":issue-c: *{}さん* がissueを閉じたよ :issue-c:", sender.login),
        github::Action::Reopened => format!(":issue-o: *{}さん* がissueをもう一回開いたよ :issue-o:", sender.login),
        _ => return Err(UnhandledIssueActionError(action).into()),
    };
    let issue_att_title = format!("[{}]#{}: {}", repo.full_name, iss.number, iss.title);

    let mut att_fields = Vec::with_capacity(1);
    if !iss.labels.is_empty() {
        let mut label_texts = iss.labels.iter().map(|l| l.name).collect::<Vec<_>>();
        label_texts.sort();

        att_fields.push(slack::AttachmentField {
            title: "Labelled",
            short: false,
            value: label_texts.join(","),
        });
    }
    let attachment = slack::Attachment::new(iss.body.as_deref().unwrap_or(""))
        .author(&iss.user.login, &iss.user.html_url, &iss.user.avatar_url)
        .title(&issue_att_title, &iss.html_url)
        .color(if iss.is_closed() { COLOR_CLOSED } else { COLOR_OPEN })
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
            "*{}さん* からの <{}|{issue_icon}#{}({})> に向けた<{}|コメント>だよ{tail_char}",
            sender.login, iss.html_url, iss.number, iss.title, cm.html_url,
        )
    } else {
        format!(
            "*{}さん* が <{}|{issue_icon}#{}({})> に<{}|コメント>したよ{tail_char}{}",
            sender.login,
            iss.html_url,
            iss.number,
            iss.title,
            cm.html_url,
            if rand::random() { "！" } else { "" },
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
        write!(fmt, "Unhandled pull request action: {:?}", self.0)
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
    let msg_base = match (action, merged, pr.draft) {
        (github::Action::ReadyForReview, _, _) => format!(
            ":pr: *{}さん* の <{}|:pr-draft:#{}: {}> がレビューできるようになったよ！よろしくね！ :pr:",
            sender.login, pr.html_url, pr.number, pr.title
        ),
        (github::Action::Opened, _, true) => format!(
            ":pr-draft: *{}さん* がPullRequestを作成したよ！ :pr-draft:",
            sender.login
        ),
        (github::Action::Reopened, _, true) => format!(
            ":pr-draft: *{}さん* がPullRequestを開き直したよ！ :pr-draft:",
            sender.login
        ),
        (github::Action::Opened, _, false) => format!(":pr: *{}さん* がPullRequestを作成したよ！ :pr:", sender.login),
        (github::Action::Reopened, _, false) => {
            format!(":pr: *{}さん* がPullRequestを開き直したよ！ :pr:", sender.login)
        }
        (github::Action::Closed, true, _) => {
            format!(":merge: *{}さん* がPullRequestをマージしたよ！ :merge:", sender.login)
        }
        (github::Action::Closed, false, _) => format!("*{}さん* がPullRequestを閉じたよ", sender.login),
        _ => return Err(UnhandledPullRequestActionError(action).into()),
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
    let msg = format!("{msg_base}{draft_msg}");

    let branch_flow_name = detect_branch_flow(
        pr.head.label.splitn(2, ":").nth(1).unwrap_or(&pr.head.label),
        pr.base.label.splitn(2, ":").nth(1).unwrap_or(&pr.base.label),
    );
    let mut att_fields = vec![slack::AttachmentField {
        title: "Branch Flow",
        short: false,
        value: format!("{branch_flow_name} ({} => {})", pr.head.label, pr.base.label),
    }];
    if !pr.labels.is_empty() {
        let mut label_texts = pr.labels.iter().map(|l| l.name).collect::<Vec<_>>();
        label_texts.sort();

        att_fields.push(slack::AttachmentField {
            title: "Labelled",
            short: false,
            value: label_texts.join(","),
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

#[derive(Debug)]
pub struct UnhandledWorkflowJobActionError(github::Action);
impl std::error::Error for UnhandledWorkflowJobActionError {}
impl std::fmt::Display for UnhandledWorkflowJobActionError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Unhandled workflow_job action: {:?}", self.0)
    }
}

async fn process_workflow_job_events(
    ctx: ExecutionContext,
    action: github::Action,
    job: github::WorkflowJob<'_>,
    deployment: Option<github::DeploymentInfo<'_>>,
    repository: github::Repository<'_>,
) -> Result<(), Error> {
    if action == github::Action::Waiting {
        // pending environment reviewer
        let deployment = deployment.ok_or(ProcessError::RequireField("deployment"))?;

        #[derive(serde::Deserialize)]
        pub struct InitCapture {
            pub repository: github::graphql::Repository,
            pub commit: github::graphql::Commit,
        }

        let (run_details, init_captures) = futures::try_join!(job.run_details().map_err(Into::into), async {
            let apiclient = ctx.connect_github(&repository.full_name).await?;
            apiclient
                .post_graphql::<github::graphql::QueryResponse<InitCapture>>(&format!(
                    "query {{ {reviewers}, commit: {commit} }}",
                    reviewers = apiclient.environment_protection_rule_query(&deployment.environment, None),
                    commit = apiclient.commit_message_and_committer_name_query(&job.head_sha)
                ))
                .await?
                .data()
                .map_err(Error::from)
        })?;

        let reviewer_users = init_captures
            .repository
            .environment
            .protection_rules
            .nodes
            .into_iter()
            .flat_map(|r| r.reviewers.nodes)
            .filter_map(|r| match r {
                github::graphql::DeploymentReviewer::User { login, .. } => Some(login),
                // TODO: Team?
            })
            .collect::<Vec<_>>();
        let prefix = [
            "以下のデプロイが承認待ちだよ!",
            "以下のデプロイをすすめるには承認が必要みたい。",
        ]
        .choose(&mut rand::thread_rng())
        .unwrap();
        let msg = format!(
            "{prefix}\n{} よろしくね!",
            reviewer_users
                .into_iter()
                .map(|gh| format!("{gh}さん"))
                .collect::<Vec<_>>()
                .join("、")
        );

        let att_fields = vec![
            slack::AttachmentField {
                title: "コミット情報",
                value: format!(
                    "<{commit_url}|ブランチ {} のコミット {}> (コミッターさん: {})「{}」",
                    job.head_branch,
                    &job.head_sha[..8],
                    init_captures.commit.committer.name,
                    init_captures.commit.message,
                    commit_url = github::commit_html_url(&repository, &job.head_sha),
                ),
                short: false,
            },
            slack::AttachmentField {
                title: "Environment",
                value: String::from(deployment.environment),
                short: true,
            },
            slack::AttachmentField {
                title: "ジョブ名",
                value: String::from(job.name),
                short: true,
            },
        ];
        let url = github::workflow_run_html_url(&job, &repository);
        let title = format!(
            "[{}] {} #{}",
            repository.full_name, job.workflow_name, run_details.run_number
        );
        let attachment = slack::Attachment::new("").title(&title, &url).fields(att_fields);

        ctx.post_message(&msg, |p| p.as_user().attachments(vec![attachment]))
            .await?;
        return Ok(());
    }

    Err(UnhandledWorkflowJobActionError(action).into())
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
