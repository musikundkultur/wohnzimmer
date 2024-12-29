use super::Event;
use chrono::Locale;
use minijinja::value::{Object, Value};
use std::sync::Arc;

impl Object for Event {
    fn get_value(self: &Arc<Self>, field: &Value) -> Option<Value> {
        let value = match field.as_str()? {
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
