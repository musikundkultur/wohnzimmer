use crate::Result;
use prometheus::{
    core::{AtomicI64, AtomicU64, GenericCounter, GenericGauge},
    opts, IntCounterVec, IntGauge, Registry,
};

pub const NAMESPACE: &str = "wohnzimmer";

/// Container for calendar metrics.
pub(crate) struct CalendarMetrics {
    calendar_events: IntGauge,
    calendar_syncs: IntCounterVec,
}

impl CalendarMetrics {
    /// Creates new CalendarMetrics.
    pub fn new() -> Result<CalendarMetrics> {
        let calendar_events = IntGauge::with_opts(
            opts!("calendar_events", "Number of events in the calendar").namespace(NAMESPACE),
        )?;

        let calendar_syncs = IntCounterVec::new(
            opts!(
                "calendar_syncs_total",
                "Total number of calendar syncs performed"
            )
            .namespace(NAMESPACE),
            &["status"],
        )?;

        Ok(CalendarMetrics {
            calendar_events,
            calendar_syncs,
        })
    }

    /// Registers the metrics in a prometheus registry.
    pub fn register(&self, registry: &Registry) -> Result<()> {
        registry.register(Box::new(self.calendar_events.clone()))?;
        registry.register(Box::new(self.calendar_syncs.clone()))?;
        Ok(())
    }

    /// Provides access to the calendar events gauge.
    pub fn calendar_events(&self) -> GenericGauge<AtomicI64> {
        self.calendar_events.clone()
    }

    /// Provides access to the calendar syncs counter.
    pub fn calendar_syncs(&self, status: &str) -> GenericCounter<AtomicU64> {
        self.calendar_syncs.with_label_values(&[status])
    }
}
