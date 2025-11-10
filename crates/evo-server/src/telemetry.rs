//! OpenTelemetry instrumentation.

use anyhow::Result;
use opentelemetry::{
    global,
    trace::TracerProvider as _,
};
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry(otel_endpoint: Option<&str>) -> Result<()> {
    let tracer_provider = if let Some(endpoint) = otel_endpoint {
        info!("Initializing OpenTelemetry with endpoint: {}", endpoint);

        // Create OTLP exporter
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .build_span_exporter()?;

        TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_sampler(Sampler::AlwaysOn)
            .with_id_generator(RandomIdGenerator::default())
            .with_resource(Resource::new(vec![
                opentelemetry::KeyValue::new(SERVICE_NAME, "evo-wasm-server"),
                opentelemetry::KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            ]))
            .build()
    } else {
        info!("OpenTelemetry disabled (no endpoint configured)");

        // No-op provider
        TracerProvider::builder()
            .with_sampler(Sampler::AlwaysOff)
            .build()
    };

    global::set_tracer_provider(tracer_provider.clone());

    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer_provider.tracer("evo-wasm-server"));

    // Set up tracing subscriber
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,evo_server=debug,evo_world=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(telemetry_layer)
        .init();

    info!("Telemetry initialized");
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
            counter.{} = $value,
            "Counter metric"
        );
    };
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {
        tracing::info!(
            counter.{} = $value,
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
            gauge.{} = $value,
            "Gauge metric"
        );
    };
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {
        tracing::info!(
            gauge.{} = $value,
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
            histogram.{} = $value,
            "Histogram metric"
        );
    };
    ($name:expr, $value:expr, $($key:expr => $val:expr),*) => {
        tracing::info!(
            histogram.{} = $value,
            $($key = $val,)*
            "Histogram metric"
        );
    };
}
