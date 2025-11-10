//! Island simulation wrapper for distributed execution.

use crate::simulation::{Simulation, SimulationResult};
use evo_core::{JobConfig, JobId, LineageId, Result};
use evo_ir::Program;
use serde::{Deserialize, Serialize};

/// An island job that can be executed by a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandJob {
    pub job_id: JobId,
    pub config: JobConfig,
    pub genomes: Vec<(LineageId, Vec<u8>)>, // Serialized programs
}

impl IslandJob {
    pub fn new(job_id: JobId, config: JobConfig, genomes: Vec<(LineageId, Program)>) -> Result<Self> {
        let serialized_genomes = genomes
            .into_iter()
            .map(|(id, program)| program.to_bytes().map(|bytes| (id, bytes)))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            job_id,
            config,
            genomes: serialized_genomes,
        })
    }

    /// Execute this island job
    pub fn execute(self) -> Result<IslandResult> {
        // Deserialize genomes
        let genomes: Vec<(LineageId, Program)> = self
            .genomes
            .into_iter()
            .map(|(id, bytes)| Program::from_bytes(&bytes).map(|program| (id, program)))
            .collect::<Result<Vec<_>>>()?;

        // Run simulation
        let mut simulation = Simulation::new(self.config, genomes)?;
        let result = simulation.run()?;

        Ok(IslandResult {
            job_id: self.job_id,
            result,
        })
    }
}

/// Result from executing an island job
#[derive(Debug, Serialize, Deserialize)]
pub struct IslandResult {
    pub job_id: JobId,
    pub result: SimulationResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use evo_core::JobConfig;
    use evo_ir::{instruction::*, program::*};

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
            .add_instruction(Instruction::load_const(Register(0), Value::Int(0)));
        step.get_block_mut(0)
            .unwrap()
            .add_instruction(Instruction::return_value(Register(0)));
        program.add_function(step);

        program
    }

    #[test]
    fn test_island_job_creation() {
        let config = JobConfig {
            num_ticks: 100,
            ..Default::default()
        };

        let genome = create_test_genome();
        let genomes = vec![(LineageId::new(), genome)];

        let job = IslandJob::new(JobId::new(), config, genomes);
        assert!(job.is_ok());
    }
}
