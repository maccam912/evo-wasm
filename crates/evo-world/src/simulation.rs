//! Simulation engine for running an island.

use crate::grid::Grid;
use crate::organism::{Organism, OrganismData};
use evo_core::{
    EnergyConfig, Error, FitnessMetrics, JobConfig, LineageId, OrganismId, Position, Result,
    TileType,
};
use evo_ir::{Compiler, Mutator, MutationConfig, Program};
use evo_runtime::{HostFunctions, OrganismContext, Runtime, RuntimeConfig};
use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct Simulation {
    grid: Grid,
    organisms: HashMap<OrganismId, Organism>,
    organism_positions: HashMap<Position, OrganismId>,
    runtime: Runtime,
    compiler: Compiler,
    mutator: Mutator,
    config: JobConfig,
    rng: ChaCha8Rng,
    tick: u64,
}

impl Simulation {
    pub fn new(config: JobConfig, genomes: Vec<(LineageId, Program)>) -> Result<Self> {
        let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
        let grid = Grid::from_config(&config.world_config, &mut rng);

        let runtime_config = RuntimeConfig {
            max_fuel: config.exec_config.max_fuel_per_step,
            max_memory_bytes: config.exec_config.max_memory_bytes,
        };
        let runtime = Runtime::new(runtime_config)?;

        let compiler = Compiler::new(evo_ir::compiler::CompilerConfig::default());
        let mutator = Mutator::new(MutationConfig::default());

        let mut sim = Self {
            grid,
            organisms: HashMap::new(),
            organism_positions: HashMap::new(),
            runtime,
            compiler,
            mutator,
            config,
            rng,
            tick: 0,
        };

        // Spawn initial organisms
        for (lineage_id, genome) in genomes {
            sim.spawn_organism(lineage_id, genome)?;
        }

        Ok(sim)
    }

    /// Run the simulation for the specified number of ticks
    pub fn run(&mut self) -> Result<SimulationResult> {
        info!("Starting simulation for {} ticks", self.config.num_ticks);

        for tick in 0..self.config.num_ticks {
            self.tick = tick;
            self.step()?;

            if tick % 1000 == 0 {
                info!(
                    "Tick {}/{}: {} organisms alive",
                    tick,
                    self.config.num_ticks,
                    self.organisms.len()
                );
            }
        }

        Ok(self.collect_results())
    }

    /// Execute one simulation step
    fn step(&mut self) -> Result<()> {
        // Regenerate resources
        self.grid
            .regenerate_resources(self.config.world_config.resource_regen_rate);

        // Get list of organism IDs to process (to avoid borrow issues)
        let organism_ids: Vec<OrganismId> = self.organisms.keys().copied().collect();

        // Shuffle for fairness
        let mut shuffled_ids = organism_ids.clone();
        shuffled_ids.shuffle(&mut self.rng);

        // Process each organism
        for id in shuffled_ids {
            self.process_organism(id)?;
        }

        // Apply hazard damage
        self.apply_hazards();

        // Remove dead organisms
        self.remove_dead_organisms();

        Ok(())
    }

    fn process_organism(&mut self, id: OrganismId) -> Result<()> {
        // Check if organism still exists (might have been killed)
        if !self.organisms.contains_key(&id) {
            return Ok(());
        }

        // Get organism (we'll need to work around borrow checker)
        let organism = self.organisms.get_mut(&id).unwrap();

        // Apply basal metabolic cost
        if !organism.consume_energy(self.config.energy_config.basal_cost) {
            organism.finalize_metrics(self.config.energy_config.initial_energy);
            return Ok(()); // Will be removed in cleanup phase
        }

        organism.tick();

        // Compile and execute organism if not already done
        if organism.instance.is_none() {
            let wasm_bytes = self.compiler.compile(&organism.genome)?;
            let position = organism.position;
            let energy = organism.energy;

            // Create context with environment query
            let grid_ptr = &self.grid as *const Grid;
            let env_query = Arc::new(move |x: i32, y: i32| {
                let grid = unsafe { &*grid_ptr };
                let tile = grid.get(Position::new(x, y));
                match tile.tile_type {
                    TileType::Empty => 0,
                    TileType::Resource => 1,
                    TileType::Obstacle => 2,
                    TileType::Hazard => 3,
                }
            });

            let context = Arc::new(OrganismContext::new(id, energy, position, env_query));
            let host_functions = HostFunctions::new(context.clone());

            let mut instance = self.runtime.instantiate(&wasm_bytes, host_functions)?;
            instance.init(self.rng.gen())?;

            organism.instance = Some(instance);
        }

        // Execute step
        let instance = organism.instance.as_mut().unwrap();
        let (_, actions) = match instance.step(0) {
            Ok(result) => result,
            Err(e) => {
                warn!("Organism {:?} execution failed: {}", id, e);
                return Ok(());
            }
        };

        // Apply instruction cost
        let fuel_used = instance.fuel_consumed();
        let instruction_cost =
            (fuel_used / 1000) as i32 * self.config.energy_config.instruction_cost_per_k;
        organism.consume_energy(instruction_cost);

        // Process actions (collect them first to avoid borrow issues)
        let organism_pos = organism.position;
        let organism_energy = organism.energy;

        for action in actions {
            self.apply_action(id, action, organism_pos, organism_energy)?;
        }

        Ok(())
    }

