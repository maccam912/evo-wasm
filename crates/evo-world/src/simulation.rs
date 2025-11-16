//! Simulation engine for running an island.

use crate::grid::Grid;
use crate::organism::{Organism, OrganismData};
use evo_core::{
    EnergyConfig, Error, FitnessMetrics, JobConfig, LineageId, OrganismId, Position, Result,
    TileType,
};
use evo_ir::{Compiler, Mutator, MutationConfig, Program};
use evo_runtime::{HostFunctions, OrganismContext, Runtime, RuntimeConfig};
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn, instrument, trace, event, Level};

pub struct Simulation {
    grid: Arc<RwLock<Grid>>,
    organisms: HashMap<OrganismId, Organism>,
    organism_positions: HashMap<Position, OrganismId>,
    runtime: Runtime,
    compiler: Compiler,
    mutator: Mutator,
    config: JobConfig,
    rng: ChaCha8Rng,
    tick: u64,
    // Reproduction tracking for metrics
    reproduction_attempts: u64,
    reproduction_successes: u64,
    total_offspring_born: u64,
}

impl Simulation {
    pub fn new(config: JobConfig, genomes: Vec<(LineageId, Program)>) -> Result<Self> {
        let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
        let grid = Arc::new(RwLock::new(Grid::from_config(&config.world_config, &mut rng)));

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
            reproduction_attempts: 0,
            reproduction_successes: 0,
            total_offspring_born: 0,
        };

        // Spawn initial organisms
        for (lineage_id, genome) in genomes {
            sim.spawn_organism(lineage_id, genome)?;
        }

