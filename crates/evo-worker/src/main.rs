//! Worker client for executing island simulations.

mod telemetry;
mod client;

use anyhow::Result;
use evo_core::WorkerConfig;
use std::sync::Arc;
use tokio::signal;
use tokio::time::{interval, Duration};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = WorkerConfig::default();

    // Initialize telemetry
    telemetry::init_telemetry(config.otel_endpoint.as_deref())?;

    info!("Starting Evo-WASM worker");
    info!("Server URL: {}", config.server_url);

    // Create worker client
    let client = Arc::new(client::WorkerClient::new(config.clone())?);

    // Start worker tasks
    let mut handles = vec![];

    for i in 0..config.max_concurrent_jobs {
        let client = client.clone();
        let handle = tokio::spawn(async move {
            info!("Worker task {} started", i);
            if let Err(e) = run_worker_loop(client, i).await {
                error!("Worker task {} failed: {}", i, e);
            }
        });
        handles.push(handle);
    }

    // Wait for shutdown signal
    shutdown_signal().await;

    info!("Shutting down worker");

    // Wait for tasks to complete (with timeout)
    let timeout = tokio::time::sleep(Duration::from_secs(30));
    tokio::pin!(timeout);

    tokio::select! {
        _ = futures::future::join_all(handles) => {
            info!("All worker tasks completed");
        }
        _ = &mut timeout => {
            info!("Shutdown timeout reached");
        }
    }

    // Shutdown telemetry
    telemetry::shutdown_telemetry();

    Ok(())
}

async fn run_worker_loop(client: Arc<client::WorkerClient>, task_id: usize) -> Result<()> {
    let poll_interval = client.config().poll_interval_ms;
    let mut interval = interval(Duration::from_millis(poll_interval));

    loop {
        interval.tick().await;

        match client.request_and_execute_job(task_id).await {
            Ok(Some(())) => {
                info!("Task {} completed a job", task_id);
            }
            Ok(None) => {
                // No job available, continue polling
            }
            Err(e) => {
                error!("Task {} encountered error: {}", task_id, e);
                // Wait a bit longer on error to avoid hammering the server
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

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
