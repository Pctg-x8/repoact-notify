
pub const REPOACT_CHANNELID: &'static str = "#repo-activities";
pub const BOT_TOKEN: &'static str = env!("SLACK_BOT_TOKEN");

#[derive(Serialize)]
pub struct Attachment<'s>
{
    pub color: &'s str,
    pub author_name: &'s str, pub author_link: &'s str, pub author_icon: &'s str,
    pub title: Option<&'s str>, pub title_link: Option<&'s str>, pub text: &'s str,
    pub fields: Vec<AttachmentField<'s>>
}
#[derive(Serialize)]
pub struct AttachmentField<'s>
{
    pub title: &'s str, pub value: String, pub short: bool
}
#[derive(Serialize)]
pub struct PostMessage<'s>
{
    pub channel: &'s str, pub text: &'s str, pub as_user: bool, pub unfurl_links: bool, pub unfurl_media: bool,
    pub attachments: Vec<Attachment<'s>>
}
impl<'s> PostMessage<'s>
{
    pub fn post(&self) -> reqwest::Result<String>
    {
        reqwest::Client::new()
            .post("https://slack.com/api/chat.postMessage")
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", BOT_TOKEN))
            .json(self).send()?.text()
    }
}
