use chrono::TimeZone;
use chrono::{Datelike, Local, Locale, NaiveDate};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

/// Type alias for calendar events grouped by year.
pub type EventsByYear = IndexMap<i32, Vec<Event>>;

/// Wrapper type for a collections of calendar events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Events(Vec<Event>);

impl Events {
    /// Loads calendar events from the provided YAML config file as `path`.
    pub fn from_path<P>(path: P) -> io::Result<Events>
    where
        P: AsRef<Path>,
    {
        let data = fs::read_to_string(path)?;

        // The config file is a map of vectors where the map keys are only there for visual
        // grouping by year but have no meaning otherwise.
        match serde_yaml::from_str::<EventsByYear>(&data) {
            Ok(events) => Ok(events.into_values().flatten().collect()),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err)),
        }
    }

    /// Filters events between a start date (inclusive) and an end date (exclusive) and returns a
    /// new `Events` value.
    pub fn between(&self, from: &NaiveDate, to: &NaiveDate) -> Events {
        self.0
            .iter()
            .filter(|event| &event.date >= from && &event.date < to)
            .cloned()
            .collect()
    }

    /// Builds an index of event year to list of events. This is used to avoid having complicated
    /// logic for displaying events by year in HTML templates.
    pub fn by_year(self) -> EventsByYear {
        let mut events_by_year: EventsByYear = IndexMap::new();

        self.0.into_iter().for_each(|event| {
            events_by_year
                .entry(event.date.year())
                .or_default()
                .push(event);
        });

        events_by_year
    }
}

impl<T> FromIterator<T> for Events
where
    T: Into<Event>,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut events: Vec<Event> = iter.into_iter().map(Into::into).collect();
        // Ensure events are always sorted by date.
        events.sort_by_key(|event| event.date);
        Events(events)
    }
}

/// Represents a single calendar event.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The date of the event.
    #[serde(with = "custom_date_format")]
    pub date: NaiveDate,
    /// The event title.
    pub title: String,
}

mod custom_date_format {
    use super::*;

    /// Serializes a date as `%e. %B` using german as locale, e.g. `13. Dezember`. This is used by
    /// minijinja in templates. The year display is handled by other means in the HTML template.
    pub fn serialize<S>(date: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .unwrap()
            .format_localized("%e. %B", Locale::de_DE)
            .to_string();
        serializer.serialize_str(&s)
    }

    /// Deserializes a date from a string formatted as `%Y-%m-%d`, e.g. `2022-12-13`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;

    macro_rules! date {
        ($y:expr, $m:expr, $d:expr) => {
            NaiveDate::from_ymd_opt($y, $m, $d).unwrap()
        };
    }

    macro_rules! event {
        ($title:expr, $y:expr, $m:expr, $d:expr) => {
            Event {
                title: $title.into(),
                date: date!($y, $m, $d),
            }
        };
    }

    #[test]
    fn events_between() {
        let events = Events::from_iter([
            event!("a", 2022, 12, 30),
            event!("b", 2022, 12, 31),
            event!("c", 2023, 1, 1),
            event!("d", 2023, 1, 2),
            event!("e", 2023, 1, 1),
        ]);

        assert_eq!(
            events.between(&date!(2022, 12, 31), &date!(2023, 1, 2)),
            Events::from_iter([
                event!("b", 2022, 12, 31),
                event!("c", 2023, 1, 1),
                event!("e", 2023, 1, 1)
            ])
        );
        assert_eq!(
            events.between(&date!(2022, 12, 31), &date!(2023, 1, 1)),
            Events::from_iter([event!("b", 2022, 12, 31)])
        );
    }

    #[test]
    fn events_by_year() {
        let events = Events::from_iter([
            event!("a", 2022, 12, 30),
            event!("b", 2022, 12, 31),
            event!("c", 2023, 1, 1),
            event!("d", 2023, 1, 2),
            event!("e", 2023, 1, 1),
        ]);

        let expected = indexmap! {
            2022 => vec![
                event!("a", 2022, 12, 30),
                event!("b", 2022, 12, 31),
            ],
            2023 => vec![
                event!("c", 2023, 1, 1),
                event!("e", 2023, 1, 1),
                event!("d", 2023, 1, 2),
            ]
        };

        assert_eq!(events.by_year(), expected);
    }
}
