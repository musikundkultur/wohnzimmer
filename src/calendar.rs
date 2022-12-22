use crate::google_calendar;

use super::Result;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Locale, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Type alias for a date represented in UTC.
pub type UtcDate = DateTime<Utc>;

/// Represents a single calendar event.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The date of the event.
    #[serde(serialize_with = "serialize_german_date")]
    pub date: UtcDate,
    /// The event title.
    pub title: String,
}

/// Type alias for calendar events grouped by year.
pub type EventsByYear = IndexMap<i32, Vec<Event>>;

/// Represents sources of calendar events.
#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum EventSourceKind {
    /// Use static events from the application configuration.
    Static,
    /// Load events from Google Calendar.
    GoogleCalendar,
}

/// Trait that needs to be implemented by a source of calendar events.
#[async_trait]
pub trait EventSource: Send + Sync {
    /// Filters events between a start date (inclusive) and an end date (exclusive).
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>>;
}

/// An `EventSource` that returns events from a static list.
pub struct StaticEventSource {
    events: Vec<Event>,
}

impl StaticEventSource {
    /// Creates a new `StaticEventSource` from an iterator.
    pub fn new<I>(iter: I) -> StaticEventSource
    where
        I: IntoIterator,
        I::Item: Into<Event>,
    {
        let mut events: Vec<Event> = iter.into_iter().map(Into::into).collect();
        // Ensure events are always sorted by date.
        events.sort_by_key(|event| event.date);

        StaticEventSource { events }
    }
}

#[async_trait]
impl EventSource for StaticEventSource {
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        let events = self
            .events
            .iter()
            .filter(|event| range.contains(&event.date))
            .cloned()
            .collect();

        Ok(events)
    }
}

#[derive(Debug)]
pub struct GoogleCalendarEventSource {
    client: google_calendar::Client,
}

impl GoogleCalendarEventSource {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            client: google_calendar::Client::new().await?,
        })
    }
}

impl From<google_calendar::models::Event> for Event {
    fn from(ev: google_calendar::models::Event) -> Self {
        Self {
            date: ev.start.date_time,
            title: ev.summary,
        }
    }
}

#[async_trait]
impl EventSource for GoogleCalendarEventSource {
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        Ok(self
            .client
            .get_events(Some(range), None, None)
            .await?
            .0
            .into_iter()
            .map(Event::from)
            .collect())
    }
}

/// The `Calendar` type wraps an event source with additional functionality.
pub struct Calendar {
    event_source: Box<dyn EventSource + 'static>,
}

impl Calendar {
    /// Creates a new `Calendar` from an event source.
    pub fn new<T>(event_source: T) -> Calendar
    where
        T: EventSource + 'static,
    {
        Calendar {
            event_source: Box::new(event_source),
        }
    }

    /// Filters events between a start date (inclusive) and an end date (exclusive).
    pub async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        self.event_source.get_events(range).await
    }

    /// Builds an index of event year to list of events. This is used to avoid having complicated
    /// logic for displaying events by year in HTML templates.
    pub async fn get_events_by_year(&self, range: Range<UtcDate>) -> Result<EventsByYear> {
        let events = self.get_events(range).await?;
        let mut events_by_year: EventsByYear = IndexMap::new();

        events.into_iter().for_each(|event| {
            events_by_year
                .entry(event.date.year())
                .or_default()
                .push(event);
        });

        Ok(events_by_year)
    }
}

/// Serializes a date as `%e. %B` using german as locale, e.g. `13. Dezember`. This is used by
/// minijinja in templates. The year display is handled by other means in the HTML template.
fn serialize_german_date<S>(date: &UtcDate, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = date
        .with_timezone(&chrono_tz::CET)
        .format_localized("%e. %B", Locale::de_DE)
        .to_string();
    serializer.serialize_str(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use indexmap::indexmap;

    macro_rules! date {
        ($y:expr, $m:expr, $d:expr) => {
            Utc.with_ymd_and_hms($y, $m, $d, 0, 0, 0).unwrap()
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

    #[actix_rt::test]
    async fn events_between() {
        let calendar = Calendar::new(StaticEventSource::new([
            event!("a", 2022, 12, 30),
            event!("b", 2022, 12, 31),
            event!("c", 2023, 1, 1),
            event!("d", 2023, 1, 2),
            event!("e", 2023, 1, 1),
        ]));

        assert_eq!(
            calendar
                .get_events(date!(2022, 12, 31)..date!(2023, 1, 2))
                .await
                .unwrap(),
            vec![
                event!("b", 2022, 12, 31),
                event!("c", 2023, 1, 1),
                event!("e", 2023, 1, 1)
            ]
        );
        assert_eq!(
            calendar
                .get_events(date!(2022, 12, 31)..date!(2023, 1, 1))
                .await
                .unwrap(),
            vec![event!("b", 2022, 12, 31)]
        );
    }

    #[actix_rt::test]
    async fn events_by_year() {
        let calendar = Calendar::new(StaticEventSource::new([
            event!("a", 2022, 12, 30),
            event!("b", 2022, 12, 31),
            event!("c", 2023, 1, 1),
            event!("d", 2023, 1, 2),
            event!("e", 2023, 1, 1),
        ]));

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

        assert_eq!(
            calendar
                .get_events_by_year(date!(2022, 1, 1)..date!(2023, 12, 31))
                .await
                .unwrap(),
            expected
        );
    }
}
