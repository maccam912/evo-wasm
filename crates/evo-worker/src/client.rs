//! Worker client for communicating with the server.

use anyhow::Result;
use evo_core::WorkerConfig;
use evo_world::{IslandJob, IslandResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

pub struct WorkerClient {
    config: WorkerConfig,
    http_client: Client,
    worker_id: String,
}

#[derive(Serialize)]
struct JobRequest {
    worker_id: Option<String>,
}

impl WorkerClient {
    pub fn new(config: WorkerConfig) -> Result<Self> {
        let worker_id = config
            .worker_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minutes
            .build()?;

        Ok(Self {
            config,
            http_client,
            worker_id,
        })
    }

    pub fn config(&self) -> &WorkerConfig {
        &self.config
    }

    /// Request a job from the server
    #[instrument(skip(self))]
    pub async fn request_job(&self) -> Result<Option<IslandJob>> {
        let url = format!("{}/api/jobs/request", self.config.server_url);

        debug!("Requesting job from {}", url);

        let response = self
            .http_client
            .post(&url)
            .json(&JobRequest {
                worker_id: Some(self.worker_id.clone()),
            })
            .send()
            .await?;

        if response.status().is_success() {
            let job: IslandJob = response.json().await?;
            info!("Received job: {:?}", job.job_id);
            Ok(Some(job))
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            // No jobs available
            debug!("No jobs available");
            Ok(None)
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            warn!("Job request failed: {} - {}", status, error_text);
            Ok(None)
        }
    }

    /// Submit job results to the server
    #[instrument(skip(self, result))]
    pub async fn submit_result(&self, result: IslandResult) -> Result<()> {
        let url = format!("{}/api/jobs/submit", self.config.server_url);

        debug!("Submitting result for job: {:?}", result.job_id);

        let response = self
            .http_client
            .post(&url)
            .json(&result)
            .send()
            .await?;

        if response.status().is_success() {
            info!("Result submitted successfully for job: {:?}", result.job_id);
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            Err(anyhow::anyhow!(
                "Failed to submit result: {} - {}",
                status,
                error_text
            ))
        }
    }

    /// Execute a job
    #[instrument(skip(self, job))]
    pub async fn execute_job(&self, job: IslandJob) -> Result<IslandResult> {
        info!("Executing job: {:?}", job.job_id);

        let start = Instant::now();

        // Execute the island simulation
        let result = tokio::task::spawn_blocking(move || job.execute())
            .await??;

        let duration = start.elapsed();

        info!(
            "Job {:?} completed in {:.2}s",
            result.job_id,
            duration.as_secs_f64()
        );

        // Record metrics
        crate::record_histogram!("job_execution_duration_seconds", duration.as_secs_f64());
        crate::record_counter!("jobs_completed", 1);

        Ok(result)
    }

    /// Request and execute a job (convenience method)
    #[instrument(skip(self))]
    pub async fn request_and_execute_job(&self, task_id: usize) -> Result<Option<()>> {
        // Request job
        let job = match self.request_job().await? {
            Some(job) => job,
            None => return Ok(None),
        };

        // Record that we got a job
        crate::record_counter!("jobs_requested", 1, "task_id" => task_id);

        // Execute job
        let result = self.execute_job(job).await?;

        // Submit result
        self.submit_result(result).await?;

        Ok(Some(()))
    }

    /// Get current dynamic rules from the server
    pub async fn get_config(&self) -> Result<evo_core::JobConfig> {
        let url = format!("{}/api/config", self.config.server_url);

        debug!("Fetching config from {}", url);

        let response = self.http_client.get(&url).send().await?;

        if response.status().is_success() {
            let config: evo_core::JobConfig = response.json().await?;
            Ok(config)
        } else {
            Err(anyhow::anyhow!(
                "Failed to fetch config: {}",
                response.status()
            ))
        }
    }
}

// Telemetry macros (similar to server)

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_client_creation() {
        let config = WorkerConfig::default();
        let client = WorkerClient::new(config);
        assert!(client.is_ok());
    }
}
