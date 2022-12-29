extern crate dotenv;

use dotenv::dotenv;
use std::error::Error;
use chrono::{Months, Utc};

use wohnzimmer::calendar::{Calendar, GoogleCalendarEventSource, EventsByYear};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();

    let calendar = Calendar::new(GoogleCalendarEventSource::new().await?);

    let now = Utc::now();
    let one_month_ago = now - Months::new(1);
    let in_six_months = now + Months::new(6);


    let events_by_year = calendar
        .get_events_by_year(one_month_ago..in_six_months)
        .await
        .unwrap_or_else(|err| {
            // Handle this error gracefully by just displaying no events instead of sending a 500
            // response.
            log::error!("failed to fetch calendar events: {}", err);
            EventsByYear::default()
        });

    println!("{:#?}", events_by_year);

    Ok(())
}
