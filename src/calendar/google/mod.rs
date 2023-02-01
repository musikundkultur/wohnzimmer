pub mod models;
use indexmap::IndexMap;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, AUTHORIZATION};
use std::ops::Range;

use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::{Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Middleware, Next};
use task_local_extensions::Extensions;
use tokio::sync::Mutex;

/// Represents a timeframe with a start and end time
pub type DateRange = Range<DateTime<Utc>>;

/// Google calendar client for making requests to the google calendar api
#[derive(Debug)]
pub struct GoogleCalendarClient {
    client: ClientWithMiddleware,
    calendar_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Errpr while authenticating with google
    #[error("Authentication error: {0}")]
    GCloudAuthError(#[from] google_cloud_auth::error::Error),

    /// Error while making a http request
    #[error("RequestError: {0}")]
    RequestError(#[from] reqwest::Error),

    /// Error while executing some middleware code
    #[error("RequestMiddlewareError: {0}")]
    RequestMiddlewareError(#[from] reqwest_middleware::Error),

    /// Error while building http headers
    #[error("InvalidHeaderError: {0}")]
    InvalidHeaderValueError(#[from] reqwest::header::InvalidHeaderValue),

    /// No calendar id is found in env when it is not passed into client constructor
    #[error("MissingCalendarIDError: {0}")]
    MissingCalendarIDError(#[from] std::env::VarError),

    /// Error while parsing responses
    #[error("JsonParsingError: {0}")]
    JsonParsingError(#[from] serde_json::Error),
}

async fn refresh_token() -> Result<google_cloud_auth::token::Token, ClientError> {
    let scopes = ["https://www.googleapis.com/auth/calendar.readonly"];
    let config = google_cloud_auth::Config {
        audience: None,
        scopes: Some(&scopes),
    };

    // This internally looks up the service account credentials that come from json key file
    // generated in the google cloud console. The lookup happens via the
    // GOOGLE_APPLICATION_CREDENTIALS environments stored in the .env file
    let ts = google_cloud_auth::create_token_source(config).await?;
    Ok(ts.token().await?)
}

impl From<ClientError> for reqwest_middleware::Error {
    fn from(err: ClientError) -> Self {
        reqwest_middleware::Error::Middleware(anyhow::Error::new(err))
    }
}

struct AuthMiddleware {
    token: Mutex<Option<google_cloud_auth::token::Token>>,
}

impl AuthMiddleware {
    fn new() -> Self {
        Self {
            token: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl Middleware for AuthMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        {
            let mut token = self.token.lock().await;

            let token = match token.as_ref() {
                Some(token) if token.valid() => token,
                _ => refresh_token().await.map(|t| {
                    *token = Some(t);
                    token.as_ref().unwrap()
                })?,
            };

            let mut header =
                HeaderValue::from_str(format!("Bearer {}", token.access_token).as_str())
                    .map_err(ClientError::from)?;
            header.set_sensitive(true);
            req.headers_mut().insert(AUTHORIZATION, header);
        }
        next.run(req, extensions).await
    }
}

impl GoogleCalendarClient {
    /// Create a new google calendar client it will fetch the service host credentials from the
    /// environment either via the GOOGLE_APPLICATION_CREDENTIALS variable pointing to the json
    /// key file generated in the google cloud console for the account or via a the
    /// GOOGLE_APPLICATION_CREDENTIALS_JSON variable containing the content of said json file
    /// encoded as base64. It will further fetch the id of the calendar that it will query from
    /// the GOOGLE_CALENDAR_ID environment variable.
    pub fn new(calendar_id: Option<String>) -> Result<Self, ClientError> {
        // We only need readonly acccess

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_str("gzip")?);

        let reqwest_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client: ClientBuilder::new(reqwest_client)
                .with(AuthMiddleware::new())
                .build(),
            calendar_id: match calendar_id {
                Some(id) => id,
                None => std::env::var("GOOGLE_CALENDAR_ID")?,
            },
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
    ) -> Result<(Vec<models::Event>, Option<String>), ClientError> {
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
            .json::<models::Events>()
            .await?;

        Ok((events.items, events.next_page_token))
    }
}

fn build_query_parameters(
    date_range: &Option<DateRange>,
    event_count: &Option<u32>,
    next_page_token: &Option<String>,
) -> IndexMap<&'static str, String> {
    // Google requires rfc3339 format for the times with a fixed offset
    // see: https://developers.google.com/calendar/api/v3/reference/events/list

    let mut query_parameters: IndexMap<&'static str, String> = IndexMap::from([
        // filter out reoccuring events
        ("singleEvents", "true".to_owned()),
        // order ascending by start time
        ("orderBy", "startTime".to_owned()),
    ]);

    if let Some(range) = date_range {
        let start_date = range.start.to_rfc3339_opts(SecondsFormat::Secs, true);
        let end_date = range.end.to_rfc3339_opts(SecondsFormat::Secs, true);
        // limit the events by a time frame
        query_parameters.insert("timeMin", start_date);
        query_parameters.insert("timeMax", end_date);
    }

    if let Some(count) = event_count {
        // limit the number of events to a specific count
        query_parameters.insert("maxResults", count.to_string());
    }

    if let Some(token) = next_page_token {
        // page token returned by previous request to fetch the next page
        query_parameters.insert("pageToken", token.clone());
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

        let expected_parameters = IndexMap::from([
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
        ]);

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

        let expected_parameters = IndexMap::from([
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
        ]);

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

        let expected_parameters = IndexMap::from([
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
        ]);

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

        let expected_parameters = IndexMap::from([
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
        ]);

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

        let expected_parameters = IndexMap::from([
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
        ]);

        assert_eq!(expected_parameters, query_parameters);
    }
}