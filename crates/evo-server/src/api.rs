//! API handlers for the server.

use crate::{checkpoint::CheckpointManager, database::Database, evolution::EvolutionEngine, job_manager::JobManager};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use evo_core::JobConfig;
use evo_world::IslandResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone)]
pub struct AppState {
    pub job_manager: Arc<JobManager>,
    pub evolution: Arc<EvolutionEngine>,
    pub checkpoint_mgr: Arc<CheckpointManager>,
    pub db: Database,
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

/// Health check endpoint
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Deserialize)]
pub struct JobRequest {
    worker_id: Option<String>,
}

/// Request a new job
pub async fn request_job(
    State(state): State<AppState>,
    Json(req): Json<JobRequest>,
) -> Result<Json<evo_world::IslandJob>, ApiError> {
    info!("Job requested by worker: {:?}", req.worker_id);

    let job = state.job_manager.get_job(&state.evolution).await?;

    Ok(Json(job))
}

#[derive(Serialize)]
pub struct SubmitResponse {
    success: bool,
}

/// Submit job results
pub async fn submit_result(
    State(state): State<AppState>,
    Json(job_result): Json<IslandResult>,
) -> Response {
    info!("Job result submitted: {:?}", job_result.job_id);

    state.job_manager.mark_job_complete(job_result.job_id).await;

    match state.evolution.process_result(job_result).await {
        Ok(_) => (StatusCode::OK, Json(SubmitResponse { success: true })).into_response(),
        Err(e) => {
            error!("Failed to process result: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to process result: {}", e)).into_response()
        }
    }
}

#[derive(Serialize)]
pub struct StatsResponse {
    total_jobs: usize,
    pending_jobs: usize,
    completed_jobs: usize,
    total_lineages: usize,
}

/// Get server statistics
pub async fn get_stats(State(state): State<AppState>) -> Result<Json<StatsResponse>, ApiError> {
    let stats = state.job_manager.get_stats().await;
    let total_lineages = state.db.count_lineages().await?;

    Ok(Json(StatsResponse {
        total_jobs: stats.total_jobs,
        pending_jobs: stats.pending_jobs,
        completed_jobs: stats.completed_jobs,
        total_lineages,
    }))
}

/// Get current job configuration
pub async fn get_config(State(state): State<AppState>) -> Json<JobConfig> {
    Json(state.evolution.get_config().await)
}

// Error handling
pub enum ApiError {
    Internal(String),
    NotFound(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        (status, message).into_response()
    }
}

impl From<evo_core::Error> for ApiError {
    fn from(err: evo_core::Error) -> Self {
        error!("Core error: {}", err);
        ApiError::Internal(err.to_string())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        error!("Error: {}", err);
        ApiError::Internal(err.to_string())
    }
}
