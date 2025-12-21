use super::Event;
use jiff::{SignedDuration, Zoned, civil::Weekday, fmt::strtime, tz::TimeZone};
use minijinja::value::{Object, Value};
use std::sync::Arc;

impl Object for Event {
    fn get_value(self: &Arc<Self>, field: &Value) -> Option<Value> {
        let start_date = self.start_date.to_zoned(TimeZone::system());

        let value = match field.as_str()? {
            "date" => Value::from(format_date(&start_date)),
            "time" => {
                let start_time = format_time(&start_date);
                let one_day = SignedDuration::from_hours(24);

                match self.end_date.map(|ts| ts.to_zoned(TimeZone::system())) {
                    Some(end_date) => {
                        let end_time = format_time(&end_date);

                        let date = if start_date.duration_until(&end_date) >= one_day {
                            // More than 24h between start and end date, format end date and time.
                            format!("{start_time} - {} {end_time}", format_date(&end_date))
                        } else {
                            // Less than 24h between start and end date, just format the end time.
                            format!("{start_time} - {end_time}")
                        };

                        Value::from(date)
                    }
                    None => Value::from(format!("{start_time}")),
                }
            }
            "title" => Value::from(&self.title),
            "description" => return self.description.as_ref().map(Value::from),
            _ => return None,
        };

        Some(value)
    }
}

fn format_time(date: &Zoned) -> strtime::Display<'_> {
    date.strftime("%H:%M")
}

fn format_date(date: &Zoned) -> String {
    format!(
        "{}, {}. {}",
        german_weekday(date),
        date.day(),
        german_month_name(date)
    )
}

fn german_weekday(date: &Zoned) -> &'static str {
    match date.weekday() {
        Weekday::Monday => "Mo",
        Weekday::Tuesday => "Di",
        Weekday::Wednesday => "Mi",
        Weekday::Thursday => "Do",
        Weekday::Friday => "Fr",
        Weekday::Saturday => "Sa",
        Weekday::Sunday => "So",
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    macro_rules! event {
        ($start_date:expr, $end_date:expr) => {
            Arc::new(Event {
                start_date: $start_date,
                end_date: $end_date,
                title: "The event".into(),
                description: None,
            })
        };
    }

    macro_rules! zoned {
        ($ts:expr) => {
            $ts.to_zoned(TimeZone::system())
        };
    }

    macro_rules! assert_field_value {
        ($ev:expr, $field:expr, $val:expr) => {
            assert_eq!($ev.get_value(&Value::from($field)), Some(Value::from($val)));
        };
    }

    #[test]
    fn custom_date_formatting() {
        let timestamp: Timestamp = "2025-03-05T18:00:00Z".parse().unwrap();
        let date = timestamp.to_zoned(TimeZone::UTC);
        assert_eq!(format_date(&date), "Mi, 5. März");
        assert_eq!(format_time(&date).to_string(), "18:00");
    }

    #[test]
    fn event_basics() {
        let event = event!("2025-02-05T18:00:00Z".parse().unwrap(), None);
        let expected_date = format_date(&zoned!(event.start_date));
        assert_field_value!(event, "title", &event.title);
        assert_field_value!(event, "date", expected_date);
    }

    #[test]
    fn event_time_without_end_date() {
        let event = event!("2025-02-05T18:00:00Z".parse().unwrap(), None);
        let expected_time = format_time(&zoned!(event.start_date)).to_string();
        assert_field_value!(event, "time", expected_time);
    }

    #[test]
    fn event_time_with_end_date() {
        let start_date = "2025-02-05T18:00:00Z".parse().unwrap();

        // End date less than 24h after start date.
        let end_date = "2025-02-06T17:59:59Z".parse().unwrap();

        let event = event!(start_date, Some(end_date));
        let expected_time = format!(
            "{} - {}",
            format_time(&zoned!(start_date)),
            format_time(&zoned!(end_date))
        );

        assert_field_value!(event, "time", expected_time);

        // End date more than 24h after start date.
        let end_date = "2025-02-06T18:00:00Z".parse().unwrap();

        let event = event!(start_date, Some(end_date));
        let expected_time = format!(
            "{} - {} {}",
            format_time(&zoned!(start_date)),
            format_date(&zoned!(end_date)),
            format_time(&zoned!(end_date))
        );

        assert_field_value!(event, "time", expected_time);
    }
}