    fn apply_action(
        &mut self,
        id: OrganismId,
        action: evo_runtime::context::Action,
        pos: Position,
        energy: i32,
    ) -> Result<()> {
        use evo_runtime::context::Action;

        match action {
            Action::Move { dx, dy } => {
                if energy < self.config.energy_config.move_cost {
                    return Ok(());
                }

                let new_pos = pos.add(dx, dy).wrap(self.grid.width, self.grid.height);

                // Check if target is occupied
                if !self.organism_positions.contains_key(&new_pos) {
                    // Check if target is passable
                    let tile = self.grid.get(new_pos);
                    if tile.tile_type != TileType::Obstacle {
                        // Move organism
                        self.organism_positions.remove(&pos);
                        self.organism_positions.insert(new_pos, id);

                        if let Some(organism) = self.organisms.get_mut(&id) {
                            organism.move_to(new_pos);
                            organism.consume_energy(self.config.energy_config.move_cost);
                        }
                    }
                }
            }

            Action::Eat => {
                if let Some(organism) = self.organisms.get_mut(&id) {
                    let tile = self.grid.get_mut(organism.position);
                    if tile.tile_type == TileType::Resource && tile.resource_amount > 0 {
                        let consumed = tile.resource_amount.min(100);
                        tile.resource_amount -= consumed;

                        let energy_gained =
                            (consumed as f32 * self.config.energy_config.eat_efficiency) as i32;
                        organism.add_energy(energy_gained);
                        organism.metrics.times_eaten += 1;
                    }
                }
            }

            Action::Attack { target_slot: _, amount } if self.config.dynamic_rules.allow_combat => {
                if energy < self.config.energy_config.attack_cost {
                    return Ok(());
                }

                // Find target in neighboring cells
                // This is simplified - a full implementation would use target_slot properly
                let neighbors = self.grid.neighbors(pos, 1);
                if let Some((target_pos, _)) = neighbors.first() {
                    if let Some(&target_id) = self.organism_positions.get(target_pos) {
                        // First, damage the target
                        let target_died = if let Some(target) = self.organisms.get_mut(&target_id) {
                            target.record_damage_received(amount);
                            target.consume_energy(amount);
                            target.energy <= 0
                        } else {
                            false
                        };

                        // Then update attacker
                        if let Some(attacker) = self.organisms.get_mut(&id) {
                            attacker.consume_energy(self.config.energy_config.attack_cost);
                            attacker.record_damage_dealt(amount);
                            if target_died {
                                attacker.record_kill();
                            }
                        }
                    }
                }
            }

            Action::Reproduce if self.config.dynamic_rules.allow_reproduction => {
                if energy < self.config.energy_config.reproduce_cost
                    || energy < self.config.energy_config.min_reproduce_energy
                {
                    return Ok(());
                }

                if self.organisms.len() >= self.config.dynamic_rules.max_population {
                    return Ok(());
                }

                // Create offspring
                if let Some(parent) = self.organisms.get_mut(&id) {
                    parent.consume_energy(self.config.energy_config.reproduce_cost);
                    parent.record_offspring();

                    // Mutate genome
                    let mut offspring_genome = parent.genome.clone();
                    self.mutator.mutate(&mut offspring_genome, &mut self.rng);

                    // Find empty adjacent cell
                    let neighbors = self.grid.neighbors(parent.position, 1);
                    for (neighbor_pos, tile) in neighbors {
                        let wrapped = neighbor_pos.wrap(self.grid.width, self.grid.height);
                        if tile.tile_type != TileType::Obstacle
                            && !self.organism_positions.contains_key(&wrapped)
                        {
                            // Spawn offspring
                            let offspring = Organism::new(
                                parent.lineage_id,
                                wrapped,
                                self.config.energy_config.initial_energy / 2,
                                offspring_genome,
                            );
                            let offspring_id = offspring.id;
                            self.organism_positions.insert(wrapped, offspring_id);
                            self.organisms.insert(offspring_id, offspring);
                            break;
                        }
                    }
                }
            }

            Action::EmitSignal { channel, value } => {
                // Signals are recorded but not yet processed
                debug!("Organism {:?} emitted signal {} on channel {}", id, value, channel);
            }

            _ => {}
        }

        Ok(())
    }

