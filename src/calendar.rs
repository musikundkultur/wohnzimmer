pub mod google;
pub mod templating;

use super::Result;
use crate::CalendarConfig;
use crate::metrics::{CalendarMetrics, CalendarSyncStatus};
use async_trait::async_trait;
use google::GoogleCalendarClient;
use indexmap::IndexMap;
use jiff::{Timestamp, ToSpan, Zoned, tz::TimeZone};
use prometheus::Registry;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::ops::Range;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::oneshot::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::Duration;

/// Represents a single calendar event.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The start date of the event.
    pub start_date: Timestamp,
    /// The end date of the event, if any.
    pub end_date: Option<Timestamp>,
    /// The event title.
    pub title: String,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.title.fmt(f)
    }
}

/// Type alias for calendar events grouped by year.
pub type EventsByYear = IndexMap<i16, Vec<Event>>;

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
            start_date: ev.start.to_timestamp(),
            end_date: Some(ev.end.to_timestamp()),
            title: ev.summary,
        }
    }
}

#[async_trait]
impl EventSource for GoogleCalendarEventSource {
    async fn fetch_events(&self) -> Result<Vec<Event>> {
        let now = Zoned::now();
        let start = now.start_of_day().unwrap();
        let end = &start + 12.months();

        let events = self
            .client
            .get_events(Some(start.timestamp()..end.timestamp()), None, None)
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
    metrics: Arc<CalendarMetrics>,
}

impl Calendar {
    /// Creates a new `Calendar` from an event source.
    pub fn new<T>(event_source: T) -> Result<Calendar>
    where
        T: EventSource + 'static,
    {
        Ok(Calendar {
            event_source: Arc::new(event_source),
            events: Default::default(),
            metrics: Arc::new(CalendarMetrics::new()?),
        })
    }

    /// Creates a new `Calendar` from configuration.
    pub async fn from_config(config: &CalendarConfig) -> Result<Calendar> {
        let event_source: Box<dyn EventSource> = match config.event_source {
            EventSourceKind::Static => Box::new(StaticEventSource::new(config.events.clone())),
            EventSourceKind::GoogleCalendar => Box::new(GoogleCalendarEventSource::new().await?),
        };

        Calendar::new(event_source)
    }

    /// Registers the calendar metrics in a prometheus registry.
    pub fn register_metrics(&self, registry: &Registry) -> Result<()> {
        self.metrics.register(registry)
    }

    /// Filters events between a start date (inclusive) and an end date (exclusive).
    pub async fn get_events(&self, range: Range<Timestamp>) -> Result<Vec<Event>> {
        let events = self.events.lock().await.clone();

        let events = events
            .into_iter()
            .filter(|event| range.contains(&event.start_date))
            .collect();

        Ok(events)
    }

    /// Builds an index of event year to list of events. This is used to avoid having complicated
    /// logic for displaying events by year in HTML templates.
    pub async fn get_events_by_year(&self, range: Range<Timestamp>) -> Result<EventsByYear> {
        let events = self.get_events(range).await?;
        let mut events_by_year: EventsByYear = IndexMap::new();

        events.into_iter().for_each(|event| {
            let start_date = event.start_date.to_zoned(TimeZone::system());
            events_by_year
                .entry(start_date.year())
                .or_default()
                .push(event);
        });

        Ok(events_by_year)
    }

    /// Synchronize events from the source into the calendar once.
    pub async fn sync_once(&self) -> Result<()> {
        log::debug!("synchronizing calendar events");

        let (result, status) = match self.event_source.fetch_events().await {
            Ok(mut events) => {
                self.metrics.events().set(events.len() as i64);

                // Ensure events are always sorted by date.
                events.sort_by_key(|event| event.start_date);
                *self.events.lock().await = events;

                (Ok(()), CalendarSyncStatus::Success)
            }
            Err(err) => (Err(err), CalendarSyncStatus::Error),
        };

        let now = Timestamp::now().as_second();
        self.metrics.latest_sync_seconds(status).set(now);
        self.metrics.syncs_total(status).inc();

        result
    }

    /// Starts to periodically sync the calendar every `interval` until a message is received via
    /// `stop`.
    async fn start_sync(&self, period: Duration, mut stop: Receiver<()>) {
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
                    return;
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
            calendar.start_sync(period, stop_rx).await;
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
        if self.stop_tx.send(()).is_ok() {
            self.join_handle.await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;
    use jiff::{civil::datetime, tz::TimeZone};
    use std::sync::atomic::{AtomicUsize, Ordering};

    macro_rules! date {
        ($y:expr, $m:expr, $d:expr) => {
            datetime($y, $m, $d, 0, 0, 0, 0)
                .to_zoned(TimeZone::system())
                .unwrap()
                .timestamp()
        };
    }

    macro_rules! event {
        ($title:expr, $y:expr, $m:expr, $d:expr) => {
            Event {
                title: $title.into(),
                start_date: date!($y, $m, $d),
                end_date: None,
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
        ]))
        .unwrap();
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
        ]))
        .unwrap();
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
        use CalendarSyncStatus::*;

        // A fake `EventSource` which just counts invocations of `fetch_events` and returns a fake
        // event.
        struct Counter(AtomicUsize);

        #[async_trait]
        impl EventSource for Counter {
            async fn fetch_events(&self) -> Result<Vec<Event>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Event {
                    title: "event".into(),
                    start_date: date!(2023, 1, 1),
                    end_date: None,
                }])
            }
        }

        let counter = Arc::new(Counter(AtomicUsize::new(0)));

        let calendar = Arc::new(Calendar::new(counter.clone()).unwrap());

        let range1 = date!(2023, 1, 1)..date!(2023, 1, 2);
        let range2 = date!(2023, 1, 2)..date!(2023, 1, 3);

        // Initially, there are no events because no sync happened.
        assert_eq!(calendar.get_events(range1.clone()).await.unwrap(), vec![]);

        assert_eq!(calendar.metrics.events().get(), 0);
        assert_eq!(calendar.metrics.syncs_total(Success).get(), 0);
        assert_eq!(calendar.metrics.syncs_total(Error).get(), 0);

        calendar.sync_once().await.unwrap();

        assert_eq!(calendar.metrics.events().get(), 1);
        assert_eq!(calendar.metrics.syncs_total(Success).get(), 1);
        assert_eq!(calendar.metrics.syncs_total(Error).get(), 0);

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
        assert_eq!(calendar.metrics.syncs_total(Success).get(), 3);
        assert_eq!(calendar.metrics.syncs_total(Error).get(), 0);

        tokio::time::sleep(Duration::from_millis(15)).await;

        // Since sync is stopped, counter should not increase.
        assert_eq!(counter.0.load(Ordering::Relaxed), 3);
    }
}