        Ok(sim)
    }

    /// Run the simulation for the specified number of ticks
    #[instrument(skip(self), fields(num_ticks = self.config.num_ticks))]
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

        // Emit comprehensive episode summary
        self.emit_episode_summary();

        Ok(self.collect_results())
    }

    /// Emit comprehensive episode summary for replay/analysis
    fn emit_episode_summary(&self) {
        let survivors = &self.organisms;
        let total_survivors = survivors.len();
        let survivors_born_after_tick_1: Vec<_> = survivors.values()
            .filter(|o| o.birth_tick > 1)
            .collect();

        let success_survivors_count = survivors_born_after_tick_1.len();

        // Calculate aggregate stats for successful organisms
        let mut success_stats = HashMap::new();
        if !survivors_born_after_tick_1.is_empty() {
            let total_offspring: u32 = survivors_born_after_tick_1.iter()
                .map(|o| o.metrics.offspring_count)
                .sum();
            let avg_age: f64 = survivors_born_after_tick_1.iter()
                .map(|o| o.age as f64)
                .sum::<f64>() / survivors_born_after_tick_1.len() as f64;
            let avg_energy: f64 = survivors_born_after_tick_1.iter()
                .map(|o| o.energy as f64)
                .sum::<f64>() / survivors_born_after_tick_1.len() as f64;
            let max_offspring = survivors_born_after_tick_1.iter()
                .map(|o| o.metrics.offspring_count)
                .max()
                .unwrap_or(0);

            success_stats.insert("total_offspring", total_offspring);
            success_stats.insert("avg_age", avg_age as u32);
            success_stats.insert("avg_energy", avg_energy as u32);
            success_stats.insert("max_offspring", max_offspring);
        }

        // Overall reproduction stats
        let success_rate = if self.reproduction_attempts > 0 {
            (self.reproduction_successes as f64 / self.reproduction_attempts as f64) * 100.0
        } else {
            0.0
        };

        info!(
            event = "episode_summary",
            total_ticks = self.config.num_ticks,
            final_tick = self.tick,
            total_survivors = total_survivors,
            survivors_born_after_tick_1 = success_survivors_count,
            reproduction_attempts_total = self.reproduction_attempts,
            reproduction_successes_total = self.reproduction_successes,
            reproduction_success_rate = format!("{:.2}%", success_rate),
            total_offspring_born_entire_simulation = self.total_offspring_born,
            "🏁 EPISODE COMPLETE - Summary Statistics"
        );

        // Detailed stats for successful organisms (born after tick 1 and survived)
        if !survivors_born_after_tick_1.is_empty() {
            info!(
                event = "successful_organisms_summary",
                count = success_survivors_count,
                total_offspring = success_stats.get("total_offspring").copied().unwrap_or(0),
                avg_age = success_stats.get("avg_age").copied().unwrap_or(0),
                avg_energy = success_stats.get("avg_energy").copied().unwrap_or(0),
                max_offspring = success_stats.get("max_offspring").copied().unwrap_or(0),
                "🌟 Successful organisms (born after tick 1, survived to end)"
            );

            // Log individual successful organisms for detailed analysis
            for organism in survivors_born_after_tick_1.iter().take(10) {
                info!(
                    event = "successful_organism_detail",
                    organism_id = ?organism.id,
                    lineage_id = ?organism.lineage_id,
                    birth_tick = organism.birth_tick,
                    final_age = organism.age,
                    final_energy = organism.energy,
                    offspring_count = organism.metrics.offspring_count,
                    tiles_explored = organism.metrics.tiles_explored,
                    times_eaten = organism.metrics.times_eaten,
                    kills = organism.metrics.kills,
                    position_x = organism.position.x,
                    position_y = organism.position.y,
                    "🏆 Top successful organism"
                );
            }
        } else {
            info!(
                event = "no_successful_organisms",
                total_survivors = total_survivors,
                "⚠️ No organisms born after tick 1 survived to the end"
            );
        }

        // Episode replay data: key moments
        event!(
            Level::INFO,
            histogram_name = "episode_duration",
            histogram_value = self.config.num_ticks,
            "Episode duration histogram"
        );

        event!(
            Level::INFO,
            gauge_name = "final_population",
            gauge_value = total_survivors,
            "Final population gauge"
        );

        event!(
            Level::INFO,
            gauge_name = "successful_organisms_final",
            gauge_value = success_survivors_count,
            "Successful organisms at end"
        );
    }

    /// Execute one simulation step
    // #[instrument(skip(self), fields(tick = self.tick))]
    fn step(&mut self) -> Result<()> {
        // Regenerate resources
        self.grid
            .write()
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

        // Periodic metrics (every 100 ticks)
        if self.tick % 100 == 0 && self.tick > 0 {
            self.emit_population_metrics();
        }

        Ok(())
    }

    /// Emit comprehensive population metrics
    fn emit_population_metrics(&self) {
        let total_pop = self.organisms.len();
        let born_after_tick_1 = self.organisms.values()
            .filter(|o| o.birth_tick > 1)
            .count();

        // Energy distribution
        let energies: Vec<i32> = self.organisms.values().map(|o| o.energy).collect();
        let avg_energy = if !energies.is_empty() {
            energies.iter().sum::<i32>() / energies.len() as i32
        } else {
            0
        };
        let max_energy = energies.iter().max().copied().unwrap_or(0);
        let min_energy = energies.iter().min().copied().unwrap_or(0);

        // Age distribution
        let ages: Vec<u64> = self.organisms.values().map(|o| o.age).collect();
        let avg_age = if !ages.is_empty() {
            ages.iter().sum::<u64>() / ages.len() as u64
        } else {
            0
        };
        let max_age = ages.iter().max().copied().unwrap_or(0);

        // Offspring counts
        let offspring_counts: Vec<u32> = self.organisms.values()
            .map(|o| o.metrics.offspring_count)
            .collect();
        let total_parents = offspring_counts.iter().filter(|&&c| c > 0).count();
        let total_offspring_alive = offspring_counts.iter().sum::<u32>();

        // Reproduction stats
        let success_rate = if self.reproduction_attempts > 0 {
            (self.reproduction_successes as f64 / self.reproduction_attempts as f64) * 100.0
        } else {
            0.0
        };

        info!(
            event = "population_metrics",
            tick = self.tick,
            total_population = total_pop,
            born_after_tick_1 = born_after_tick_1,
            avg_energy = avg_energy,
            max_energy = max_energy,
            min_energy = min_energy,
            avg_age = avg_age,
            max_age = max_age,
            total_parents = total_parents,
            total_offspring_alive = total_offspring_alive,
            reproduction_attempts = self.reproduction_attempts,
            reproduction_successes = self.reproduction_successes,
            reproduction_success_rate = format!("{:.2}%", success_rate),
            total_offspring_born = self.total_offspring_born,
            "Population metrics snapshot"
        );

        // Emit as gauge metrics for Grafana
        event!(
            Level::INFO,
            gauge_name = "population_total",
            gauge_value = total_pop,
            tick = self.tick,
            "Population gauge"
        );

        event!(
            Level::INFO,
            gauge_name = "population_born_after_tick_1",
            gauge_value = born_after_tick_1,
            tick = self.tick,
            "Population born after tick 1"
        );

        event!(
            Level::INFO,
            gauge_name = "avg_energy",
            gauge_value = avg_energy,
            tick = self.tick,
            "Average energy"
        );

        event!(
            Level::INFO,
            gauge_name = "reproduction_success_rate",
            gauge_value = success_rate as i32,
            tick = self.tick,
            "Reproduction success rate"
        );
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
            let grid_clone = self.grid.clone();
            let env_query = Arc::new(move |x: i32, y: i32| {
                let grid = grid_clone.read();
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

                let (width, height) = {
                    let grid = self.grid.read();
                    (grid.width, grid.height)
                };
                let new_pos = pos.add(dx, dy).wrap(width, height);

                // Check if target is occupied
                if !self.organism_positions.contains_key(&new_pos) {
                    // Check if target is passable
                    let tile_type = {
                        let grid = self.grid.read();
                        grid.get(new_pos).tile_type
                    };

                    if tile_type != TileType::Obstacle {
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
                    let organism_pos = organism.position;
                    let mut grid = self.grid.write();
                    let tile = grid.get_mut(organism_pos);
                    if tile.tile_type == TileType::Resource && tile.resource_amount > 0 {
                        let consumed = tile.resource_amount.min(100);
                        tile.resource_amount -= consumed;

                        let energy_gained =
                            (consumed as f32 * self.config.energy_config.eat_efficiency) as i32;
                        drop(grid); // Release lock before mutating organism
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
                let neighbors = {
                    let grid = self.grid.read();
                    grid.neighbors(pos, 1)
                };

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
                self.reproduction_attempts += 1;

                // Track why reproduction might fail
                let mut failure_reason = None;

                // Check energy requirements
                if energy < self.config.energy_config.reproduce_cost {
                    failure_reason = Some("insufficient_energy_for_cost");
                    trace!(
                        organism_id = ?id,
                        current_energy = energy,
                        reproduce_cost = self.config.energy_config.reproduce_cost,
                        tick = self.tick,
                        "Reproduction failed: insufficient energy for cost"
                    );
                } else if energy < self.config.energy_config.min_reproduce_energy {
                    failure_reason = Some("below_minimum_energy");
                    trace!(
                        organism_id = ?id,
                        current_energy = energy,
                        min_reproduce_energy = self.config.energy_config.min_reproduce_energy,
                        tick = self.tick,
                        "Reproduction failed: below minimum energy threshold"
                    );
                }

                if failure_reason.is_none() && self.organisms.len() >= self.config.dynamic_rules.max_population {
                    failure_reason = Some("max_population_reached");
                    trace!(
                        organism_id = ?id,
                        population = self.organisms.len(),
                        max_population = self.config.dynamic_rules.max_population,
                        tick = self.tick,
                        "Reproduction failed: max population reached"
                    );
                }

                if let Some(reason) = failure_reason {
                    // Log failed attempt
                    event!(
                        Level::DEBUG,
                        counter_name = "reproduction_failures",
                        counter_value = 1,
                        failure_reason = reason,
                        organism_id = ?id,
                        tick = self.tick,
                        "Reproduction attempt failed"
                    );
                    return Ok(());
                }

                // Get current population before any mutations
                let current_population = self.organisms.len();

                // Create offspring
                if let Some(parent) = self.organisms.get_mut(&id) {
                    let parent_energy_before = parent.energy;
                    let parent_age = parent.age;
                    let parent_birth_tick = parent.birth_tick;
                    let parent_offspring_count = parent.metrics.offspring_count;

                    parent.consume_energy(self.config.energy_config.reproduce_cost);
                    parent.record_offspring();

                    let parent_position = parent.position;
                    let parent_lineage = parent.lineage_id;

                    // Mutate genome
                    let mut offspring_genome = parent.genome.clone();
                    self.mutator.mutate(&mut offspring_genome, &mut self.rng);

                    // Find empty adjacent cell
                    let (neighbors, width, height) = {
                        let grid = self.grid.read();
                        (grid.neighbors(parent_position, 1), grid.width, grid.height)
                    };

                    let mut offspring_spawned = false;
                    for (neighbor_pos, tile) in neighbors {
                        let wrapped = neighbor_pos.wrap(width, height);
                        if tile.tile_type != TileType::Obstacle
                            && !self.organism_positions.contains_key(&wrapped)
                        {
                            // Base offspring energy
                            let base_offspring_energy = self.config.energy_config.initial_energy / 2;

                            // MASSIVE BUFF: If parent was born after tick 1 and is successfully reproducing,
                            // give offspring a huge energy bonus
                            let buff_multiplier = if parent_birth_tick > 1 {
                                5.0 // 5x energy bonus for offspring of successful reproducers!
                            } else {
                                1.0
                            };

                            let offspring_energy = (base_offspring_energy as f32 * buff_multiplier) as i32;

                            // Spawn offspring
                            let offspring = Organism::new_with_birth_tick(
                                parent_lineage,
                                wrapped,
                                offspring_energy,
                                offspring_genome.clone(),
                                self.tick,
                            );
                            let offspring_id = offspring.id;

                            // Get parent energy after reproduction
                            let parent_energy_after = parent.energy;

                            // Also give the PARENT a massive energy buff as a reward!
                            let parent_final_energy = if parent_birth_tick > 1 {
                                let parent_buff = 1000; // 1000 energy bonus for successful reproduction!
                                parent.add_energy(parent_buff);
                                let final_energy = parent.energy;

                                info!(
                                    event = "parent_reproduction_buff",
                                    parent_id = ?id,
                                    buff_amount = parent_buff,
                                    new_parent_energy = final_energy,
                                    tick = self.tick,
                                    "🌟 Parent received massive energy buff for successful reproduction!"
                                );
                                final_energy
                            } else {
                                parent.energy
                            };

                            // COMPREHENSIVE LOGGING for successful reproduction
                            info!(
                                event = "reproduction_success",
                                parent_id = ?id,
                                offspring_id = ?offspring_id,
                                lineage_id = ?parent_lineage,
                                tick = self.tick,
                                parent_age = parent_age,
                                parent_birth_tick = parent_birth_tick,
                                parent_energy_before = parent_energy_before,
                                parent_energy_after = parent_energy_after,
                                parent_final_energy = parent_final_energy,
                                parent_offspring_count = parent_offspring_count + 1,
                                offspring_energy = offspring_energy,
                                buff_applied = parent_birth_tick > 1,
                                buff_multiplier = buff_multiplier,
                                parent_position_x = parent_position.x,
                                parent_position_y = parent_position.y,
                                offspring_position_x = wrapped.x,
                                offspring_position_y = wrapped.y,
                                population = current_population + 1,
                                "🎉 Organism successfully reproduced!"
                            );

                            self.organism_positions.insert(wrapped, offspring_id);
                            self.organisms.insert(offspring_id, offspring);
                            self.reproduction_successes += 1;
                            self.total_offspring_born += 1;
                            offspring_spawned = true;

                            // Record metrics
                            event!(
                                Level::INFO,
                                counter_name = "reproductions_successful",
                                counter_value = 1,
                                parent_birth_tick = parent_birth_tick,
                                buff_applied = parent_birth_tick > 1,
                                "Reproduction success metric"
                            );

                            break;
                        }
                    }

                    if !offspring_spawned {
                        trace!(
                            organism_id = ?id,
                            tick = self.tick,
                            position_x = parent_position.x,
                            position_y = parent_position.y,
                            "Reproduction failed: no empty adjacent cell found"
                        );

                        event!(
                            Level::DEBUG,
                            counter_name = "reproduction_failures",
                            counter_value = 1,
                            failure_reason = "no_empty_adjacent_cell",
                            organism_id = ?id,
                            tick = self.tick,
                            "Reproduction attempt failed"
                        );
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
            let is_hazard = {
                let grid = self.grid.read();
                grid.get(*pos).tile_type == TileType::Hazard
            };

            if is_hazard {
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

                // Log death with comprehensive details
                let was_born_after_tick_1 = organism.birth_tick > 1;
                let lifetime = self.tick - organism.birth_tick;

                if was_born_after_tick_1 {
                    // Extra detailed logging for organisms born after tick 1
                    info!(
                        event = "organism_death",
                        organism_id = ?id,
                        lineage_id = ?organism.lineage_id,
                        tick = self.tick,
                        birth_tick = organism.birth_tick,
                        lifetime = lifetime,
                        age = organism.age,
                        final_energy = organism.energy,
                        offspring_count = organism.metrics.offspring_count,
                        kills = organism.metrics.kills,
                        tiles_explored = organism.metrics.tiles_explored,
                        times_eaten = organism.metrics.times_eaten,
                        damage_dealt = organism.metrics.damage_dealt,
                        damage_received = organism.metrics.damage_received,
                        born_after_tick_1 = true,
                        "💀 Organism born after tick 1 died"
                    );
                } else {
                    debug!(
                        event = "organism_death",
                        organism_id = ?id,
                        tick = self.tick,
                        birth_tick = organism.birth_tick,
                        age = organism.age,
                        offspring_count = organism.metrics.offspring_count,
                        "Organism died"
                    );
                }

                // Track death metrics
                event!(
                    Level::INFO,
                    counter_name = "organism_deaths",
                    counter_value = 1,
                    born_after_tick_1 = was_born_after_tick_1,
                    had_offspring = organism.metrics.offspring_count > 0,
                    "Organism death metric"
                );
            }
        }
    }

    fn spawn_organism(&mut self, lineage_id: LineageId, genome: Program) -> Result<()> {
        // Find random empty position
        let (width, height) = {
            let grid = self.grid.read();
            (grid.width, grid.height)
        };

        for _ in 0..100 {
            let x = self.rng.gen_range(0..width);
            let y = self.rng.gen_range(0..height);
            let pos = Position::new(x, y);

            if !self.organism_positions.contains_key(&pos) {
                let tile_type = {
                    let grid = self.grid.read();
                    grid.get(pos).tile_type
                };

                if tile_type != TileType::Obstacle {
                    let organism = Organism::new_with_birth_tick(
                        lineage_id,
                        pos,
                        self.config.energy_config.initial_energy,
                        genome,
                        self.tick,
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
