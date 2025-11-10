//! Checkpoint and restore functionality.

use crate::database::Database;
use evo_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    pub timestamp: i64,
    pub data: CheckpointData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckpointData {
    // Add server state that needs to be checkpointed
    pub num_jobs_created: u64,
    pub num_jobs_completed: u64,
}

pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
    db: Database,
}

impl CheckpointManager {
    pub fn new(checkpoint_dir: String, db: Database) -> Self {
        Self {
            checkpoint_dir: PathBuf::from(checkpoint_dir),
            db,
        }
    }

    /// Create a checkpoint
    pub async fn create_checkpoint(&self) -> Result<()> {
        info!("Creating checkpoint");

        // Ensure checkpoint directory exists
        fs::create_dir_all(&self.checkpoint_dir)
            .await
            .map_err(|e| Error::Io(e))?;

        // Create checkpoint data
        let checkpoint = Checkpoint {
            version: 1,
            timestamp: chrono::Utc::now().timestamp(),
            data: CheckpointData {
                num_jobs_created: 0,
                num_jobs_completed: 0,
            },
        };

        // Serialize checkpoint
        let checkpoint_bytes = bincode::serialize(&checkpoint)
            .map_err(|e| Error::Serialization(format!("Failed to serialize checkpoint: {}", e)))?;

        // Store in database
        self.db.store_checkpoint(&checkpoint_bytes).await?;

        // Also write to file for redundancy
        let checkpoint_path = self
            .checkpoint_dir
            .join(format!("checkpoint_{}.bin", checkpoint.timestamp));
        fs::write(&checkpoint_path, &checkpoint_bytes)
            .await
            .map_err(|e| Error::Io(e))?;

        info!("Checkpoint created at {:?}", checkpoint_path);
        Ok(())
    }

    /// Restore from the latest checkpoint
    pub async fn restore_latest(&self) -> Result<()> {
        info!("Attempting to restore from latest checkpoint");

        // Try to restore from database first
        if let Some(checkpoint_bytes) = self.db.get_latest_checkpoint().await? {
            return self.restore_from_bytes(&checkpoint_bytes);
        }

        // Fall back to file system
        if !self.checkpoint_dir.exists() {
            warn!("No checkpoint directory found");
            return Err(Error::NotFound("No checkpoints found".to_string()));
        }

        // Find latest checkpoint file
        let mut entries = fs::read_dir(&self.checkpoint_dir)
            .await
            .map_err(|e| Error::Io(e))?;

        let mut latest: Option<(PathBuf, i64)> = None;

        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Io(e))? {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("checkpoint_") && name.ends_with(".bin") {
                    if let Some(timestamp_str) = name
                        .strip_prefix("checkpoint_")
                        .and_then(|s| s.strip_suffix(".bin"))
                    {
                        if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                            if latest.is_none() || timestamp > latest.as_ref().unwrap().1 {
                                latest = Some((path.clone(), timestamp));
                            }
                        }
                    }
                }
            }
        }

        if let Some((path, _)) = latest {
            let checkpoint_bytes = fs::read(&path).await.map_err(|e| Error::Io(e))?;
            self.restore_from_bytes(&checkpoint_bytes)?;
            info!("Restored from checkpoint: {:?}", path);
            Ok(())
        } else {
            Err(Error::NotFound("No checkpoint files found".to_string()))
        }
    }

    fn restore_from_bytes(&self, bytes: &[u8]) -> Result<()> {
        let checkpoint: Checkpoint = bincode::deserialize(bytes)
            .map_err(|e| Error::Serialization(format!("Failed to deserialize checkpoint: {}", e)))?;

        info!(
            "Restoring checkpoint from timestamp: {}",
            checkpoint.timestamp
        );

        // Restore server state here
        // For now, just log the data
        info!("Checkpoint data: {:?}", checkpoint.data);

        Ok(())
    }

    /// Start periodic checkpoint creation
    pub async fn start_periodic_checkpoints(&self, interval_secs: u64) {
        let mut interval = interval(Duration::from_secs(interval_secs));

        loop {
            interval.tick().await;

            if let Err(e) = self.create_checkpoint().await {
                error!("Failed to create checkpoint: {}", e);
            }
        }
    }

    /// Clean up old checkpoints, keeping only the most recent N
    pub async fn cleanup_old_checkpoints(&self, keep_count: usize) -> Result<()> {
        if !self.checkpoint_dir.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(&self.checkpoint_dir)
            .await
            .map_err(|e| Error::Io(e))?;

        let mut checkpoints: Vec<(PathBuf, i64)> = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Io(e))? {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("checkpoint_") && name.ends_with(".bin") {
                    if let Some(timestamp_str) = name
                        .strip_prefix("checkpoint_")
                        .and_then(|s| s.strip_suffix(".bin"))
                    {
                        if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                            checkpoints.push((path, timestamp));
                        }
                    }
                }
            }
        }

        if checkpoints.len() <= keep_count {
            return Ok(());
        }

        // Sort by timestamp descending
        checkpoints.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove old checkpoints
        for (path, _) in checkpoints.iter().skip(keep_count) {
            if let Err(e) = fs::remove_file(path).await {
                warn!("Failed to remove old checkpoint {:?}: {}", path, e);
            } else {
                info!("Removed old checkpoint: {:?}", path);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_checkpoint_serialization() {
        let checkpoint = Checkpoint {
            version: 1,
            timestamp: chrono::Utc::now().timestamp(),
            data: CheckpointData {
                num_jobs_created: 100,
                num_jobs_completed: 95,
            },
        };

        let bytes = bincode::serialize(&checkpoint).unwrap();
        let deserialized: Checkpoint = bincode::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.version, checkpoint.version);
        assert_eq!(
            deserialized.data.num_jobs_created,
            checkpoint.data.num_jobs_created
        );
    }
}
