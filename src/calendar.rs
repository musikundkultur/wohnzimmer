pub mod google;

use super::Result;
use crate::{CalendarCacheConfig, CalendarConfig};
use async_trait::async_trait;
use cached::{Cached, SizedCache};
use chrono::{DateTime, Datelike, Locale, Utc};
use google::GoogleCalendarClient;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::{Duration, Instant};

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
    client: GoogleCalendarClient,
}

impl GoogleCalendarEventSource {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: GoogleCalendarClient::new(None)?,
        })
    }
}

impl From<google::models::Event> for Event {
    fn from(ev: google::models::Event) -> Self {
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

#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    expiry: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> CacheEntry<T> {
        CacheEntry {
            value,
            expiry: Instant::now() + ttl,
        }
    }

    fn extend_lifetime(&mut self, ttl: Duration) {
        self.expiry = Instant::now() + ttl;
    }

    fn is_expired(&self) -> bool {
        self.expiry < Instant::now()
    }
}

type CacheKey = Range<UtcDate>;
type EventCache = SizedCache<CacheKey, CacheEntry<Vec<Event>>>;

pub struct CachedEventSource<T> {
    cache: Arc<Mutex<EventCache>>,
    inner: Arc<T>,
    ttl: Duration,
}

impl<T> CachedEventSource<T> {
    /// Creates a new `CachedEventSource` from an inner `T` and a cache entry TTL.
    pub fn new(inner: T, ttl: Duration) -> CachedEventSource<T> {
        CachedEventSource {
            cache: Arc::new(Mutex::new(EventCache::with_size(10))), // Size of 10 is plenty for our use case.
            inner: Arc::new(inner),
            ttl,
        }
    }

    /// Creates a new `CachedEventSource` from an inner `T` and a cache configuration.
    pub fn from_config(inner: T, config: &CalendarCacheConfig) -> CachedEventSource<T> {
        let ttl_seconds = config.ttl_seconds.unwrap_or(10);

        log::info!("creating event cache with TTL of {ttl_seconds}s");

        CachedEventSource::new(inner, Duration::from_secs(ttl_seconds))
    }
}

impl<T> CachedEventSource<T>
where
    T: EventSource + 'static,
{
    fn spawn_refresh_task(&self, key: CacheKey, range: Range<UtcDate>) {
        let event_source = self.inner.clone();
        let cache = self.cache.clone();
        let ttl = self.ttl;

        task::spawn(async move {
            log::debug!("refreshing cache key `{:?}`", key);

            match event_source.get_events(range).await {
                Ok(events) => {
                    let entry = CacheEntry::new(events, ttl);
                    cache.lock().await.cache_set(key, entry);
                }
                Err(err) => {
                    log::error!("failed to refresh cache key `{:?}`: {}", key, err);
                }
            }
        });
    }
}

#[async_trait]
impl<T> EventSource for CachedEventSource<T>
where
    T: EventSource + 'static,
{
    async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        let key = range.clone();

        let mut cache = self.cache.lock().await;

        match cache.cache_get_mut(&key) {
            Some(entry) if entry.is_expired() => {
                log::debug!("cache hit for `{:?}` (expiring)", key);

                // Extend the lifetime of the expired entry by another TTL period before spawning
                // the background refresh task. We do this to prevent other in-flight requests from
                // triggering another refresh for this key while its background refresh task is
                // still running.
                entry.extend_lifetime(self.ttl);
                self.spawn_refresh_task(key, range);
                Ok(entry.value.clone())
            }
            Some(entry) => {
                log::debug!("cache hit for `{:?}`", key);

                Ok(entry.value.clone())
            }
            None => {
                log::debug!("cache miss for `{:?}`, fetching events from source", key);

                let events = self.inner.get_events(range.clone()).await?;
                let entry = CacheEntry::new(events.clone(), self.ttl);
                cache.cache_set(key, entry);
                Ok(events)
            }
        }
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

#[async_trait]
impl<T> EventSource for Arc<T>
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
    pub fn from_config(config: &CalendarConfig) -> Result<Calendar> {
        let event_source: Box<dyn EventSource> = match config.event_source {
            EventSourceKind::Static => Box::new(StaticEventSource::new(config.events.clone())),
            EventSourceKind::GoogleCalendar => Box::new(GoogleCalendarEventSource::new()?),
        };

        let calendar = if config.cache.enabled {
            Calendar::new(CachedEventSource::from_config(event_source, &config.cache))
        } else {
            Calendar::new(event_source)
        };

        Ok(calendar)
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
    async fn cached_event_source() {
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

        let counter = Arc::new(Counter(AtomicUsize::new(0)));

        let es = CachedEventSource::new(counter.clone(), Duration::from_millis(100));
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
        tokio::time::sleep(Duration::from_millis(100)).await;

        // These calls trigger a background cache refresh, because the cache expired.
        es.get_events(range2).await.unwrap();
        es.get_events(range1).await.unwrap();

        // Give the background tasks some time to finish.
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(counter.0.load(Ordering::Relaxed), 4);
    }
}