    fn apply_hazards(&mut self) {
        let hazard_damage = self.config.world_config.hazard_damage;

        for (pos, id) in self.organism_positions.iter() {
            let tile = self.grid.get(*pos);
            if tile.tile_type == TileType::Hazard {
                if let Some(organism) = self.organisms.get_mut(id) {
                    organism.consume_energy(hazard_damage);
                }
            }
        }
    }

    fn remove_dead_organisms(&mut self) {
        let dead: Vec<OrganismId> = self
            .organisms
            .iter()
            .filter(|(_, org)| !org.is_alive())
            .map(|(id, _)| *id)
            .collect();

        for id in dead {
            if let Some(organism) = self.organisms.remove(&id) {
                self.organism_positions.remove(&organism.position);
                debug!("Organism {:?} died at age {}", id, organism.age);
            }
        }
    }

    fn spawn_organism(&mut self, lineage_id: LineageId, genome: Program) -> Result<()> {
        // Find random empty position
        for _ in 0..100 {
            let x = self.rng.gen_range(0..self.grid.width);
            let y = self.rng.gen_range(0..self.grid.height);
            let pos = Position::new(x, y);

            if !self.organism_positions.contains_key(&pos) {
                let tile = self.grid.get(pos);
                if tile.tile_type != TileType::Obstacle {
                    let organism = Organism::new(
                        lineage_id,
                        pos,
                        self.config.energy_config.initial_energy,
                        genome,
                    );
                    let id = organism.id;
                    self.organism_positions.insert(pos, id);
                    self.organisms.insert(id, organism);
                    return Ok(());
                }
            }
        }

        Err(Error::Other("Failed to find spawn position".to_string()))
    }

    fn collect_results(&mut self) -> SimulationResult {
        let mut lineage_stats: HashMap<LineageId, Vec<FitnessMetrics>> = HashMap::new();
        let mut survivors: Vec<OrganismData> = Vec::new();

        // Finalize all organisms
        for organism in self.organisms.values_mut() {
            organism.finalize_metrics(self.config.energy_config.initial_energy);

            lineage_stats
                .entry(organism.lineage_id)
                .or_insert_with(Vec::new)
                .push(organism.metrics.clone());

            survivors.push(OrganismData::from(&*organism));
        }

        SimulationResult {
            lineage_stats,
            survivors,
            total_ticks: self.tick,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulationResult {
    pub lineage_stats: HashMap<LineageId, Vec<FitnessMetrics>>,
    pub survivors: Vec<OrganismData>,
    pub total_ticks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use evo_core::JobConfig;
    use evo_ir::{instruction::*, program::*};

    fn create_test_genome() -> Program {
        let mut program = Program::new();

        // Simple init function
        let mut init = Function::new("init".to_string(), 1, ReturnType::Void);
        init.get_block_mut(0)
            .unwrap()
            .add_instruction(Instruction::return_void());
        program.add_function(init);

        // Simple step function that just returns
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
    fn test_simulation_creation() {
        let config = JobConfig {
            num_ticks: 100,
            seed: 42,
            ..Default::default()
        };

        let genome = create_test_genome();
        let genomes = vec![(LineageId::new(), genome)];

        let sim = Simulation::new(config, genomes);
        assert!(sim.is_ok());
    }
}
