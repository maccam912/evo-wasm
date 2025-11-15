//! Evolution engine for global selection and breeding.

use crate::database::Database;
use evo_core::{JobConfig, JobId, LineageId, LineageStats, Result};
use evo_ir::{Mutator, MutationConfig, Program};
use evo_world::{IslandJob, IslandResult};
use parking_lot::RwLock;
use rand::{seq::SliceRandom, Rng};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use tracing::{debug, info, instrument};

pub struct EvolutionEngine {
    db: Database,
    config: RwLock<JobConfig>,
    mutator: Mutator,
    lineage_stats: RwLock<HashMap<LineageId, LineageStats>>,
    rng: RwLock<ChaCha8Rng>,
}

impl EvolutionEngine {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            config: RwLock::new(JobConfig::default()),
            mutator: Mutator::new(MutationConfig::default()),
            lineage_stats: RwLock::new(HashMap::new()),
            rng: RwLock::new(ChaCha8Rng::from_entropy()),
        }
    }

    /// Create a new island job
    #[instrument(skip(self))]
    pub async fn create_job(&self) -> Result<IslandJob> {
        let job_id = JobId::new();
        let config = self.config.read().clone();

        // Select genomes for this island
        let genomes = self.select_genomes_for_job(10).await?;

        // Create job
        let job = IslandJob::new(job_id, config, genomes)?;

        // Store job in database
        self.db.store_job(&job).await?;

        info!("Created job {:?} with {} genomes", job_id, job.genomes.len());
        Ok(job)
    }

    /// Process results from a completed job
    #[instrument(skip(self, result), fields(job_id = ?result.job_id, survivors = result.result.survivors.len()))]
    pub async fn process_result(&self, result: IslandResult) -> Result<()> {
        info!(
            "Processing result for job {:?}: {} survivors",
            result.job_id,
            result.result.survivors.len()
        );

        // Update lineage statistics
        {
            let mut stats = self.lineage_stats.write();
            for (lineage_id, metrics_list) in &result.result.lineage_stats {
                let lineage_stat = stats.entry(*lineage_id).or_insert_with(|| {
                    LineageStats::new(*lineage_id)
                });

                for metrics in metrics_list {
                    lineage_stat.update(metrics);
                }
            }
        }

        // Store survivors in database
        for survivor in &result.result.survivors {
            self.db.store_genome(survivor.lineage_id, &survivor.genome).await?;
        }

        // Perform selection and breeding if we have enough data
        let num_lineages = self.lineage_stats.read().len();
        if num_lineages >= 20 {
            self.perform_selection().await?;
        }

        Ok(())
    }

    /// Select genomes for a new job
    #[instrument(skip(self))]
    async fn select_genomes_for_job(
        &self,
        count: usize,
    ) -> Result<Vec<(LineageId, Program)>> {
        // Get all lineages from database
        let all_genomes = self.db.get_all_genomes().await?;

        if all_genomes.is_empty() {
            // Bootstrap: create initial random genomes
            return self.create_initial_genomes(count);
        }

        // Select genomes based on fitness
        let stats = self.lineage_stats.read();
        let mut scored: Vec<(LineageId, f64, Program)> = all_genomes
            .into_iter()
            .map(|(id, program)| {
                let fitness = stats
                    .get(&id)
                    .map(|s| s.best_fitness.scalar_fitness())
                    .unwrap_or(0.0);
                (id, fitness, program)
            })
            .collect();

        // Sort by fitness
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Take top performers + some random ones for diversity
        let mut selected = Vec::new();
        let top_count = count * 7 / 10; // 70% top performers
        let random_count = count - top_count; // 30% random

        for (id, _, program) in scored.iter().take(top_count) {
            selected.push((*id, program.clone()));
        }

        // Add random genomes for diversity
        let mut rng = self.rng.write();
        for (id, _, program) in scored.choose_multiple(&mut *rng, random_count) {
            selected.push((*id, program.clone()));
        }

        Ok(selected)
    }

    /// Perform selection and breeding to create new genomes
    #[instrument(skip(self))]
    async fn perform_selection(&self) -> Result<()> {
        info!("Performing selection and breeding");

        let survivors = {
            let stats = self.lineage_stats.read();

            // Find best lineages using Pareto ranking
            let mut lineages: Vec<(LineageId, &LineageStats)> = stats.iter()
                .map(|(id, stat)| (*id, stat))
                .collect();

            // Sort by scalar fitness for simplicity
            // TODO: Implement proper Pareto ranking
            lineages.sort_by(|a, b| {
                b.1.best_fitness
                    .scalar_fitness()
                    .partial_cmp(&a.1.best_fitness.scalar_fitness())
                    .unwrap()
            });

            // Keep top 50%
            let keep_count = lineages.len() / 2;
            let survivors: Vec<LineageId> = lineages
                .iter()
                .take(keep_count)
                .map(|(id, _)| *id)
                .collect();

            info!("Selected {} survivors from {} lineages", survivors.len(), lineages.len());

            survivors
        }; // stats lock is dropped here

        // Create offspring through mutation and crossover
        let offspring_count = 10;

        for _ in 0..offspring_count {
            // Select two random parents
            if survivors.len() >= 2 {
                let (parent1_id, parent2_id) = {
                    let mut rng = self.rng.write();
                    (
                        survivors[(*rng).gen_range(0..survivors.len())],
                        survivors[(*rng).gen_range(0..survivors.len())]
                    )
                }; // rng lock is dropped here

                if let (Ok(Some(parent1)), Ok(Some(parent2))) = (
                    self.db.get_genome(parent1_id).await,
                    self.db.get_genome(parent2_id).await,
                ) {
                    // Crossover and mutate
                    let child = {
                        let mut rng = self.rng.write();
                        let child = self.mutator.crossover(&parent1, &parent2, &mut *rng);
                        self.mutator.mutate(&mut child.clone(), &mut *rng);
                        child
                    }; // rng lock is dropped here

                    // Store new lineage
                    let new_lineage_id = LineageId::new();
                    self.db.store_genome(new_lineage_id, &child).await?;
                    debug!("Created new lineage: {:?}", new_lineage_id);
                }
            }
        }

        Ok(())
    }

    /// Create initial random genomes
    #[instrument(skip(self))]
    fn create_initial_genomes(&self, count: usize) -> Result<Vec<(LineageId, Program)>> {
        info!("Creating {} initial genomes", count);

        let mut genomes = Vec::new();
        let mut rng = self.rng.write();

        for _ in 0..count {
            let lineage_id = LineageId::new();

            // Create a simple initial program
            let mut program = Program::new();

            // Add init function
            use evo_ir::{instruction::*, program::*};

            let mut init = Function::new("init".to_string(), 1, ReturnType::Void);
            init.get_block_mut(0)
                .unwrap()
                .add_instruction(Instruction::return_void());
            program.add_function(init);

            // Add simple step function that moves randomly and eats
            let mut step = Function::new("step".to_string(), 1, ReturnType::Int);
            let block = step.get_block_mut(0).unwrap();

            // Load random direction
            block.add_instruction(Instruction::load_const(
                Register(0),
                Value::Int((*rng).gen_range(-1..=1)),
            ));
            block.add_instruction(Instruction::load_const(
                Register(1),
                Value::Int((*rng).gen_range(-1..=1)),
            ));

            // Move
            block.add_instruction(
                Instruction::new(Opcode::Move)
                    .with_operands(vec![Operand::Register(Register(0)), Operand::Register(Register(1))]),
            );

            // Eat
            block.add_instruction(
                Instruction::new(Opcode::Eat).with_dest(Register(2)),
            );

            // Try to reproduce (CRITICAL: without this, organisms never reproduce!)
            // Reuse Register(0) since move directions are no longer needed
            block.add_instruction(
                Instruction::new(Opcode::Reproduce).with_dest(Register(0)),
            );

            // Return
            block.add_instruction(Instruction::return_value(Register(2)));

            program.add_function(step);

            // Apply some random mutations
            self.mutator.mutate(&mut program, &mut *rng);

            genomes.push((lineage_id, program));
        }

        Ok(genomes)
    }

    /// Get current configuration
    #[instrument(skip(self))]
    pub async fn get_config(&self) -> JobConfig {
        self.config.read().clone()
    }

    /// Update configuration
    #[instrument(skip(self, config))]
    pub async fn update_config(&self, config: JobConfig) {
        *self.config.write() = config;
        info!("Configuration updated");
    }
}
