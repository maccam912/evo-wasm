//! OpenTelemetry instrumentation for worker.

use anyhow::Result;
use opentelemetry::{
    global,
    trace::TracerProvider as _,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry(otel_endpoint: Option<&str>) -> Result<()> {
    // Check for standard OTEL environment variable, fallback to custom parameter
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .or_else(|| otel_endpoint.map(|s| s.to_string()));

    let tracer = if let Some(endpoint) = endpoint {
        info!("Initializing OpenTelemetry with OTLP endpoint: {}", endpoint);

        // Configure OTLP exporter
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(&endpoint);

        // Build and install OTLP pipeline (sets global tracer provider and returns tracer)
        opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(
                opentelemetry_sdk::trace::Config::default()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(Resource::new(vec![
                        opentelemetry::KeyValue::new(SERVICE_NAME,
                            std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "evo-wasm-worker".to_string())
                        ),
                        opentelemetry::KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                    ]))
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)?
    } else {
        info!("OpenTelemetry disabled (no endpoint configured)");

        // Create no-op tracer provider and get tracer
        let tracer_provider = TracerProvider::builder()
            .with_config(
                opentelemetry_sdk::trace::Config::default()
                    .with_sampler(Sampler::AlwaysOff)
            )
            .build();

        global::set_tracer_provider(tracer_provider.clone());
        tracer_provider.tracer("evo-wasm-worker")
    };

    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer);

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
