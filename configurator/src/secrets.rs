use futures_util::FutureExt;
use rusoto_secretsmanager::{GetSecretValueRequest, SecretsManager};

#[derive(serde::Deserialize)]
pub struct MasqueradeConfiguratorSecrets {
    pub slack_app_signing_secret: String,
}

#[derive(serde::Deserialize)]
pub struct ServiceSecrets {
    pub slack_bot_token: String,
}

pub async fn load(
) -> Result<(MasqueradeConfiguratorSecrets, ServiceSecrets), Box<dyn std::error::Error + Send + Sync>>
{
    let c = rusoto_secretsmanager::SecretsManagerClient::new(rusoto_core::Region::ApNortheast1);
    let msq_secrets = c
        .get_secret_value(GetSecretValueRequest {
            secret_id: String::from("masquerade-configurator"),
            ..Default::default()
        })
        .map(|r| {
            r.map_err(From::from).and_then(|r| {
                serde_json::from_str(&r.secret_string.expect("no secret string?"))
                    .map_err(From::from)
            })
        });
    let service_secrets = c
        .get_secret_value(GetSecretValueRequest {
            secret_id: String::from("repoact-notify"),
            ..Default::default()
        })
        .map(|r| {
            r.map_err(From::from).and_then(|r| {
                serde_json::from_str(&r.secret_string.expect("no secret string?"))
                    .map_err(From::from)
            })
        });

    futures_util::try_join!(msq_secrets, service_secrets)
}
