use rusoto_secretsmanager::{GetSecretValueRequest, SecretsManager, SecretsManagerClient};

#[derive(serde::Deserialize)]
pub struct Secrets {
    pub slack_bot_token: String,
    pub github_app_id: String,
    pub github_app_installation_id: String,
    pub github_webhook_verification_secret: String,
    pub github_app_pem: String,
}
impl Secrets {
    pub async fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = SecretsManagerClient::new(rusoto_core::Region::ApNortheast1)
            .get_secret_value(GetSecretValueRequest {
                secret_id: String::from("repoact-notify"),
                version_id: None,
                version_stage: None,
            })
            .await?
            .secret_string
            .expect("No secret string?");

        serde_json::from_str(&data).map_err(From::from)
    }
}
