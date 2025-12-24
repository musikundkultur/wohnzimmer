use crate::Result;
use prometheus::{
    Histogram, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Registry,
    core::{AtomicI64, AtomicU64, GenericCounter, GenericGauge},
    histogram_opts, opts,
};

pub const NAMESPACE: &str = "wohnzimmer";

/// Container for calendar metrics.
pub(crate) struct CalendarMetrics {
    events: IntGaugeVec,
    events_total: IntGauge,
    latest_sync_timestamp_seconds: IntGaugeVec,
    sync_duration_seconds: HistogramVec,
    syncs_total: IntCounterVec,
}

impl CalendarMetrics {
    /// Creates new CalendarMetrics.
    pub fn new() -> Result<CalendarMetrics> {
        let events = IntGaugeVec::new(
            opts!("calendar_events", "Number of events in the calendar").namespace(NAMESPACE),
            &["detail"],
        )?;

        let events_total = IntGauge::with_opts(
            opts!(
                "calendar_events_total",
                "Total number of events in the calendar"
            )
            .namespace(NAMESPACE),
        )?;

        let latest_sync_timestamp_seconds = IntGaugeVec::new(
            opts!(
                "calendar_latest_sync_timestamp_seconds",
                "UNIX timestamp seconds of the latest successful calendar sync"
            )
            .namespace(NAMESPACE),
            &["status"],
        )?;

        let sync_duration_seconds = HistogramVec::new(
            histogram_opts!(
                "calendar_sync_duration_seconds",
                "Calendar sync duration in seconds"
            )
            .namespace(NAMESPACE),
            &["status"],
        )?;

        let syncs_total = IntCounterVec::new(
            opts!(
                "calendar_syncs_total",
                "Total number of calendar syncs performed"
            )
            .namespace(NAMESPACE),
            &["status"],
        )?;

        Ok(CalendarMetrics {
            events,
            events_total,
            latest_sync_timestamp_seconds,
            sync_duration_seconds,
            syncs_total,
        })
    }

    /// Registers the metrics in a prometheus registry.
    pub fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.events.clone()))?;
        registry.register(Box::new(self.events_total.clone()))?;
        registry.register(Box::new(self.latest_sync_timestamp_seconds.clone()))?;
        registry.register(Box::new(self.sync_duration_seconds.clone()))?;
        registry.register(Box::new(self.syncs_total.clone()))?;
        Ok(())
    }

    /// Provides access to the calendar events gauge.
    pub fn events(&self, detail: EventDetail) -> GenericGauge<AtomicI64> {
        self.events.with_label_values(&[detail.as_str()])
    }

    /// Provides access to the total calendar events gauge.
    pub fn events_total(&self) -> GenericGauge<AtomicI64> {
        self.events_total.clone()
    }

    /// Provides access to the latest calendar sync UNIX timestamp gauge.
    pub fn latest_sync_timestamp_seconds(
        &self,
        status: CalendarSyncStatus,
    ) -> GenericGauge<AtomicI64> {
        self.latest_sync_timestamp_seconds
            .with_label_values(&[status.as_str()])
    }

    /// Provides access to the calendar sync duration seconds histogram.
    pub fn sync_duration_seconds(&self, status: CalendarSyncStatus) -> Histogram {
        self.sync_duration_seconds
            .with_label_values(&[status.as_str()])
    }

    /// Provides access to the calendar syncs counter.
    pub fn syncs_total(&self, status: CalendarSyncStatus) -> GenericCounter<AtomicU64> {
        self.syncs_total.with_label_values(&[status.as_str()])
    }
}

/// Status of a calendar sync operation.
#[derive(Debug, Copy, Clone)]
pub(crate) enum CalendarSyncStatus {
    /// Calendar sync was successful.
    Success,
    /// An error occurred while syncing the calendar.
    Error,
}

impl CalendarSyncStatus {
    /// Returns the status as a &str.
    pub fn as_str(&self) -> &str {
        match self {
            CalendarSyncStatus::Success => "success",
            CalendarSyncStatus::Error => "error",
        }
    }
}

/// Level of detail calendar events provide.
#[derive(Debug, Copy, Clone)]
pub(crate) enum EventDetail {
    /// Events with description.
    Desc,
    /// Simple events without description.
    Simple,
}

impl EventDetail {
    /// Returns the detail level as a &str.
    pub fn as_str(&self) -> &str {
        match self {
            EventDetail::Desc => "description",
            EventDetail::Simple => "simple",
        }
    }
}
