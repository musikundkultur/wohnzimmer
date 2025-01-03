use super::Event;
use jiff::{tz::TimeZone, SignedDuration, Zoned};
use minijinja::value::{Object, Value};
use std::sync::Arc;

impl Object for Event {
    fn get_value(self: &Arc<Self>, field: &Value) -> Option<Value> {
        let start_date = self.start_date.to_zoned(TimeZone::system());

        let value = match field.as_str()? {
            "date" => Value::from(german_date(&start_date)),
            "time" => {
                let start_time = start_date.strftime("%H:%M");
                let one_day = SignedDuration::from_hours(24);

                let formatted = match self.end_date.map(|ts| ts.to_zoned(TimeZone::system())) {
                    Some(end_date) if start_date.duration_until(&end_date) > one_day => {
                        // More than 24h between start and end date, format end date and time.
                        format!(
                            "{start_time} - {} {}",
                            german_date(&end_date),
                            end_date.strftime("%H:%M")
                        )
                    }
                    Some(end_date) => {
                        // Less than 24h between start and end date, just format the end time.
                        format!("{start_time} - {}", end_date.strftime("%H:%M"))
                    }
                    None => format!("{start_time}"),
                };

                Value::from(formatted)
            }
            "title" => Value::from(self.title.clone()),
            _ => return None,
        };

        Some(value)
    }
}

fn german_date(date: &Zoned) -> String {
    format!("{}. {}", date.strftime("%e"), german_month_name(date))
}

fn german_month_name(date: &Zoned) -> &'static str {
    match date.month() {
        1 => "Januar",
        2 => "Februar",
        3 => "März",
        4 => "April",
        5 => "Mai",
        6 => "Juni",
        7 => "Juli",
        8 => "August",
        9 => "September",
        10 => "Oktober",
        11 => "November",
        12 => "Dezember",
        _ => unreachable!("month can only be in range 1..=12"),
    }
}
