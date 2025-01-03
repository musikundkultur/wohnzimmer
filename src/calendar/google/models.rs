use jiff::{civil::Date, tz::TimeZone, Timestamp};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Creator {
    pub email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Organizer {
    pub email: String,
    pub display_name: String,
    #[serde(rename(deserialize = "self"))]
    pub _self: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Timepoint {
    pub date_time: Option<Timestamp>,
    pub date: Option<Date>,
    pub time_zone: Option<String>,
}

impl Timepoint {
    // Converts the timepoint into a UTC Timestamp.
    pub fn to_timestamp(&self) -> Timestamp {
        self.date_time.unwrap_or_else(|| {
            self.date
                .and_then(|date| date.at(0, 0, 0, 0).to_zoned(TimeZone::system()).ok())
                .map(|zoned| zoned.timestamp())
                .unwrap_or_default()
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Attachment {
    pub file_url: String,
    pub title: String,
    pub mime_type: String,
    pub icon_link: String,
    pub file_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Event {
    pub kind: String,
    pub etag: String,
    pub id: String,
    pub status: String,
    pub html_link: String,
    pub created: String,
    pub updated: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub creator: Creator,
    pub organizer: Organizer,
    pub start: Timepoint,
    pub end: Timepoint,
    #[serde(rename(deserialize = "iCalUID"))]
    pub i_cal_uid: String,
    pub sequence: u64,
    pub event_type: String,
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Events {
    pub kind: String,
    pub etag: String,
    pub summary: String,
    pub updated: String,
    pub time_zone: String,
    pub access_role: String,
    pub default_reminders: Vec<String>,
    pub next_sync_token: Option<String>,
    pub items: Vec<Event>,
    pub next_page_token: Option<String>,
}
