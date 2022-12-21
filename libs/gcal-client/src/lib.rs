mod models;
use reqwest::header;
use std::error::Error;
use std::ops::Range;

use chrono::{DateTime, SecondsFormat, Utc};

pub type DateRange = Range<DateTime<Utc>>;

#[derive(Debug)]
pub struct Client {
    client: reqwest::Client,
    calendar_id: String,
}

impl Client {
    /// Create a new google calendar client it will fetch the service host credentials from the
    /// environment either via the GOOGLE_APPLICATION_CREDENTIALS variable pointing to the json
    /// key file generated in the google cloud console for the account or via a the
    /// GOOGLE_APPLICATION_CREDENTIALS_JSON variable containing the content of said json file
    /// encoded as base64. It will further fetch the id of the calendar that it will query from
    /// the GOOGLE_CALENDAR_ID environment variable.
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        // We only need readonly acccess
        let scopes = ["https://www.googleapis.com/auth/calendar.readonly"];

        let config = google_cloud_auth::Config {
            audience: None,
            scopes: Some(&scopes),
        };

        // This internally looks up the service account credentials that come from json key file
        // generated in the google cloud console. The lookup happens via the
        // GOOGLE_APPLICATION_CREDENTIALS environments stored in the .env file
        let ts = google_cloud_auth::create_token_source(config).await?;
        let token = ts.token().await?;

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(format!("Bearer {}", token.access_token).as_str())?,
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            header::HeaderValue::from_str("gzip")?,
        );
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_str(
                format!("GCAL_CLIENT/{} (gzip)", env!("CARGO_PKG_VERSION")).as_str(),
            )?,
        );

        Ok(Self {
            client: reqwest::Client::builder()
                .default_headers(headers)
                .build()?,
            calendar_id: std::env::var("GOOGLE_CALENDAR_ID")?,
        })
    }

    /// Queries events from the google calendar. The query can be filtered by a DateRange and the
    /// number of results can be limited to a certain number of events in which case the Result
    /// might return a page token for pagination purposes that should be used in the next request
    /// to get the next page of events. This function will return a a list of events that fulfill
    /// the given requirements.
    pub async fn get_events(
        &self,
        date_range: Option<DateRange>,
        event_count: Option<u32>,
        next_page_token: Option<String>,
    ) -> Result<(Vec<models::Event>, Option<String>), Box<dyn Error>> {
        let events_request = self.client.get(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            self.calendar_id
        ));

        let events = events_request
            .query(&build_query_parameters(
                &date_range,
                &event_count,
                &next_page_token,
            ))
            .send()
            .await?
            .text()
            .await?;

        let events: models::Events = serde_json::from_str(&events)?;
        Ok((events.items, events.next_page_token))
    }
}

fn build_query_parameters(
    date_range: &Option<DateRange>,
    event_count: &Option<u32>,
    next_page_token: &Option<String>,
) -> Vec<(&'static str, String)> {
    // Google requires rfc3339 format for the times with a fixed offset
    // see: https://developers.google.com/calendar/api/v3/reference/events/list

    let mut query_parameters: Vec<(&'static str, String)> = vec![
        // filter out reoccuring events
        ("singleEvents", "true".to_owned()),
        // order ascending by start time
        ("orderBy", "startTime".to_owned()),
    ];

    if let Some(range) = date_range {
        let start_date = range.start.to_rfc3339_opts(SecondsFormat::Secs, true);
        let end_date = range.end.to_rfc3339_opts(SecondsFormat::Secs, true);
        // limit the events by a time frame
        query_parameters.append(&mut vec![("timeMin", start_date), ("timeMax", end_date)]);
    }

    if let Some(count) = event_count {
        // limit the number of events to a specific count
        query_parameters.push(("maxResults", count.to_string()));
    }

    if let Some(token) = next_page_token {
        // page token returned by previous request to fetch the next page
        query_parameters.push(("pageToken", token.clone()));
    }

    query_parameters
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    #[test]
    fn build_query_parameters_without_parameters() {
        let query_parameters = build_query_parameters(&None, &None, &None);

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
        ];

        assert_eq!(expected_parameters, query_parameters);
    }
    #[test]
    fn build_query_parameters_without_page_token_and_event_count_limit() {
        let start_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-08:00").unwrap();
        let start_date: DateTime<Utc> = DateTime::from_utc(start_date.naive_utc(), Utc);

        let end_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-09:00").unwrap();
        let end_date: DateTime<Utc> = DateTime::from_utc(end_date.naive_utc(), Utc);

        let date_range = DateRange {
            start: start_date,
            end: end_date,
        };

        let query_parameters = build_query_parameters(&Some(date_range), &None, &None);

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
        ];

        assert_eq!(expected_parameters, query_parameters);
    }

    #[test]
    fn build_query_parameters_without_page_token() {
        let start_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-08:00").unwrap();
        let start_date: DateTime<Utc> = DateTime::from_utc(start_date.naive_utc(), Utc);

        let end_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-09:00").unwrap();
        let end_date: DateTime<Utc> = DateTime::from_utc(end_date.naive_utc(), Utc);

        let date_range = DateRange {
            start: start_date,
            end: end_date,
        };

        let query_parameters = build_query_parameters(&Some(date_range), &Some(30), &None);

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            ("maxResults", "30".to_owned()),
        ];

        assert_eq!(expected_parameters, query_parameters);
    }

    #[test]
    fn build_query_parameters_without_event_count() {
        let start_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-08:00").unwrap();
        let start_date: DateTime<Utc> = DateTime::from_utc(start_date.naive_utc(), Utc);

        let end_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-09:00").unwrap();
        let end_date: DateTime<Utc> = DateTime::from_utc(end_date.naive_utc(), Utc);

        let date_range = DateRange {
            start: start_date,
            end: end_date,
        };

        let query_parameters =
            build_query_parameters(&Some(date_range), &None, &Some("abcd".to_owned()));

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            ("pageToken", "abcd".to_owned()),
        ];

        assert_eq!(expected_parameters, query_parameters);
    }

    #[test]
    fn build_query_parameters_with_event_count_and_page_token() {
        let start_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-08:00").unwrap();
        let start_date: DateTime<Utc> = DateTime::from_utc(start_date.naive_utc(), Utc);

        let end_date: DateTime<FixedOffset> =
            DateTime::parse_from_rfc3339("1996-12-19T16:39:57-09:00").unwrap();
        let end_date: DateTime<Utc> = DateTime::from_utc(end_date.naive_utc(), Utc);

        let date_range = DateRange {
            start: start_date,
            end: end_date,
        };

        let query_parameters =
            build_query_parameters(&Some(date_range), &Some(30), &Some("abcd".to_owned()));

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
            ("maxResults", "30".to_owned()),
            ("pageToken", "abcd".to_owned()),
        ];

        assert_eq!(expected_parameters, query_parameters);
    }
}
