//! OpenTelemetry instrumentation for worker.

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

        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .build_span_exporter()?;

        TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_sampler(Sampler::AlwaysOn)
            .with_id_generator(RandomIdGenerator::default())
            .with_resource(Resource::new(vec![
                opentelemetry::KeyValue::new(SERVICE_NAME, "evo-wasm-worker"),
                opentelemetry::KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            ]))
            .build()
    } else {
        info!("OpenTelemetry disabled (no endpoint configured)");

        TracerProvider::builder()
            .with_sampler(Sampler::AlwaysOff)
            .build()
    };

    global::set_tracer_provider(tracer_provider.clone());

    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer_provider.tracer("evo-wasm-worker"));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,evo_worker=debug,evo_world=debug".into()),
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
