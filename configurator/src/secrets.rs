use futures_util::FutureExt;

#[derive(serde::Deserialize)]
pub struct MasqueradeConfiguratorSecrets {
    pub slack_app_signing_secret: String,
}

#[derive(serde::Deserialize)]
pub struct ServiceSecrets {
    pub slack_bot_token: String,
}

pub async fn load(
    sdk_config: &aws_config::SdkConfig,
) -> Result<(MasqueradeConfiguratorSecrets, ServiceSecrets), Box<dyn std::error::Error + Send + Sync>> {
    let c = aws_sdk_secretsmanager::Client::new(sdk_config);
    let msq_secrets = c
        .get_secret_value()
        .secret_id("masquerade-configurator")
        .send()
        .map(|r| serde_json::from_str(r?.secret_string().unwrap_or("")).map_err(From::from));
    let service_secrets = c
        .get_secret_value()
        .secret_id("repoact-notify")
        .send()
        .map(|r| serde_json::from_str(r?.secret_string().unwrap_or("")).map_err(From::from));

    futures_util::try_join!(msq_secrets, service_secrets)
}
