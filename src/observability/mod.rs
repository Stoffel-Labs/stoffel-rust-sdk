//! Observability and metrics (RFC-010)
//!
//! This module provides lightweight, lock-free metrics primitives and health
//! status types for monitoring Stoffel MPC servers. All metric types use
//! `std::sync::atomic` operations and are safe to share across threads.
//!
//! # Metric Types
//!
//! - [`Counter`] — monotonically increasing value (e.g., total requests)
//! - [`Gauge`] — value that can go up and down (e.g., active connections)
//! - [`Histogram`] — distribution of observed values with fixed buckets
//!
//! # Aggregated Metrics
//!
//! - [`ServerMetrics`] — pre-defined collection of metrics for an MPC server
//!
//! # Health
//!
//! - [`HealthStatus`] — tri-state health indicator (Healthy / Degraded / Unhealthy)

pub mod otel;

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Counter
// ---------------------------------------------------------------------------

/// A monotonically increasing counter.
///
/// Counters are useful for tracking totals that only go up, such as the number
/// of completed computations or connection attempts.
///
/// # Thread Safety
///
/// All operations use `Relaxed` ordering, which is sufficient for counters
/// that are read for monitoring rather than synchronisation.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::observability::Counter;
///
/// let requests = Counter::new("http_requests_total");
/// requests.inc();
/// requests.inc_by(5);
/// assert_eq!(requests.get(), 6);
/// ```
pub struct Counter {
    value: AtomicU64,
    name: String,
}

impl Counter {
    /// Create a new counter with the given name, starting at zero.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            value: AtomicU64::new(0),
            name: name.into(),
        }
    }

    /// Increment the counter by one.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the counter by an arbitrary amount.
    pub fn inc_by(&self, amount: u64) {
        self.value.fetch_add(amount, Ordering::Relaxed);
    }

    /// Read the current value.
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Return the metric name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Gauge
// ---------------------------------------------------------------------------

/// A gauge that can increase or decrease.
///
/// Gauges represent a snapshot value, such as the number of currently
/// connected peers or active computations.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::observability::Gauge;
///
/// let active = Gauge::new("active_connections");
/// active.inc();
/// active.inc();
/// active.dec();
/// assert_eq!(active.get(), 1);
/// active.set(42);
/// assert_eq!(active.get(), 42);
/// ```
pub struct Gauge {
    value: AtomicUsize,
    name: String,
}

impl Gauge {
    /// Create a new gauge with the given name, starting at zero.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            value: AtomicUsize::new(0),
            name: name.into(),
        }
    }

    /// Set the gauge to an absolute value.
    pub fn set(&self, value: usize) {
        self.value.store(value, Ordering::Relaxed);
    }

    /// Read the current value.
    pub fn get(&self) -> usize {
        self.value.load(Ordering::Relaxed)
    }

    /// Increment the gauge by one.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the gauge by one.
    ///
    /// Uses saturating subtraction — the gauge will not wrap below zero.
    pub fn dec(&self) {
        // fetch_sub with Relaxed can wrap; use a CAS loop to saturate at 0.
        loop {
            let current = self.value.load(Ordering::Relaxed);
            if current == 0 {
                return;
            }
            if self
                .value
                .compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
    }

    /// Return the metric name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Histogram
// ---------------------------------------------------------------------------

/// Default bucket boundaries (in milliseconds or generic units).
const DEFAULT_BUCKETS: &[f64] = &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0];

/// A histogram that tracks the distribution of observed values.
///
/// Values are sorted into fixed buckets. Each bucket counts the number of
/// observations whose value is **less than or equal to** its upper bound.
/// The histogram also tracks the total count and sum of all observations.
///
/// Floating-point sums are stored as raw `u64` bits inside an [`AtomicU64`]
/// to remain lock-free and thread-safe.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::observability::Histogram;
///
/// let latency = Histogram::with_default_buckets("request_latency_ms");
/// latency.observe(12.5);
/// latency.observe(3.0);
/// assert_eq!(latency.count(), 2);
/// ```
pub struct Histogram {
    buckets: Vec<(f64, AtomicU64)>,
    sum: AtomicU64,
    count: AtomicU64,
    name: String,
}

impl Histogram {
    /// Create a histogram with custom bucket boundaries.
    ///
    /// Boundaries should be provided in ascending order. Each boundary
    /// represents the inclusive upper bound of a bucket.
    pub fn new(name: impl Into<String>, bucket_bounds: &[f64]) -> Self {
        let buckets = bucket_bounds
            .iter()
            .map(|&b| (b, AtomicU64::new(0)))
            .collect();
        Self {
            buckets,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
            name: name.into(),
        }
    }

    /// Create a histogram with the default bucket boundaries.
    ///
    /// Default buckets: `[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]`
    pub fn with_default_buckets(name: impl Into<String>) -> Self {
        Self::new(name, DEFAULT_BUCKETS)
    }

