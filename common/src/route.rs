use aws_sdk_dynamodb::types::AttributeValue;

#[derive(Debug, thiserror::Error)]
pub enum RouteReadWriteError {
    #[error("Route record key {0} is not found in the record")]
    KeyNotFound(&'static str),
    #[error("Route record key {0} is not a string")]
    ValueIsNotString(&'static str),
    #[error(transparent)]
    DynamoDBError(#[from] aws_sdk_dynamodb::Error),
}

pub struct Route {
    pub repository_fullpath: String,
    pub channel_id: String,
}
impl Route {
    const TABLE_NAME: &'static str = "Peridot-GithubActivityNotification-RouteMap";

    pub async fn get(client: &aws_sdk_dynamodb::Client, route_id: String) -> Result<Option<Self>, RouteReadWriteError> {
        let Some(mut item) = client
            .get_item()
            .table_name(Self::TABLE_NAME)
            .key("path", AttributeValue::S(route_id))
            .send()
            .await
            .map_err(aws_sdk_dynamodb::Error::from)?
            .item
        else {
            return Ok(None);
        };

        let repository_fullpath = match item.remove("repository_fullpath") {
            Some(AttributeValue::S(x)) => x,
            Some(_) => return Err(RouteReadWriteError::ValueIsNotString("repository_fullpath")),
            None => return Err(RouteReadWriteError::KeyNotFound("repository_fullpath")),
        };
        let channel_id = match item.remove("channel_id") {
            Some(AttributeValue::S(x)) => x,
            Some(_) => return Err(RouteReadWriteError::ValueIsNotString("channel_id")),
            None => return Err(RouteReadWriteError::KeyNotFound("channel_id")),
        };

        Ok(Some(Self {
            repository_fullpath,
            channel_id,
        }))
    }

    pub async fn put(self, client: &aws_sdk_dynamodb::Client, route_id: String) -> Result<(), RouteReadWriteError> {
        client
            .put_item()
            .table_name(Self::TABLE_NAME)
            .item("path", AttributeValue::S(route_id))
            .item("repository_fullpath", AttributeValue::S(self.repository_fullpath))
            .item("channel_id", AttributeValue::S(self.channel_id))
            .send()
            .await
            .map_err(aws_sdk_dynamodb::Error::from)?;

        Ok(())
    }
}
