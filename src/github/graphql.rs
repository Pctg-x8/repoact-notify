#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub start_cursor: String,
    pub end_cursor: String,
    pub has_next_page: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "__typename")]
#[serde(rename_all = "camelCase")]
pub enum DeploymentReviewer {
    User { name: String, login: String },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection<Node> {
    pub nodes: Vec<Node>,
    pub page_info: Option<PageInfo>,
    pub total_count: Option<u64>,
}

pub type DeploymentReviewerConnection = Connection<DeploymentReviewer>;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentProtectionRule {
    pub reviewers: DeploymentReviewerConnection,
}

pub type DeploymentProtectionRuleConnection = Connection<DeploymentProtectionRule>;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub protection_rules: DeploymentProtectionRuleConnection,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub environment: Environment,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitActor {
    pub name: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub message: String,
    pub committer: GitActor,
}

#[derive(Debug, serde::Deserialize)]
#[serde(transparent)]
pub struct QueryError(pub serde_json::Value);
impl std::error::Error for QueryError {}
impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, f)
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueryResponse<ResponseData> {
    Data(ResponseData),
    Errors(serde_json::Value),
}
impl<D> QueryResponse<D> {
    pub fn data(self) -> Result<D, QueryError> {
        match self {
            Self::Data(d) => Ok(d),
            Self::Errors(e) => Err(QueryError(e)),
        }
    }
}

#[derive(serde::Serialize)]
pub struct GraphQLPostForm<'s> {
    pub query: &'s str,
}

impl super::ApiClient<'_> {
    pub fn commit_message_and_committer_name_query(&self, sha: &str) -> String {
        let url = format!("https://github.com/{}/commit/{sha}", self.repo_fullname);

        format!("resource(url: {url:?}) {{ ...on Commit {{ message committer {{ name }} }} }}")
    }

    pub fn environment_protection_rule_query(&self, environment_name: &str, from_cursor: Option<&str>) -> String {
        let mut spl = self.repo_fullname.splitn(2, "/");
        let repo_owner = spl.next().unwrap_or("");
        let repo_name = spl.next().unwrap_or("");

        format!(
            r#"repository(owner: {repo_owner:?}, name: {repo_name:?}) {{
            environment(name: {environment_name:?}) {{
                protectionRules(first: 1) {{
                    totalCount
                    nodes {{
                        reviewers({q}) {{
                            nodes {{
                                __typename
                                ... on User {{
                                    name
                                    login
                                }}
                            }}
                        }}
                    }}
                }}
            }}
        }}"#,
            q = from_cursor.map_or(String::from("first: 1"), |c| format!("after: {c:?}"))
        )
    }

    pub async fn post_graphql<R: serde::de::DeserializeOwned>(&self, query: &str) -> reqwest::Result<R> {
        let s = self
            .authorized_post_request("https://api.github.com/graphql")
            .json(&GraphQLPostForm { query })
            .send()
            .await?
            .text()
            .await?;
        Ok(serde_json::from_str(&s)
            .unwrap_or_else(|e| panic!("Failed to decode object while parsing response: {s}: {e:?}")))
    }
}