    /// Record an observed value.
    ///
    /// The value is added to every bucket whose upper bound is >= `value`,
    /// the running sum is updated, and the total count is incremented.
    pub fn observe(&self, value: f64) {
        // Increment matching buckets (cumulative: every bucket with bound >= value).
        for (bound, counter) in &self.buckets {
            if value <= *bound {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Atomically add `value` to sum using a CAS loop on the raw f64 bits.
        loop {
            let current_bits = self.sum.load(Ordering::Relaxed);
            let current = f64::from_bits(current_bits);
            let new = current + value;
            if self
                .sum
                .compare_exchange_weak(
                    current_bits,
                    new.to_bits(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the total number of observations.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Return the sum of all observed values.
    pub fn sum(&self) -> f64 {
        f64::from_bits(self.sum.load(Ordering::Relaxed))
    }

    /// Return the metric name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the bucket boundaries and their cumulative counts.
    pub fn buckets(&self) -> Vec<(f64, u64)> {
        self.buckets
            .iter()
            .map(|(bound, count)| (*bound, count.load(Ordering::Relaxed)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// HealthStatus
// ---------------------------------------------------------------------------

/// Health status of a component.
///
/// Used by health-check endpoints and the coordinator to determine whether a
/// server should receive work.
///
/// - [`Healthy`](HealthStatus::Healthy) — fully operational
/// - [`Degraded`](HealthStatus::Degraded) — operational but with issues
/// - [`Unhealthy`](HealthStatus::Unhealthy) — unable to serve requests
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is fully operational.
    Healthy,
    /// Component is operational but experiencing issues.
    Degraded {
        /// Human-readable reason for degradation.
        reason: String,
    },
    /// Component is unable to serve requests.
    Unhealthy {
        /// Human-readable reason for being unhealthy.
        reason: String,
    },
}

impl HealthStatus {
    /// Returns `true` only when the status is [`Healthy`](HealthStatus::Healthy).
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Returns `true` when the component can still serve requests
    /// (either [`Healthy`](HealthStatus::Healthy) or [`Degraded`](HealthStatus::Degraded)).
    pub fn is_operational(&self) -> bool {
        !matches!(self, HealthStatus::Unhealthy { .. })
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded { reason } => write!(f, "Degraded: {}", reason),
            HealthStatus::Unhealthy { reason } => write!(f, "Unhealthy: {}", reason),
        }
    }
}

// ---------------------------------------------------------------------------
// ServerMetrics
// ---------------------------------------------------------------------------

/// Pre-defined collection of metrics for an MPC server.
///
/// Provides counters, gauges, and histograms that cover the most common
/// monitoring needs for a Stoffel MPC server.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::observability::ServerMetrics;
///
/// let metrics = ServerMetrics::new();
/// metrics.connected_peers.inc();
/// metrics.total_connections.inc();
/// metrics.computation_latency_ms.observe(42.0);
/// assert_eq!(metrics.connected_peers.get(), 1);
/// ```
pub struct ServerMetrics {
    // -- Connection metrics -------------------------------------------------
    /// Number of currently connected MPC peers.
    pub connected_peers: Gauge,
    /// Number of currently connected clients.
    pub connected_clients: Gauge,
    /// Total connections accepted since server start.
    pub total_connections: Counter,
    /// Total connection errors since server start.
    pub connection_errors: Counter,

    // -- Computation metrics ------------------------------------------------
    /// Total computations that completed successfully.
    pub computations_completed: Counter,
    /// Total computations that failed.
    pub computations_failed: Counter,
    /// Number of computations currently in progress.
    pub active_computations: Gauge,

    // -- Preprocessing metrics ----------------------------------------------
    /// Remaining Beaver triples available for multiplication gates.
    pub preprocessing_triples_remaining: Gauge,
    /// Remaining random shares available.
    pub preprocessing_random_shares_remaining: Gauge,

    // -- Latency metrics ----------------------------------------------------
    /// End-to-end computation latency in milliseconds.
    pub computation_latency_ms: Histogram,
    /// Consensus round latency in milliseconds.
    pub consensus_latency_ms: Histogram,
}

impl ServerMetrics {
    /// Create a new `ServerMetrics` with all counters/gauges at zero.
    pub fn new() -> Self {
        Self {
            connected_peers: Gauge::new("stoffel_connected_peers"),
            connected_clients: Gauge::new("stoffel_connected_clients"),
            total_connections: Counter::new("stoffel_total_connections"),
            connection_errors: Counter::new("stoffel_connection_errors"),

            computations_completed: Counter::new("stoffel_computations_completed"),
            computations_failed: Counter::new("stoffel_computations_failed"),
            active_computations: Gauge::new("stoffel_active_computations"),

            preprocessing_triples_remaining: Gauge::new("stoffel_preprocessing_triples_remaining"),
            preprocessing_random_shares_remaining: Gauge::new(
                "stoffel_preprocessing_random_shares_remaining",
            ),

            computation_latency_ms: Histogram::with_default_buckets(
                "stoffel_computation_latency_ms",
            ),
            consensus_latency_ms: Histogram::with_default_buckets(
                "stoffel_consensus_latency_ms",
            ),
        }
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Counter tests ------------------------------------------------------

    #[test]
    fn counter_starts_at_zero() {
        let c = Counter::new("test");
        assert_eq!(c.get(), 0);
    }

    #[test]
    fn counter_inc() {
        let c = Counter::new("test");
        c.inc();
        c.inc();
        assert_eq!(c.get(), 2);
    }

    #[test]
    fn counter_inc_by() {
        let c = Counter::new("test");
        c.inc_by(10);
        c.inc();
        c.inc_by(5);
        assert_eq!(c.get(), 16);
    }

    #[test]
    fn counter_name() {
        let c = Counter::new("my_counter");
        assert_eq!(c.name(), "my_counter");
    }

    // -- Gauge tests --------------------------------------------------------

    #[test]
    fn gauge_starts_at_zero() {
        let g = Gauge::new("test");
        assert_eq!(g.get(), 0);
    }

    #[test]
    fn gauge_set_and_get() {
        let g = Gauge::new("test");
        g.set(42);
        assert_eq!(g.get(), 42);
    }

    #[test]
    fn gauge_inc_dec() {
        let g = Gauge::new("test");
        g.inc();
        g.inc();
        g.inc();
        g.dec();
        assert_eq!(g.get(), 2);
    }

    #[test]
    fn gauge_dec_saturates_at_zero() {
        let g = Gauge::new("test");
        g.inc();
        g.dec();
        g.dec(); // should not wrap
        assert_eq!(g.get(), 0);
    }

    #[test]
    fn gauge_name() {
        let g = Gauge::new("my_gauge");
        assert_eq!(g.name(), "my_gauge");
    }

    // -- Histogram tests ----------------------------------------------------

    #[test]
    fn histogram_starts_empty() {
        let h = Histogram::with_default_buckets("test");
        assert_eq!(h.count(), 0);
        assert_eq!(h.sum(), 0.0);
    }

    #[test]
    fn histogram_observe_updates_count_and_sum() {
        let h = Histogram::with_default_buckets("test");
        h.observe(10.0);
        h.observe(20.0);
        assert_eq!(h.count(), 2);
        assert!((h.sum() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_bucket_counting() {
        let h = Histogram::new("test", &[10.0, 50.0, 100.0]);
        h.observe(5.0); // <= 10, 50, 100
        h.observe(30.0); // <= 50, 100
        h.observe(75.0); // <= 100
        h.observe(200.0); // none

        let buckets = h.buckets();
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0], (10.0, 1)); // only 5.0
        assert_eq!(buckets[1], (50.0, 2)); // 5.0 and 30.0
        assert_eq!(buckets[2], (100.0, 3)); // 5.0, 30.0, and 75.0
    }

    #[test]
    fn histogram_default_buckets() {
        let h = Histogram::with_default_buckets("test");
        assert_eq!(h.buckets().len(), DEFAULT_BUCKETS.len());
    }

    #[test]
    fn histogram_name() {
        let h = Histogram::with_default_buckets("my_histogram");
        assert_eq!(h.name(), "my_histogram");
    }

    // -- HealthStatus tests -------------------------------------------------

    #[test]
    fn health_status_healthy() {
        let status = HealthStatus::Healthy;
        assert!(status.is_healthy());
        assert!(status.is_operational());
    }

    #[test]
    fn health_status_degraded() {
        let status = HealthStatus::Degraded {
            reason: "high latency".to_string(),
        };
        assert!(!status.is_healthy());
        assert!(status.is_operational());
    }

    #[test]
    fn health_status_unhealthy() {
        let status = HealthStatus::Unhealthy {
            reason: "no peers".to_string(),
        };
        assert!(!status.is_healthy());
        assert!(!status.is_operational());
    }

    #[test]
    fn health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(
            HealthStatus::Degraded {
                reason: "slow".into()
            }
            .to_string(),
            "Degraded: slow"
        );
        assert_eq!(
            HealthStatus::Unhealthy {
                reason: "down".into()
            }
            .to_string(),
            "Unhealthy: down"
        );
    }

    // -- ServerMetrics tests ------------------------------------------------

    #[test]
    fn server_metrics_default() {
        let m = ServerMetrics::default();
        assert_eq!(m.connected_peers.get(), 0);
        assert_eq!(m.total_connections.get(), 0);
        assert_eq!(m.computation_latency_ms.count(), 0);
    }

    #[test]
    fn server_metrics_usage() {
        let m = ServerMetrics::new();
        m.connected_peers.inc();
        m.connected_peers.inc();
        m.total_connections.inc();
        m.computations_completed.inc();
        m.computation_latency_ms.observe(42.0);

        assert_eq!(m.connected_peers.get(), 2);
        assert_eq!(m.total_connections.get(), 1);
        assert_eq!(m.computations_completed.get(), 1);
        assert_eq!(m.computation_latency_ms.count(), 1);
    }
}
