use super::Result;
use crate::{CalendarCacheConfig, CalendarConfig};
use async_trait::async_trait;
use cached::{CachedAsync, TimedSizedCache};
use chrono::{DateTime, Datelike, Locale, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use tokio::sync::Mutex;

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

pub struct CachingEventSource<T> {
    cache: Mutex<TimedSizedCache<Range<UtcDate>, Vec<Event>>>,
    inner: T,
}

impl<T> CachingEventSource<T> {
    /// Creates a new `CachingEventSource` from an inner `T`, a cache size and a cache entry TTL in
    /// seconds.
    pub fn new(inner: T, size: usize, ttl_seconds: u64) -> CachingEventSource<T> {
        CachingEventSource {
            cache: Mutex::new(TimedSizedCache::with_size_and_lifespan(
                size.max(1), // Cache size has to be at least 1.
                ttl_seconds,
            )),
            inner,
        }
    }

    /// Creates a new `CachingEventSource` from an inner `T` and a cache configuration.
    pub fn from_config(inner: T, config: &CalendarCacheConfig) -> CachingEventSource<T> {
        let size = config.size.unwrap_or(100).max(1);
        let ttl_seconds = config.ttl_seconds.unwrap_or(10);

        log::info!("creating event cache with cache size {size} and TTL of {ttl_seconds}s");

        CachingEventSource::new(inner, size, ttl_seconds)
    }
}

#[async_trait]
impl<T> EventSource for CachingEventSource<T>
where
    T: EventSource,
{
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        self.cache
            .lock()
            .await
            .try_get_or_set_with(range.clone(), || self.inner.get_events(range))
            .await
            .cloned()
    }
}

#[async_trait]
impl<T> EventSource for &T
where
    T: EventSource + ?Sized,
{
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        (*self).get_events(range).await
    }
}

#[async_trait]
impl<T> EventSource for Box<T>
where
    T: EventSource + ?Sized,
{
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        (**self).get_events(range).await
    }
}

/// The `Calendar` type wraps an event source with additional functionality.
pub struct Calendar {
    event_source: Box<dyn EventSource>,
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

    /// Creates a new `Calendar` from configuration.
    pub fn from_config(config: &CalendarConfig) -> Calendar {
        let event_source = match config.event_source {
            EventSourceKind::Static => Box::new(StaticEventSource::new(config.events.clone())),
            EventSourceKind::GoogleCalendar => {
                // @TODO(mohmann): we need to create a `GoogleCalendarEventSource` implementation.
                log::warn!("Google Calendar support is not implemented yet, falling back to static events from config");
                Box::new(StaticEventSource::new(config.events.clone()))
            }
        };

        if config.cache.enabled {
            Calendar::new(CachingEventSource::from_config(event_source, &config.cache))
        } else {
            Calendar::new(event_source)
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
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    #[actix_rt::test]
    async fn caching_event_source() {
        // A fake `EventSource` which just counts invocations of `get_events` and returns a fake
        // event.
        struct Counter(AtomicUsize);

        #[async_trait]
        impl EventSource for Counter {
            async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Event {
                    title: "event".into(),
                    date: range.start.clone(),
                }])
            }
        }

        let counter = Counter(AtomicUsize::new(0));

        let es = CachingEventSource::new(&counter, 10, 1);
        let range1 = date!(2022, 1, 1)..date!(2022, 1, 2);
        let range2 = date!(2022, 1, 2)..date!(2022, 1, 3);

        assert_eq!(
            es.get_events(range1.clone()).await.unwrap(),
            vec![event!("event", 2022, 1, 1)]
        );

        assert_eq!(
            es.get_events(range2.clone()).await.unwrap(),
            vec![event!("event", 2022, 1, 2)]
        );

        // This call is cached.
        assert_eq!(
            es.get_events(range1.clone()).await.unwrap(),
            vec![event!("event", 2022, 1, 1)]
        );

        assert_eq!(counter.0.load(Ordering::Relaxed), 2);

        // Let the cache expire.
        tokio::time::sleep(tokio::time::Duration::new(1, 0)).await;

        // These calls are not cached, because the cache expired.
        es.get_events(range2).await.unwrap();
        es.get_events(range1).await.unwrap();

        assert_eq!(counter.0.load(Ordering::Relaxed), 4);
    }
}
