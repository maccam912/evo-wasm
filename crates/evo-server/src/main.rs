//! Central coordination server for Evo-WASM.

mod api;
mod checkpoint;
mod database;
mod evolution;
mod job_manager;
mod telemetry;

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use evo_core::ServerConfig;
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, instrument};
use axum::middleware::{self, Next};
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = ServerConfig::default();

    // Initialize telemetry
    telemetry::init_telemetry(config.otel_endpoint.as_deref())?;

    info!("Starting Evo-WASM server on {}:{}", config.bind_address, config.port);

    // Initialize database
    let db = database::Database::new(&config.database_path).await?;
    db.migrate().await?;

    // Initialize job manager
    let job_manager = Arc::new(job_manager::JobManager::new());

    // Initialize evolution engine
    let evolution = Arc::new(evolution::EvolutionEngine::new(db.clone()));

    // Initialize checkpoint manager
    let checkpoint_mgr = Arc::new(checkpoint::CheckpointManager::new(
        config.checkpoint_dir.clone(),
        db.clone(),
    ));

    // Start checkpoint background task
    let checkpoint_mgr_clone = checkpoint_mgr.clone();
    let checkpoint_interval = config.checkpoint_interval_secs;
    tokio::spawn(async move {
        checkpoint_mgr_clone
            .start_periodic_checkpoints(checkpoint_interval)
            .await;
    });

    // Try to restore from latest checkpoint
    if let Err(e) = checkpoint_mgr.restore_latest().await {
        tracing::warn!("Failed to restore from checkpoint: {}", e);
    }

    // Build API router
    let app = Router::new()
        .route("/health", get(api::health))
        .route("/api/jobs/request", post(api::request_job))
        .route("/api/jobs/submit", post(|state, json| async move { api::submit_result(state, json).await }))
        .route("/api/stats", get(api::get_stats))
        .route("/api/config", get(api::get_config))
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn(extract_trace_context))
        .layer(TraceLayer::new_for_http())
        .with_state(api::AppState {
            job_manager,
            evolution,
            checkpoint_mgr,
            db,
        });

    // Start server
    let addr = format!("{}:{}", config.bind_address, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Shutdown telemetry
    telemetry::shutdown_telemetry();

    Ok(())
}

/// Middleware to extract trace context from incoming HTTP requests
async fn extract_trace_context(
    request: Request<Body>,
    next: Next,
) -> Response {
    // Extract trace context from headers
    let headers = request.headers();
    let mut header_map = std::collections::HashMap::new();

    for (key, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            header_map.insert(key.as_str().to_string(), value_str.to_string());
        }
    }

    // Extract context using the global propagator
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&header_map)
    });

    // Set the extracted context as the parent of the current span
    tracing::Span::current().set_parent(parent_context);

    next.run(request).await
}

#[instrument]
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received");
}
