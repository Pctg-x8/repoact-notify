use std::collections::HashMap;

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
pub struct GatewayRequest<H = HashMap<String, String>> {
    pub headers: H,
    pub body: String,
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

async fn handler(
    e: lambda_runtime::LambdaEvent<GatewayRequest<SlackRequestHeaders>>,
) -> Result<(), lambda_runtime::Error> {
    let (msq_secrets, service_secrets) = secrets::load().await?;

    verify_slack_command_request(
        &e.payload.body,
        &e.payload.headers.x_slack_request_timestamp,
        &msq_secrets.slack_app_signing_secret,
        e.payload.headers.x_slack_signature,
    )?;

    Ok(())
}

fn verify_slack_command_request<'s>(
    body: &str,
    request_timestamp: &str,
    signing_secret: &str,
    valid_signature: String,
) -> Result<(), ProcessError> {
    let mut key_bytes = Vec::with_capacity(signing_secret.len() / 2);
    for s in signing_secret.as_bytes().chunks_exact(2) {
        fn hex_byte_to_u8(b: u8) -> u8 {
            match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => 0x0a + (b - b'a'),
                b'A'..=b'F' => 0x0a + (b - b'A'),
                _ => unreachable!(),
            }
        }

        key_bytes.push((hex_byte_to_u8(s[0]) << 4) | hex_byte_to_u8(s[1]));
    }

    let key = hmac::Key::new(HMAC_SHA256, &key_bytes);
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
