pub const REPOACT_CHANNELID: &'static str = "#repo-activities";

fn bot_token() -> String {
    std::env::var("SLACK_BOT_TOKEN").expect("no SLACK_BOT_TOKEN set")
}

#[derive(serde::Serialize)]
pub struct Attachment<'s> {
    pub color: Option<&'s str>,
    pub author_name: Option<&'s str>,
    pub author_link: Option<&'s str>,
    pub author_icon: Option<&'s str>,
    pub title: Option<&'s str>,
    pub title_link: Option<&'s str>,
    pub text: &'s str,
    pub fields: Vec<AttachmentField<'s>>,
}
impl<'s> Attachment<'s> {
    pub const fn new(text: &'s str) -> Self {
        Self {
            text,
            color: None,
            author_name: None,
            author_link: None,
            author_icon: None,
            title: None,
            title_link: None,
            fields: Vec::new(),
        }
    }

    pub const fn author(mut self, name: &'s str, link: &'s str, icon: &'s str) -> Self {
        self.author_name = Some(name);
        self.author_link = Some(link);
        self.author_icon = Some(icon);
        self
    }
    pub const fn title(mut self, title: &'s str, link: &'s str) -> Self {
        self.title = Some(title);
        self.title_link = Some(link);
        self
    }
    pub const fn color(mut self, color: &'s str) -> Self {
        self.color = Some(color);
        self
    }
    pub fn fields(mut self, fields: Vec<AttachmentField<'s>>) -> Self {
        self.fields = fields;
        self
    }
}
#[derive(serde::Serialize)]
pub struct AttachmentField<'s> {
    pub title: &'s str,
    pub value: String,
    pub short: bool,
}
#[derive(serde::Serialize)]
pub struct PostMessage<'s> {
    pub channel: &'s str,
    pub text: &'s str,
    pub as_user: bool,
    pub unfurl_links: bool,
    pub unfurl_media: bool,
    pub attachments: Vec<Attachment<'s>>,
}
impl<'s> PostMessage<'s> {
    pub fn post(&self) -> reqwest::Result<String> {
        reqwest::Client::new()
            .post("https://slack.com/api/chat.postMessage")
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", bot_token()),
            )
            .json(self)
            .send()?
            .text()
    }

    pub const fn new(channel: &'s str, text: &'s str) -> Self {
        PostMessage {
            channel,
            text,
            as_user: false,
            unfurl_links: false,
            unfurl_media: false,
            attachments: Vec::new(),
        }
    }
    pub const fn as_user(mut self) -> Self {
        self.as_user = true;
        self
    }
    pub fn attachments(mut self, attachments: Vec<Attachment<'s>>) -> Self {
        self.attachments = attachments;
        self
    }
}
