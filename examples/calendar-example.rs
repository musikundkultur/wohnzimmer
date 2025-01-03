extern crate dotenv;

use dotenv::dotenv;
use jiff::{ToSpan, Zoned};
use std::error::Error;
use wohnzimmer::calendar::{Calendar, GoogleCalendarEventSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();

    let calendar = Calendar::new(GoogleCalendarEventSource::new().await?);
    calendar.sync_once().await?;

    let now = Zoned::now();
    let start = &now - 1.months();
    let end = &now + 6.months();

    let events_by_year = calendar
        .get_events_by_year(start.timestamp()..end.timestamp())
        .await?;

    println!("{:#?}", events_by_year);

    Ok(())
}
