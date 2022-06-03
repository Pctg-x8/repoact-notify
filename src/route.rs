use std::collections::HashMap;

use rusoto_dynamodb::DynamoDb;

#[derive(Debug)]
pub enum RouteReadWriteError {
    KeyNotFound(&'static str),
    ValueIsNotString(&'static str),
}
impl std::error::Error for RouteReadWriteError {}
impl std::fmt::Display for RouteReadWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyNotFound(k) => write!(f, "Route record key {k} is not found in the record"),
            Self::ValueIsNotString(k) => write!(f, "Route record key {k} is not string"),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct Route {
    pub repository_fullpath: String,
    pub channel_id: String,
}
impl Route {
    pub async fn get(
        route_id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mut primary_keys = HashMap::with_capacity(1);
        primary_keys.insert(
            String::from("path"),
            rusoto_dynamodb::AttributeValue {
                s: Some(String::from(route_id)),
                ..Default::default()
            },
        );
        let item = rusoto_dynamodb::DynamoDbClient::new(rusoto_core::Region::ApNortheast1)
            .get_item(rusoto_dynamodb::GetItemInput {
                table_name: String::from("Peridot-GithubActivityNotification-RouteMap"),
                key: primary_keys,
                ..Default::default()
            })
            .await?
            .item;

        item.map(|mut r| {
            Ok(Self {
                repository_fullpath: r
                    .remove("repository_fullpath")
                    .ok_or(RouteReadWriteError::KeyNotFound("repository_fullpath"))?
                    .s
                    .ok_or(RouteReadWriteError::ValueIsNotString("repository_fullpath"))?,
                channel_id: r
                    .remove("channel_id")
                    .ok_or(RouteReadWriteError::KeyNotFound("channel_id"))?
                    .s
                    .ok_or(RouteReadWriteError::ValueIsNotString("channel_id"))?,
            })
        })
        .transpose()
    }
}
