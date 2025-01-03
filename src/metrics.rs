use crate::Result;
use prometheus::{
    core::{AtomicI64, AtomicU64, GenericCounter, GenericGauge},
    opts, IntCounterVec, IntGaugeVec, Registry,
};

/// Container type for all custom application metrics.
#[derive(Clone, Debug)]
pub struct Metrics {
    calendar_events: IntGaugeVec,
    calendar_syncs: IntCounterVec,
}

impl Metrics {
    /// Creates metrics using the given namespace and registers them to the prometheus registry.
    pub fn new(namespace: &str, registry: &Registry) -> Result<Metrics> {
        let calendar_events = IntGaugeVec::new(
            opts!("calendar_events", "Number of events in the calendar").namespace(namespace),
            &[],
        )?;

        let calendar_syncs = IntCounterVec::new(
            opts!(
                "calendar_syncs_total",
                "Total number of calendar syncs performed"
            )
            .namespace(namespace),
            &["status"],
        )?;

        registry.register(Box::new(calendar_events.clone()))?;
        registry.register(Box::new(calendar_syncs.clone()))?;

        Ok(Metrics {
            calendar_events,
            calendar_syncs,
        })
    }

    pub fn calendar_events(&self) -> GenericGauge<AtomicI64> {
        self.calendar_events.with_label_values(&[])
    }

    pub fn calendar_syncs(&self, result: &str) -> GenericCounter<AtomicU64> {
        self.calendar_syncs.with_label_values(&[result])
    }
}
