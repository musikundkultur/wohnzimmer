mod models;

extern crate dotenv;

use dotenv::dotenv;
use reqwest::header;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    // The id of the calendar that we want to query. It can be found in the calendar settings in
    // google calendar. It is stored in the .env file CALENDAR_ID environment variable
    let google_calendar_id = std::env::var("CALENDAR_ID")?;

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

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let events = client
        .get(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            google_calendar_id
        ))
        .send()
        .await?
        .text()
        .await?;

    let events: models::Events = serde_json::from_str(&events)?;
    println!("Events: {:#?}", events);

    Ok(())
}
