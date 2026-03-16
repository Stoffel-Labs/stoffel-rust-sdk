//! OpenTelemetry configuration placeholder (RFC-010).
//!
//! This module defines the configuration structure for future OpenTelemetry
//! integration. No actual OTel SDK dependency is required — this is a
//! configuration-only placeholder that downstream code can populate and pass
//! to an eventual exporter.

use std::collections::HashMap;
use std::time::Duration;

/// Configuration for OpenTelemetry metric and trace export.
///
/// This is a placeholder struct that captures the information needed to
/// initialise an OTel exporter. Actual export will be implemented once
/// the `opentelemetry` crate is added as an optional dependency.
///
/// # Examples
///
/// ```
/// use stoffel_rust_sdk::observability::otel::OtelConfig;
///
/// let config = OtelConfig::new("stoffel-mpc-server")
///     .with_metrics_endpoint("http://localhost:4317")
///     .with_traces_endpoint("http://localhost:4317");
///
/// assert_eq!(config.service_name, "stoffel-mpc-server");
/// assert_eq!(config.metrics_endpoint.as_deref(), Some("http://localhost:4317"));
/// ```
#[derive(Clone, Debug)]
pub struct OtelConfig {
    /// OTLP endpoint for metrics export (e.g., `http://localhost:4317`).
    pub metrics_endpoint: Option<String>,
    /// OTLP endpoint for traces export (e.g., `http://localhost:4317`).
    pub traces_endpoint: Option<String>,
    /// The `service.name` resource attribute.
    pub service_name: String,
    /// Additional resource attributes attached to every exported signal.
    pub resource_attributes: HashMap<String, String>,
    /// Interval between metric export batches.
    pub export_interval: Duration,
}

impl OtelConfig {
    /// Create a new configuration with the given service name and sensible
    /// defaults.
    ///
    /// Defaults:
    /// - No endpoints configured (export disabled)
    /// - 60-second export interval
    /// - No additional resource attributes
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            metrics_endpoint: None,
            traces_endpoint: None,
            service_name: service_name.into(),
            resource_attributes: HashMap::new(),
            export_interval: Duration::from_secs(60),
        }
    }

    /// Set the OTLP metrics endpoint.
    pub fn with_metrics_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.metrics_endpoint = Some(endpoint.into());
        self
    }

    /// Set the OTLP traces endpoint.
    pub fn with_traces_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.traces_endpoint = Some(endpoint.into());
        self
    }

    /// Add a resource attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.resource_attributes.insert(key.into(), value.into());
        self
    }

    /// Set the export interval.
    pub fn with_export_interval(mut self, interval: Duration) -> Self {
        self.export_interval = interval;
        self
    }
}
