use crate::Result;
use prometheus::{
    core::{AtomicI64, AtomicU64, GenericCounter, GenericGauge},
    opts, IntCounterVec, IntGauge, IntGaugeVec, Registry,
};

pub const NAMESPACE: &str = "wohnzimmer";

/// Container for calendar metrics.
pub(crate) struct CalendarMetrics {
    events: IntGauge,
    latest_sync_seconds: IntGaugeVec,
    syncs_total: IntCounterVec,
}

impl CalendarMetrics {
    /// Creates new CalendarMetrics.
    pub fn new() -> Result<CalendarMetrics> {
        let events = IntGauge::with_opts(
            opts!("calendar_events", "Number of events in the calendar").namespace(NAMESPACE),
        )?;

        let latest_sync_seconds = IntGaugeVec::new(
            opts!(
                "calendar_latest_sync_seconds",
                "UNIX timestamp seconds of the latest successful calendar sync"
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
            latest_sync_seconds,
            syncs_total,
        })
    }

    /// Registers the metrics in a prometheus registry.
    pub fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.events.clone()))?;
        registry.register(Box::new(self.latest_sync_seconds.clone()))?;
        registry.register(Box::new(self.syncs_total.clone()))?;
        Ok(())
    }

    /// Provides access to the calendar events gauge.
    pub fn events(&self) -> GenericGauge<AtomicI64> {
        self.events.clone()
    }

    /// Provides access to the latest calendar sync UNIX timestamp gauge.
    pub fn latest_sync_seconds(&self, status: CalendarSyncStatus) -> GenericGauge<AtomicI64> {
        self.latest_sync_seconds
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
