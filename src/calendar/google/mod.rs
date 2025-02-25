pub mod models;

use google_cloud_auth::token::DefaultTokenSourceProvider;
use google_cloud_token::{TokenSource, TokenSourceProvider};
use http::Extensions;
use indexmap::IndexMap;
use jiff::Timestamp;
use reqwest::header::{ACCEPT_ENCODING, AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::{Request, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Middleware, Next};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Error while authenticating with google.
    #[error("failed to authenticate: {0}")]
    GCloudAuth(#[from] google_cloud_auth::error::Error),

    /// Error while making a http request.
    #[error("failure requesting remote resource: {0}")]
    Request(#[from] reqwest::Error),

    /// Error while executing some middleware code.
    #[error("request middleware failed with: {0}")]
    RequestMiddleware(#[from] reqwest_middleware::Error),

    /// Error while building http headers.
    #[error("encountered invalid HTTP header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    /// Error when `GOOGLE_CALENDAR_ID` environment variable is not set.
    #[error("missing required environment variable `GOOGLE_CALENDAR_ID`")]
    MissingCalendarID,

    /// Error while parsing a JSON response.
    #[error("failed to parse response as JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// Error while obtaining an authentication token.
    #[error("failed to obtain authentication token: {0}")]
    Token(String),
}

impl From<ClientError> for reqwest_middleware::Error {
    fn from(err: ClientError) -> Self {
        reqwest_middleware::Error::Middleware(anyhow::Error::new(err))
    }
}

struct AuthMiddleware {
    token_source: Arc<dyn TokenSource>,
}

impl AuthMiddleware {
    fn new(token_source: Arc<dyn TokenSource>) -> AuthMiddleware {
        AuthMiddleware { token_source }
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
        let token = self
            .token_source
            .token()
            .await
            .map_err(|err| ClientError::Token(err.to_string()))?;

        let mut header = HeaderValue::try_from(token).map_err(ClientError::from)?;
        header.set_sensitive(true);
        req.headers_mut().insert(AUTHORIZATION, header);
        next.run(req, extensions).await
    }
}

/// Google calendar client for making requests to the google calendar api
#[derive(Debug)]
pub struct GoogleCalendarClient {
    client: ClientWithMiddleware,
    calendar_id: String,
}

impl GoogleCalendarClient {
    /// Create a new google calendar client it will fetch the service host credentials from the
    /// environment either via the GOOGLE_APPLICATION_CREDENTIALS variable pointing to the json
    /// key file generated in the google cloud console for the account or via a the
    /// GOOGLE_APPLICATION_CREDENTIALS_JSON variable containing the content of said json file
    /// encoded as base64. It will further fetch the id of the calendar that it will query from
    /// the GOOGLE_CALENDAR_ID environment variable.
    pub async fn new() -> Result<GoogleCalendarClient, ClientError> {
        let calendar_id = match std::env::var("GOOGLE_CALENDAR_ID") {
            Ok(calendar_id) => calendar_id,
            Err(_) => return Err(ClientError::MissingCalendarID),
        };

        let scopes = ["https://www.googleapis.com/auth/calendar.readonly"];
        let config = google_cloud_auth::project::Config::default().with_scopes(&scopes);

        let token_source = DefaultTokenSourceProvider::new(config)
            .await?
            .token_source();

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_str("gzip")?);

        let client = ClientBuilder::new(
            reqwest::Client::builder()
                .default_headers(headers)
                .timeout(Duration::from_secs(10))
                .build()?,
        )
        .with(AuthMiddleware::new(token_source))
        .build();

        Ok(GoogleCalendarClient {
            client,
            calendar_id,
        })
    }

    /// Queries events from the google calendar. The query can be filtered by a DateRange and the
    /// number of results can be limited to a certain number of events in which case the Result
    /// might return a page token for pagination purposes that should be used in the next request
    /// to get the next page of events. This function will return a a list of events that fulfill
    /// the given requirements.
    pub async fn get_events(
        &self,
        date_range: Option<Range<Timestamp>>,
        event_count: Option<u32>,
        next_page_token: Option<String>,
    ) -> Result<(Vec<models::Event>, Option<String>), ClientError> {
        let events_request = self.client.get(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            self.calendar_id
        ));

        let query = build_query_parameters(&date_range, &event_count, &next_page_token);

        let events = events_request
            .query(&query)
            .send()
            .await?
            .json::<models::Events>()
            .await?;

        log::debug!("fetched {} events from Google Calendar", events.items.len());

        Ok((events.items, events.next_page_token))
    }
}

fn build_query_parameters(
    date_range: &Option<Range<Timestamp>>,
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
        // limit the events by a time frame
        query_parameters.insert("timeMin", range.start.to_string());
        query_parameters.insert("timeMax", range.end.to_string());
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
    use super::*;

    #[test]
    fn build_query_parameters_without_parameters() {
        let query_parameters = build_query_parameters(&None, &None, &None);

        let expected_parameters =
            IndexMap::from([("singleEvents", "true"), ("orderBy", "startTime")]);

        assert_eq!(expected_parameters, query_parameters);
    }
    #[test]
    fn build_query_parameters_without_page_token_and_event_count_limit() {
        let start_date = "1996-12-19T16:39:57-08:00".parse().unwrap();
        let end_date = "1996-12-19T16:39:57-09:00".parse().unwrap();

        let query_parameters = build_query_parameters(&Some(start_date..end_date), &None, &None);

        let expected_parameters = IndexMap::from([
            ("singleEvents", "true".to_owned()),
            ("orderBy", "startTime".to_owned()),
            ("timeMin", start_date.to_string()),
            ("timeMax", end_date.to_string()),
        ]);

        assert_eq!(expected_parameters, query_parameters);
    }

    #[test]
    fn build_query_parameters_without_page_token() {
        let start_date = "1996-12-19T16:39:57-08:00".parse().unwrap();
        let end_date = "1996-12-19T16:39:57-09:00".parse().unwrap();

        let query_parameters =
            build_query_parameters(&Some(start_date..end_date), &Some(30), &None);

        let expected_parameters = IndexMap::from([
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("timeMin", "1996-12-20T00:39:57Z"),
            ("timeMax", "1996-12-20T01:39:57Z"),
            ("maxResults", "30"),
        ]);

        assert_eq!(expected_parameters, query_parameters);
    }

    #[test]
    fn build_query_parameters_without_event_count() {
        let start_date = "1996-12-19T16:39:57-08:00".parse().unwrap();
        let end_date = "1996-12-19T16:39:57-09:00".parse().unwrap();

        let query_parameters =
            build_query_parameters(&Some(start_date..end_date), &None, &Some("abcd".to_owned()));

        let expected_parameters = IndexMap::from([
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("timeMin", "1996-12-20T00:39:57Z"),
            ("timeMax", "1996-12-20T01:39:57Z"),
            ("pageToken", "abcd"),
        ]);

        assert_eq!(expected_parameters, query_parameters);
    }

    #[test]
    fn build_query_parameters_with_event_count_and_page_token() {
        let start_date = "1996-12-19T16:39:57-08:00".parse().unwrap();
        let end_date = "1996-12-19T16:39:57-09:00".parse().unwrap();

        let query_parameters = build_query_parameters(
            &Some(start_date..end_date),
            &Some(30),
            &Some("abcd".to_owned()),
        );

        let expected_parameters = IndexMap::from([
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("timeMin", "1996-12-20T00:39:57Z"),
            ("timeMax", "1996-12-20T01:39:57Z"),
            ("maxResults", "30"),
            ("pageToken", "abcd"),
        ]);

        assert_eq!(expected_parameters, query_parameters);
    }
}
