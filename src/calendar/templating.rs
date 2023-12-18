use super::Event;
use chrono::Locale;
use minijinja::value::{StructObject, Value};

impl StructObject for Event {
    fn get_field(&self, name: &str) -> Option<Value> {
        let value = match name {
            "date" => {
                let start_date = self
                    .start_date
                    .with_timezone(&chrono_tz::CET)
                    .format_localized("%e. %B", Locale::de_DE);

                Value::from(format!("{start_date}"))
            }
            "time" => {
                let start_time = self
                    .start_date
                    .with_timezone(&chrono_tz::CET)
                    .format("%k:%M");

                match self.end_date {
                    Some(end_date) => {
                        let end_date = if self.start_date.date_naive() == end_date.date_naive() {
                            // Single-day event, just format the end time.
                            end_date.with_timezone(&chrono_tz::CET).format("%k:%M")
                        } else {
                            // Multi-day event, format end date and time.
                            end_date
                                .with_timezone(&chrono_tz::CET)
                                .format_localized("%e. %B %k:%M", Locale::de_DE)
                        };

                        Value::from(format!("{start_time} - {end_date}"))
                    }
                    None => Value::from(format!("{start_time}")),
                }
            }
            "title" => Value::from(self.title.clone()),
            _ => return None,
        };

        Some(value)
    }
}
