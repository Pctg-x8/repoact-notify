use std::{borrow::Cow, collections::HashMap};

use repoact_notify_common::{slack, Route};
use ring::{
    constant_time,
    hmac::{self, HMAC_SHA256},
};

mod secrets;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    env_logger::init();
    lambda_runtime::run(lambda_runtime::service_fn(handler)).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequest<H = HashMap<String, String>> {
    pub headers: H,
    pub body: String,
    pub is_base64_encoded: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SlackRequestHeaders {
    pub x_slack_request_timestamp: String,
    pub x_slack_signature: String,
}

#[derive(serde::Deserialize)]
pub struct SlackSlashCommandPayload {
    pub channel_id: String,
    pub text: String,
    pub command: String,
}

#[derive(Debug)]
pub enum ProcessError {
    SlackRequestValidationFailed(String, String),
}
impl std::error::Error for ProcessError {}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::SlackRequestValidationFailed(c, e) => {
                write!(f, "Invalid request: computed={c:?} expected={e:?}")
            }
        }
    }
}

pub enum ParseError<'s> {
    SyntaxError(nom::Err<nom::error::Error<&'s str>>),
    UnrecognizedCommand(&'s str),
}
impl std::fmt::Display for ParseError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError(e) => write!(f, "Syntax error: {e}"),
            Self::UnrecognizedCommand(s) => write!(f, "Unrecognized command: {s}"),
        }
    }
}

async fn handler(
    e: lambda_runtime::LambdaEvent<GatewayRequest<SlackRequestHeaders>>,
) -> Result<String, lambda_runtime::Error> {
    let (msq_secrets, service_secrets) = secrets::load().await?;
    let body = if e.payload.is_base64_encoded {
        String::from_utf8(base64::decode(e.payload.body)?)?
    } else {
        e.payload.body
    };

    verify_slack_command_request(
        &body,
        &e.payload.headers.x_slack_request_timestamp,
        &msq_secrets.slack_app_signing_secret,
        e.payload.headers.x_slack_signature,
    )?;

    let payload: SlackSlashCommandPayload = serde_urlencoded::from_str(&body)?;
    let args = match &payload.command as &str {
        "/add-repoact-notify" => parse_add_args(&payload.text)
            .map_err(ParseError::SyntaxError)
            .map(|(_, item)| item),
        _ => Err(ParseError::UnrecognizedCommand(&payload.command).into()),
    };
    let args = match args {
        Ok(a) => a,
        Err(e) => return Ok(format!("Error while parsing args({:?}): {e}", payload.text)),
    };

    match args {
        Args::Add {
            repo_fullname,
            path,
        } => {
            // prebuild message
            let msg = format!("これから<https://github.com/{repo_fullname}:{repo_fullname}>の状況をこのチャンネルに通知していくよ!よろしくね!");

            Route {
                repository_fullpath: repo_fullname.into_owned(),
                channel_id: payload.channel_id.clone(),
            }
            .put(path.into_owned())
            .await?;

            slack::PostMessage::new(&payload.channel_id, &msg)
                .as_user()
                .post(&service_secrets.slack_bot_token)
                .await?;
        }
    }

    Ok(String::new())
}

fn verify_slack_command_request<'s>(
    body: &str,
    request_timestamp: &str,
    signing_secret: &str,
    valid_signature: String,
) -> Result<(), ProcessError> {
    let key = hmac::Key::new(HMAC_SHA256, &signing_secret.as_bytes());
    let payload = format!("v0:{request_timestamp}:{body}");
    let computed = hmac::sign(&key, payload.as_bytes());
    let mut verify_target = Vec::with_capacity(computed.as_ref().len() * 2 + 3);
    verify_target.extend(b"v0=");
    for b in computed.as_ref() {
        verify_target.extend(format!("{b:02x}").into_bytes());
    }

    constant_time::verify_slices_are_equal(&verify_target, valid_signature.as_bytes()).map_err(
        |_| {
            ProcessError::SlackRequestValidationFailed(
                unsafe { String::from_utf8_unchecked(verify_target) },
                valid_signature,
            )
        },
    )
}

enum Args<'s> {
    Add {
        repo_fullname: Cow<'s, str>,
        path: Cow<'s, str>,
    },
}
fn parse_add_args<'s>(args: &'s str) -> nom::IResult<&'s str, Args<'s>> {
    nom::combinator::map(
        nom::sequence::tuple((
            arg_fragment,
            nom::bytes::complete::take_while(char::is_whitespace),
            arg_fragment,
        )),
        |(repo_fullname, _, path)| Args::Add {
            repo_fullname,
            path,
        },
    )(args)
}

fn arg_fragment<'s>(input: &'s str) -> nom::IResult<&'s str, Cow<'s, str>> {
    // reduced version of https://github.com/Geal/nom/blob/main/examples/string.rs
    #[derive(Clone)]
    enum Fragment<'s> {
        Literal(&'s str),
        Escaped(char),
    }
    let str_build = nom::multi::fold_many0(
        nom::branch::alt((
            nom::combinator::map(
                nom::combinator::verify(nom::bytes::complete::is_not("\"\\"), |s: &str| {
                    !s.is_empty()
                }),
                Fragment::Literal,
            ),
            nom::sequence::preceded(
                nom::character::complete::char('\\'),
                nom::branch::alt((
                    nom::combinator::value(
                        Fragment::Escaped('\n'),
                        nom::character::complete::char('n'),
                    ),
                    nom::combinator::value(
                        Fragment::Escaped('\t'),
                        nom::character::complete::char('t'),
                    ),
                    nom::combinator::value(
                        Fragment::Escaped('\r'),
                        nom::character::complete::char('r'),
                    ),
                )),
            ),
        )),
        String::new,
        |mut s, f| match f {
            Fragment::Literal(sl) => {
                s.push_str(sl);
                s
            }
            Fragment::Escaped(c) => {
                s.push(c);
                s
            }
        },
    );
    let str_parser = nom::sequence::delimited(
        nom::character::complete::char('"'),
        str_build,
        nom::character::complete::char('"'),
    );
    let ident_parser = nom::bytes::complete::take_while1(|c: char| !c.is_whitespace());

    nom::branch::alt((
        nom::combinator::map(str_parser, Cow::Owned),
        nom::combinator::map(ident_parser, Cow::Borrowed),
    ))(input)
}
