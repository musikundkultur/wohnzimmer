extern crate dotenv;

use chrono::{Months, Utc};
use dotenv::dotenv;
use std::error::Error;

use wohnzimmer::calendar::{Calendar, GoogleCalendarEventSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    dotenv().ok();

    let calendar = Calendar::new(GoogleCalendarEventSource::new()?);

    let now = Utc::now();
    let one_month_ago = now - Months::new(1);
    let in_six_months = now + Months::new(6);

    let events_by_year = calendar
        .get_events_by_year(one_month_ago..in_six_months)
        .await?;

    println!("{:#?}", events_by_year);

    Ok(())
}
