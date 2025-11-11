//! OpenTelemetry instrumentation for worker.

use anyhow::Result;
use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    logs, propagation::TraceContextPropagator, runtime::Tokio, trace, Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry(otel_endpoint: Option<&str>) -> Result<()> {
    // Set global text map propagator for W3C Trace Context
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Check for standard OTEL environment variable, fallback to custom parameter
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .or_else(|| otel_endpoint.map(|s| s.to_string()));

    if let Some(endpoint) = endpoint {
        // Shared resource: shows up on both traces and logs
        let resource = Resource::new(vec![
            KeyValue::new(
                SERVICE_NAME,
                std::env::var("OTEL_SERVICE_NAME")
                    .unwrap_or_else(|_| "evo-wasm-worker".to_string()),
            ),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        ]);

        // ---- Configure traces ----
        let tracer_provider = trace::TracerProvider::builder()
            .with_resource(resource.clone())
            .with_sampler(trace::Sampler::AlwaysOn)
            .with_batch_exporter(
                opentelemetry_otlp::SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(&endpoint)
                    .build()?,
                Tokio,
            )
            .build();

        let tracer = tracer_provider.tracer("evo-wasm-worker");
        global::set_tracer_provider(tracer_provider);

        let trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        // ---- Configure logs ----
        let logger_provider = logs::LoggerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(
                opentelemetry_otlp::LogExporter::builder()
                    .with_tonic()
                    .with_endpoint(&endpoint)
                    .build()?,
                Tokio,
            )
            .build();

        // Bridge tracing events -> OTEL LogRecords (+ attach TraceId/SpanId)
        let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

        // ---- Set up tracing-subscriber registry ----
        // Use JSON formatting for human-readable stdout logs
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .json()
            .flatten_event(true);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,evo_worker=debug,evo_world=debug".into()),
            )
            .with(fmt_layer)       // stdout logs (JSON)
            .with(trace_layer)     // export spans to OTLP
            .with(otel_log_layer)  // export logs to OTLP with trace context
            .init();

        info!("Telemetry initialized with OTLP endpoint: {}", endpoint);
    } else {
        // Create no-op tracer provider
        let tracer_provider = trace::TracerProvider::builder()
            .with_sampler(trace::Sampler::AlwaysOff)
            .build();

        let tracer = tracer_provider.tracer("evo-wasm-worker");
        global::set_tracer_provider(tracer_provider);

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        // Just use JSON formatting to stdout
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .json()
            .flatten_event(true);

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,evo_worker=debug,evo_world=debug".into()),
            )
            .with(fmt_layer)
            .with(telemetry_layer)
            .init();

        info!("OpenTelemetry disabled (no endpoint configured)");
    }

    Ok(())
}

pub fn shutdown_telemetry() {
    info!("Shutting down telemetry");
    global::shutdown_tracer_provider();
}
