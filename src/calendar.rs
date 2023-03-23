pub mod google;

use super::Result;
use crate::CalendarConfig;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Locale, Months, Utc};
use google::GoogleCalendarClient;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::io;
use std::ops::Range;
use std::sync::Arc;
use tokio::sync::oneshot::{self, Receiver, Sender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Duration;

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
    /// Fetches events from the source.
    async fn fetch_events(&self) -> Result<Vec<Event>>;
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
        StaticEventSource {
            events: iter.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl EventSource for StaticEventSource {
    async fn fetch_events(&self) -> Result<Vec<Event>> {
        Ok(self.events.clone())
    }
}

#[derive(Debug)]
pub struct GoogleCalendarEventSource {
    client: GoogleCalendarClient,
}

impl GoogleCalendarEventSource {
    pub async fn new() -> Result<GoogleCalendarEventSource> {
        Ok(GoogleCalendarEventSource {
            client: GoogleCalendarClient::new().await?,
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
    async fn fetch_events(&self) -> Result<Vec<Event>> {
        let now = Utc::now();
        let one_month_ago = now - Months::new(1);
        let in_six_months = now + Months::new(6);

        let events = self
            .client
            .get_events(Some(one_month_ago..in_six_months), None, None)
            .await?;

        Ok(events.0.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl<T> EventSource for Box<T>
where
    T: EventSource + ?Sized,
{
    async fn fetch_events(&self) -> Result<Vec<Event>> {
        (**self).fetch_events().await
    }
}

#[async_trait]
impl<T> EventSource for Arc<T>
where
    T: EventSource + ?Sized,
{
    async fn fetch_events(&self) -> Result<Vec<Event>> {
        (**self).fetch_events().await
    }
}

/// The `Calendar` type wraps an event source with additional functionality.
#[derive(Clone)]
pub struct Calendar {
    event_source: Arc<dyn EventSource>,
    events: Arc<Mutex<Vec<Event>>>,
}

impl Calendar {
    /// Creates a new `Calendar` from an event source.
    pub fn new<T>(event_source: T) -> Calendar
    where
        T: EventSource + 'static,
    {
        Calendar {
            event_source: Arc::new(event_source),
            events: Default::default(),
        }
    }

    /// Creates a new `Calendar` from configuration.
    pub async fn from_config(config: &CalendarConfig) -> Result<Calendar> {
        let event_source: Box<dyn EventSource> = match config.event_source {
            EventSourceKind::Static => Box::new(StaticEventSource::new(config.events.clone())),
            EventSourceKind::GoogleCalendar => Box::new(GoogleCalendarEventSource::new().await?),
        };

        Ok(Calendar::new(event_source))
    }

    /// Filters events between a start date (inclusive) and an end date (exclusive).
    pub async fn get_events(&self, range: Range<UtcDate>) -> Result<Vec<Event>> {
        let events = self.events.lock().await.clone();

        let events = events
            .into_iter()
            .filter(|event| range.contains(&event.date))
            .collect();

        Ok(events)
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

    /// Synchronize events from the source into the calendar once.
    pub async fn sync_once(&self) -> Result<()> {
        log::debug!("synchronizing calendar events");
        let mut events = self.event_source.fetch_events().await?;
        // Ensure events are always sorted by date.
        events.sort_by_key(|event| event.date);
        *self.events.lock().await = events;
        Ok(())
    }

    /// Starts to periodically sync the calendar every `interval` until a message is received via
    /// `stop`.
    async fn start_sync(&self, period: Duration, mut stop: Receiver<()>) -> Result<()> {
        log::info!("synchronizing calendar events every {:?}", period);
        let mut interval = tokio::time::interval(period);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(err) = self.sync_once().await {
                        log::error!("failed to sync calendar events: {err}");
                    }
                }
                _ = &mut stop => {
                    log::info!("stopping calendar sync");
                    return Ok(());
                }
            }
        }
    }

    /// Starts a background task to sync calendar events. Returns a `SyncTaskHandle` to stop the
    /// sync.
    pub async fn spawn_sync_task(&self, period: Duration) -> SyncTaskHandle {
        let calendar = self.clone();
        let (stop_tx, stop_rx) = oneshot::channel();

        let join_handle = tokio::spawn(async move {
            let _ = calendar.start_sync(period, stop_rx).await;
        });

        SyncTaskHandle {
            join_handle,
            stop_tx,
        }
    }
}

/// A handle for stopping a calendar sync task.
pub struct SyncTaskHandle {
    join_handle: JoinHandle<()>,
    stop_tx: Sender<()>,
}

impl SyncTaskHandle {
    /// Stops the calendar sync task. Blocks until the background task is finished.
    pub async fn stop(self) -> io::Result<()> {
        if let Ok(_) = self.stop_tx.send(()) {
            self.join_handle.await?;
        }

        Ok(())
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
        calendar.sync_once().await.unwrap();

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
        calendar.sync_once().await.unwrap();

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
    async fn calendar_sync() {
        // A fake `EventSource` which just counts invocations of `fetch_events` and returns a fake
        // event.
        struct Counter(AtomicUsize);

        #[async_trait]
        impl EventSource for Counter {
            async fn fetch_events(&self) -> Result<Vec<Event>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Event {
                    title: "event".into(),
                    date: date!(2023, 1, 1),
                }])
            }
        }

        let counter = Arc::new(Counter(AtomicUsize::new(0)));

        let calendar = Arc::new(Calendar::new(counter.clone()));

        let range1 = date!(2023, 1, 1)..date!(2023, 1, 2);
        let range2 = date!(2023, 1, 2)..date!(2023, 1, 3);

        // Initially, there are no events because no sync happened.
        assert_eq!(calendar.get_events(range1.clone()).await.unwrap(), vec![]);

        calendar.sync_once().await.unwrap();

        assert_eq!(
            calendar.get_events(range1.clone()).await.unwrap(),
            vec![event!("event", 2023, 1, 1)]
        );

        // No events in range.
        assert_eq!(calendar.get_events(range2.clone()).await.unwrap(), vec![]);

        // We only fetched the events once from the source.
        assert_eq!(counter.0.load(Ordering::Relaxed), 1);

        let sync_task_handle = calendar.spawn_sync_task(Duration::from_millis(10)).await;

        tokio::time::sleep(Duration::from_millis(15)).await;

        // Stop the sync again.
        sync_task_handle.stop().await.unwrap();

        // Manual `sync_one` above + initial sync + sync after 10ms = 3 syncs.
        assert_eq!(counter.0.load(Ordering::Relaxed), 3);

        tokio::time::sleep(Duration::from_millis(15)).await;

        // Since sync is stopped, counter should not increase.
        assert_eq!(counter.0.load(Ordering::Relaxed), 3);
    }
}
