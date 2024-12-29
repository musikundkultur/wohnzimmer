use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;

mod rfc3339_to_datetime_utc {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<String>::deserialize(deserializer)? {
            Some(s) => {
                let offset_time =
                    DateTime::parse_from_rfc3339(&s).map_err(serde::de::Error::custom)?;
                Ok(Some(DateTime::from_naive_utc_and_offset(
                    offset_time.naive_utc(),
                    Utc,
                )))
            }
            None => Ok(None),
        }
    }
}

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
    #[serde(default, with = "rfc3339_to_datetime_utc")]
    pub date_time: Option<DateTime<Utc>>,
    pub date: Option<NaiveDate>,
    pub time_zone: Option<String>,
}

impl Timepoint {
    // Converts the timepoint into a UTC DateTime.
    pub fn to_date_time(&self) -> DateTime<Utc> {
        self.date_time.unwrap_or_else(|| {
            self.date
                .and_then(|date| {
                    date.and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_local_timezone(chrono_tz::CET)
                        .earliest()
                        .map(|date| date.to_utc())
                })
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
