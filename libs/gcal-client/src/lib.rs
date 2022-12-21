mod models;
use reqwest::header;
use std::error::Error;
use std::ops::Range;

use chrono::{DateTime, Utc};

pub type DateRange = Range<DateTime<Utc>>;

#[derive(Debug)]
pub struct Client {
    client: reqwest::Client,
    calendar_id: String,
}

impl Client {
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

    pub async fn get_events(
        &self,
        date_range: DateRange,
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
    date_range: &DateRange,
    event_count: &Option<u32>,
    next_page_token: &Option<String>,
) -> Vec<(&'static str, String)> {
    let start_date = date_range
        .start
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let end_date = date_range
        .end
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut query_parameters: Vec<(&'static str, String)> = vec![
        ("singleEvents", "true".to_owned()),
        ("orderBy", "startTime".to_owned()),
        ("timeMin", start_date),
        ("timeMax", end_date),
    ];

    if let Some(count) = event_count {
        query_parameters.push(("maxResults", count.to_string()));
    }

    if let Some(token) = next_page_token {
        query_parameters.push(("pageToken", token.clone()));
    }

    query_parameters
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

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

        let query_parameters = build_query_parameters(&date_range, &None, &None);

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
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

        let query_parameters = build_query_parameters(&date_range, &Some(30), &None);

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
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

        let query_parameters = build_query_parameters(&date_range, &None, &Some("abcd".to_owned()));

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
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
            build_query_parameters(&date_range, &Some(30), &Some("abcd".to_owned()));

        let expected_parameters: Vec<(&str, String)> = vec![
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            (
                "timeMin",
                start_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            (
                "timeMax",
                end_date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            ("maxResults", "30".to_owned()),
            ("pageToken", "abcd".to_owned()),
        ];

        assert_eq!(expected_parameters, query_parameters);
    }
}
