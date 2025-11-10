//! Database layer for persisting genomes and statistics.

use evo_core::{LineageId, Result, Error};
use evo_ir::Program;
use evo_world::IslandJob;
use sqlx::{sqlite::SqlitePool, Row};
use std::path::Path;
use tracing::info;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Database(format!("Failed to create database directory: {}", e))
            })?;
        }

        let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", path))
            .await
            .map_err(|e| Error::Database(format!("Failed to connect to database: {}", e)))?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS genomes (
                lineage_id TEXT PRIMARY KEY,
                genome_data BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Migration failed: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                job_id TEXT PRIMARY KEY,
                job_data BLOB NOT NULL,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Migration failed: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS checkpoints (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                checkpoint_data BLOB NOT NULL,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Migration failed: {}", e)))?;

        info!("Database migrations complete");
        Ok(())
    }

    pub async fn store_genome(&self, lineage_id: LineageId, genome: &Program) -> Result<()> {
        let genome_bytes = genome.to_bytes()?;
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO genomes (lineage_id, genome_data, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(lineage_id) DO UPDATE SET
                genome_data = ?2,
                updated_at = ?4
            "#,
        )
        .bind(lineage_id.0.to_string())
        .bind(&genome_bytes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to store genome: {}", e)))?;

        Ok(())
    }

    pub async fn get_genome(&self, lineage_id: LineageId) -> Result<Option<Program>> {
        let row = sqlx::query("SELECT genome_data FROM genomes WHERE lineage_id = ?1")
            .bind(lineage_id.0.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to get genome: {}", e)))?;

        match row {
            Some(row) => {
                let genome_bytes: Vec<u8> = row.get("genome_data");
                let program = Program::from_bytes(&genome_bytes)?;
                Ok(Some(program))
            }
            None => Ok(None),
        }
    }

    pub async fn get_all_genomes(&self) -> Result<Vec<(LineageId, Program)>> {
        let rows = sqlx::query("SELECT lineage_id, genome_data FROM genomes")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to get all genomes: {}", e)))?;

        let mut genomes = Vec::new();
        for row in rows {
            let lineage_id_str: String = row.get("lineage_id");
            let lineage_id = LineageId(
                uuid::Uuid::parse_str(&lineage_id_str)
                    .map_err(|e| Error::Database(format!("Invalid lineage ID: {}", e)))?,
            );
            let genome_bytes: Vec<u8> = row.get("genome_data");
            let program = Program::from_bytes(&genome_bytes)?;
            genomes.push((lineage_id, program));
        }

        Ok(genomes)
    }

    pub async fn count_lineages(&self) -> Result<usize> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM genomes")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to count lineages: {}", e)))?;

        let count: i64 = row.get("count");
        Ok(count as usize)
    }

    pub async fn store_job(&self, job: &IslandJob) -> Result<()> {
        let job_bytes = bincode::serialize(job)
            .map_err(|e| Error::Serialization(format!("Failed to serialize job: {}", e)))?;
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO jobs (job_id, job_data, created_at)
            VALUES (?1, ?2, ?3)
            "#,
        )
        .bind(job.job_id.0.to_string())
        .bind(&job_bytes)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to store job: {}", e)))?;

        Ok(())
    }

    pub async fn store_checkpoint(&self, checkpoint_data: &[u8]) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO checkpoints (checkpoint_data, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(checkpoint_data)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to store checkpoint: {}", e)))?;

        Ok(())
    }

    pub async fn get_latest_checkpoint(&self) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query(
            "SELECT checkpoint_data FROM checkpoints ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to get checkpoint: {}", e)))?;

        Ok(row.map(|r| r.get("checkpoint_data")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evo_ir::{instruction::*, program::*};

    async fn create_test_db() -> Database {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db
    }

    fn create_test_genome() -> Program {
        let mut program = Program::new();
        let mut init = Function::new("init".to_string(), 1, ReturnType::Void);
        init.get_block_mut(0)
            .unwrap()
            .add_instruction(Instruction::return_void());
        program.add_function(init);

        let mut step = Function::new("step".to_string(), 1, ReturnType::Int);
        step.get_block_mut(0)
            .unwrap()
            .add_instruction(Instruction::load_const(Register(0), Value::Int(42)));
        step.get_block_mut(0)
            .unwrap()
            .add_instruction(Instruction::return_value(Register(0)));
        program.add_function(step);
        program
    }

    #[tokio::test]
    async fn test_store_and_retrieve_genome() {
        let db = create_test_db().await;
        let lineage_id = LineageId::new();
        let genome = create_test_genome();

        db.store_genome(lineage_id, &genome).await.unwrap();

        let retrieved = db.get_genome(lineage_id).await.unwrap();
        assert!(retrieved.is_some());

        let retrieved_genome = retrieved.unwrap();
        assert_eq!(retrieved_genome.num_functions(), genome.num_functions());
    }

    #[tokio::test]
    async fn test_count_lineages() {
        let db = create_test_db().await;
        assert_eq!(db.count_lineages().await.unwrap(), 0);

        let lineage_id = LineageId::new();
        let genome = create_test_genome();
        db.store_genome(lineage_id, &genome).await.unwrap();

        assert_eq!(db.count_lineages().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_all_genomes() {
        let db = create_test_db().await;

        for _ in 0..5 {
            let lineage_id = LineageId::new();
            let genome = create_test_genome();
            db.store_genome(lineage_id, &genome).await.unwrap();
        }

        let all_genomes = db.get_all_genomes().await.unwrap();
        assert_eq!(all_genomes.len(), 5);
    }
}
