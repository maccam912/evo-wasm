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
use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{self, format::Writer, FmtContext, FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Custom JSON formatter that includes OpenTelemetry trace_id and span_id
struct JsonWithTraceId;

impl<S, N> FormatEvent<S, N> for JsonWithTraceId
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        use opentelemetry::trace::TraceContextExt;

        let mut visit_buf = String::new();
        let mut visitor = serde_json::Map::new();

        // Extract event metadata
        let metadata = event.metadata();
        visitor.insert("timestamp".to_string(), serde_json::json!(chrono::Utc::now().to_rfc3339()));
        visitor.insert("level".to_string(), serde_json::json!(metadata.level().as_str()));
        visitor.insert("target".to_string(), serde_json::json!(metadata.target()));

        // Extract trace context from current span
        if let Some(span) = ctx.lookup_current() {
            // Get the OpenTelemetry context from the span
            let extensions = span.extensions();
            if let Some(otel_ctx) = extensions.get::<opentelemetry::Context>() {
                let span_ref = otel_ctx.span();
                let span_context = span_ref.span_context();
                if span_context.is_valid() {
                    visitor.insert("trace_id".to_string(), serde_json::json!(span_context.trace_id().to_string()));
                    visitor.insert("span_id".to_string(), serde_json::json!(span_context.span_id().to_string()));
                }
            }

            // Add span name
            visitor.insert("span".to_string(), serde_json::json!(span.name()));
        }

        // Extract thread info
        visitor.insert("thread_id".to_string(), serde_json::json!(format!("{:?}", std::thread::current().id())));
        visitor.insert("thread_name".to_string(), serde_json::json!(std::thread::current().name().unwrap_or("unknown")));

        // Extract event fields
        {
            let mut field_visitor = JsonVisitor(&mut visit_buf);
            event.record(&mut field_visitor);
        } // Drop field_visitor here

        if !visit_buf.is_empty() {
            if let Ok(fields) = serde_json::from_str::<serde_json::Value>(&visit_buf) {
                if let serde_json::Value::Object(field_map) = fields {
                    for (key, value) in field_map {
                        visitor.insert(key, value);
                    }
                }
            }
        }

        // Write the JSON output
        let json = serde_json::to_string(&visitor).map_err(|_| std::fmt::Error)?;
        writeln!(writer, "{}", json)?;

        Ok(())
    }
}

/// Visitor for collecting event fields into JSON
struct JsonVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // Simple field collection - in production you'd want proper JSON serialization
        if self.0.is_empty() {
            *self.0 = format!("{{\"{}\": \"{:?}\"", field.name(), value);
        } else {
            use std::fmt::Write;
            let _ = write!(self.0, ", \"{}\": \"{:?}\"", field.name(), value);
        }
    }
}

impl<'a> Drop for JsonVisitor<'a> {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            self.0.push('}');
        }
    }
}

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

        // Stdout formatter for local debugging (JSON format with trace context)
        let fmt_layer = fmt::layer()
            .event_format(JsonWithTraceId);

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
            .event_format(JsonWithTraceId);

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
