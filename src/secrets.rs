#[derive(serde::Deserialize)]
pub struct Secrets {
    pub slack_bot_token: String,
    pub github_app_id: String,
    pub github_app_installation_id: String,
    pub github_webhook_verification_secret: String,
    pub github_app_pem: String,
}
impl Secrets {
    pub async fn load(config: &aws_config::SdkConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = aws_sdk_secretsmanager::Client::new(config)
            .get_secret_value()
            .secret_id("repoact-notify")
            .send()
            .await?
            .secret_string
            .expect("No secret string?");

        serde_json::from_str(&data).map_err(From::from)
    }
}
