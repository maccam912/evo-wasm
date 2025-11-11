//! OpenTelemetry instrumentation following best practices.
//!
//! This module sets up comprehensive observability with:
//! - Distributed tracing via OTLP
//! - Structured logging via OTLP with automatic trace correlation
//! - W3C Trace Context propagation for distributed traces

use anyhow::Result;
use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    logs, propagation::TraceContextPropagator, runtime::Tokio, trace, Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_telemetry(otel_endpoint: Option<&str>) -> Result<()> {
    // Set global text map propagator for W3C Trace Context
    // This enables distributed tracing across service boundaries
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Check for standard OTEL environment variable, fallback to custom parameter
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .or_else(|| otel_endpoint.map(|s| s.to_string()));

    if let Some(endpoint) = endpoint {
        info!("Initializing OpenTelemetry with OTLP endpoint: {}", endpoint);

        // Shared resource configuration for all telemetry signals
        let resource = Resource::new(vec![
            KeyValue::new(
                SERVICE_NAME,
                std::env::var("OTEL_SERVICE_NAME")
                    .unwrap_or_else(|_| "evo-wasm-server".to_string()),
            ),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        ]);

        // ==================== TRACES ====================
        // Configure trace exporter and provider
        let trace_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()?;

        // Configure batch span processor with aggressive settings for better trace visibility
        let batch_config = trace::BatchConfigBuilder::default()
            .with_max_queue_size(2048)
            .with_max_export_batch_size(512)
            .with_scheduled_delay(std::time::Duration::from_secs(1))
            .with_max_export_timeout(std::time::Duration::from_secs(30))
            .build();

        let batch_processor = trace::BatchSpanProcessor::builder(trace_exporter, Tokio)
            .with_batch_config(batch_config)
            .build();

        let tracer_provider = trace::TracerProvider::builder()
            .with_resource(resource.clone())
            .with_sampler(trace::Sampler::AlwaysOn)
            .with_span_processor(batch_processor)
            .build();

        // Create tracer from the provider
        let tracer = tracer_provider.tracer("evo-wasm-server");

        // Set the global tracer provider
        global::set_tracer_provider(tracer_provider);

        // ==================== LOGS ====================
        // Configure log exporter and provider
        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()?;

        let logger_provider = logs::LoggerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(log_exporter, Tokio)
            .build();

        // Bridge tracing events to OpenTelemetry logs
        // This automatically attaches trace_id and span_id to logs
        let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

        // ==================== SUBSCRIBER ====================
        // Set up the tracing subscriber with all layers

        // Environment filter for log levels
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| {
                // Default: info level, debug for our crates
                "info,evo_server=debug,evo_world=debug,evo_worker=debug".into()
            });

        // Stdout formatter for local debugging (JSON format)
        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .json();

        // OpenTelemetry trace layer
        let otel_trace_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer);

        // Combine all layers
        tracing_subscriber::registry()
            .with(env_filter)         // Apply filter to all layers
            .with(fmt_layer)          // JSON logs to stdout
            .with(otel_trace_layer)   // Traces to OTLP
            .with(otel_log_layer)     // Logs to OTLP with trace context
            .init();

        info!("✓ OpenTelemetry initialized: traces and logs exporting to {}", endpoint);
    } else {
        info!("OpenTelemetry disabled (no endpoint configured)");

        // Minimal setup without OTLP export
        let tracer_provider = trace::TracerProvider::builder()
            .with_sampler(trace::Sampler::AlwaysOff)
            .build();

        let tracer = tracer_provider.tracer("evo-wasm-server");
        global::set_tracer_provider(tracer_provider);

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .json();

        tracing_subscriber::registry()
            .with(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,evo_server=debug,evo_world=debug".into()),
            )
            .with(fmt_layer)
            .with(telemetry_layer)
            .init();
    }

    Ok(())
}

pub fn shutdown_telemetry() {
    info!("Shutting down telemetry");
    global::shutdown_tracer_provider();
}

// Instrumentation macros for common operations

/// Record a counter metric
#[macro_export]
macro_rules! record_counter {
    ($name:expr, $value:expr) => {
        tracing::info!(
            counter_name = $name,
            counter_value = $value,
            "Counter metric"
        );
    };
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {
        tracing::info!(
            counter_name = $name,
            counter_value = $value,
            $($key = $val,)*
            "Counter metric"
        );
    };
}

/// Record a gauge metric
#[macro_export]
macro_rules! record_gauge {
    ($name:expr, $value:expr) => {
        tracing::info!(
            gauge_name = $name,
            gauge_value = $value,
            "Gauge metric"
        );
    };
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {
        tracing::info!(
            gauge_name = $name,
            gauge_value = $value,
            $($key = $val,)*
            "Gauge metric"
        );
    };
}

/// Record a histogram metric
#[macro_export]
macro_rules! record_histogram {
    ($name:expr, $value:expr) => {
        tracing::info!(
            histogram_name = $name,
            histogram_value = $value,
            "Histogram metric"
        );
    };
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {
        tracing::info!(
            histogram_name = $name,
            histogram_value = $value,
            $($key = $val,)*
            "Histogram metric"
        );
    };
}
