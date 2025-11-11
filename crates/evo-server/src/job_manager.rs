//! Job management and distribution.

use crate::evolution::EvolutionEngine;
use dashmap::DashMap;
use evo_core::{JobId, Result};
use evo_world::IslandJob;
use parking_lot::RwLock;
use std::time::{Duration, Instant};
use tracing::{debug, warn, instrument};

#[derive(Debug, Clone)]
pub struct JobStats {
    pub total_jobs: usize,
    pub pending_jobs: usize,
    pub completed_jobs: usize,
}

struct JobInfo {
    job: IslandJob,
    assigned_at: Instant,
}

pub struct JobManager {
    pending_jobs: RwLock<Vec<IslandJob>>,
    assigned_jobs: DashMap<JobId, JobInfo>,
    completed_jobs: RwLock<usize>,
    total_jobs: RwLock<usize>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            pending_jobs: RwLock::new(Vec::new()),
            assigned_jobs: DashMap::new(),
            completed_jobs: RwLock::new(0),
            total_jobs: RwLock::new(0),
        }
    }

    /// Get a job for a worker
    #[instrument(skip(self, evolution))]
    pub async fn get_job(&self, evolution: &EvolutionEngine) -> Result<IslandJob> {
        // First, try to get a pending job
        {
            let mut pending = self.pending_jobs.write();
            if let Some(job) = pending.pop() {
                let job_id = job.job_id;
                self.assigned_jobs.insert(
                    job_id,
                    JobInfo {
                        job: job.clone(),
                        assigned_at: Instant::now(),
                    },
                );
                debug!("Assigned pending job: {:?}", job_id);
                return Ok(job);
            }
        }

        // If no pending jobs, create a new one
        let job = evolution.create_job().await?;
        let job_id = job.job_id;

        {
            let mut total = self.total_jobs.write();
            *total += 1;
        }

        self.assigned_jobs.insert(
            job_id,
            JobInfo {
                job: job.clone(),
                assigned_at: Instant::now(),
            },
        );

        debug!("Created and assigned new job: {:?}", job_id);
        Ok(job)
    }

    /// Mark a job as completed
    #[instrument(skip(self))]
    pub async fn mark_job_complete(&self, job_id: JobId) {
        if self.assigned_jobs.remove(&job_id).is_some() {
            let mut completed = self.completed_jobs.write();
            *completed += 1;
            debug!("Job completed: {:?}", job_id);
        }
    }

    /// Check for timed-out jobs and reassign them
    #[instrument(skip(self))]
    pub async fn check_timeouts(&self, timeout: Duration) {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for entry in self.assigned_jobs.iter() {
            if now.duration_since(entry.value().assigned_at) > timeout {
                timed_out.push(*entry.key());
            }
        }

        if !timed_out.is_empty() {
            warn!("Found {} timed-out jobs", timed_out.len());

            let mut pending = self.pending_jobs.write();
            for job_id in timed_out {
                if let Some((_, info)) = self.assigned_jobs.remove(&job_id) {
                    pending.push(info.job);
                }
            }
        }
    }

    /// Get job statistics
    #[instrument(skip(self))]
    pub async fn get_stats(&self) -> JobStats {
        let pending = self.pending_jobs.read().len();
        let assigned = self.assigned_jobs.len();
        let completed = *self.completed_jobs.read();
        let total = *self.total_jobs.read();

        JobStats {
            total_jobs: total,
            pending_jobs: pending + assigned,
            completed_jobs: completed,
        }
    }

    /// Add a job to the queue
    #[instrument(skip(self, job), fields(job_id = ?job.job_id))]
    pub async fn enqueue_job(&self, job: IslandJob) {
        let mut pending = self.pending_jobs.write();
        pending.push(job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_job_manager() {
        let manager = JobManager::new();
        let stats = manager.get_stats().await;

        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.pending_jobs, 0);
        assert_eq!(stats.completed_jobs, 0);
    }
}
