# Complete Code Reference Guide

## File Locations & Key Mechanics

### 1. ORGANISM LIFECYCLE & DEATH

**File:** `/home/user/evo-wasm/crates/evo-world/src/organism.rs`

| Mechanic | Lines | Code |
|----------|-------|------|
| **Death check** | 46-48 | `pub fn is_alive(&self) -> bool { self.energy > 0 }` |
| **Energy consumption** | 54-62 | `consume_energy()` - dies if tries to spend more than available |
| **Basal cost applied** | 128-130 | `organism.consume_energy(basal_cost)` per tick in process_organism |
| **Age increment** | 70-73 | `organism.tick()` increments age each step |
| **Metrics finalization** | 92-94 | `finalize_metrics()` called when organism dies |

---

### 2. ENERGY CONFIGURATION (ALL COSTS DEFINED HERE)

**File:** `/home/user/evo-wasm/crates/evo-core/src/config.rs` (lines 41-75)

```rust
pub struct EnergyConfig {
    pub initial_energy: i32 = 1000,              // Starting energy
    pub basal_cost: i32 = 1,                     // Cost per tick
    pub instruction_cost_per_k: i32 = 1,         // Per 1000 fuel
    pub move_cost: i32 = 5,                      // Per movement
    pub attack_cost: i32 = 10,                   // Per attack
    pub reproduce_cost: i32 = 500,               // Per offspring
    pub eat_efficiency: f32 = 0.8,               // % energy from food
    pub min_reproduce_energy: i32 = 600,         // Minimum to attempt
}
```

**WHERE USED:**
- Basal cost: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:128`
- Move cost: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:201-227`
- Attack cost: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:250-283`
- Reproduce cost: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:285-333`

---

### 3. REPRODUCTION MECHANICS

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 285-333)

```rust
Action::Reproduce if self.config.dynamic_rules.allow_reproduction => {
    // Line 286-288: Check energy requirements
    if energy < self.config.energy_config.reproduce_cost
        || energy < self.config.energy_config.min_reproduce_energy
    {
        return Ok(());  // Fails silently
    }
    
    // Line 292: Population cap check
    if self.organisms.len() >= self.config.dynamic_rules.max_population {
        return Ok(());
    }
    
    // Line 297-298: Parent loses energy
    parent.consume_energy(reproduce_cost);  // -500 energy
    parent.record_offspring();
    
    // Line 320-325: Offspring spawned with HALF initial energy
    let offspring = Organism::new(
        parent_lineage,
        wrapped,
        self.config.energy_config.initial_energy / 2,  // 500 energy
        offspring_genome,
    );
}
```

**KEY ISSUE:** Minimum reproduction energy (600) requires massive accumulation for low reproduction benefit.

---

### 4. FITNESS METRICS & CALCULATION

**File:** `/home/user/evo-wasm/crates/evo-core/src/fitness.rs` (lines 8-45)

```rust
pub struct FitnessMetrics {
    pub lifetime: u64,                // Time survived
    pub net_energy: i64,              // Energy at death - initial energy
    pub offspring_count: u32,         // Number produced
    pub tiles_explored: u32,          // Distinct tiles visited
    pub kills: u32,                   // Successful attacks
    pub times_eaten: u32,             // Times food consumed
    pub damage_dealt: i64,            // Total damage dealt
    pub damage_received: i64,         // Total damage taken
}

pub fn scalar_fitness(&self) -> f64 {
    let lifetime_score = self.lifetime as f64 * 1.0;
    let energy_score = self.net_energy.max(0) as f64 * 0.5;
    let offspring_score = self.offspring_count as f64 * 100.0;  // HUGE!
    let exploration_score = self.tiles_explored as f64 * 0.1;
    let combat_score = self.kills as f64 * 50.0;
    
    lifetime_score + energy_score + offspring_score + exploration_score + combat_score
}
```

**THE PROBLEM:** Offspring weighted 100x, but producing 11+ is impossible with available energy.

**WHERE UPDATED:**
- on_tick: `/home/user/evo-wasm/crates/evo-world/src/organism.rs:70-73`
- on_move: `/home/user/evo-wasm/crates/evo-world/src/organism.rs:64-68`
- on_eat: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:245`
- on_offspring: `/home/user/evo-wasm/crates/evo-world/src/organism.rs:87-89`
- finalize: `/home/user/evo-wasm/crates/evo-world/src/organism.rs:92-94`

---

### 5. RESOURCE & EATING MECHANICS

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 232-248)

```rust
Action::Eat => {
    if let Some(organism) = self.organisms.get_mut(&id) {
        let organism_pos = organism.position;
        let mut grid = self.grid.write();
        let tile = grid.get_mut(organism_pos);
        
        if tile.tile_type == TileType::Resource && tile.resource_amount > 0 {
            let consumed = tile.resource_amount.min(100);  // Max 100 per eat
            tile.resource_amount -= consumed;
            
            let energy_gained = (consumed as f32 
                * self.config.energy_config.eat_efficiency) as i32;  // 0.8x = 80 max
            organism.add_energy(energy_gained);
        }
    }
}
```

**Max gain:** 100 resources × 0.8 efficiency = 80 energy per eat action

**Resource Regeneration:** `/home/user/evo-wasm/crates/evo-core/src/types.rs:186-192`

```rust
pub fn regenerate(&mut self, rate: f32) {
    if self.tile_type == TileType::Resource && self.resource_amount < self.max_resource {
        let growth = (rate * self.resource_amount as f32
            * (1.0 - self.resource_amount as f32 / self.max_resource as f32)) as i32;
        self.resource_amount = (self.resource_amount + growth.max(1)).min(self.max_resource);
    }
}
```

**Growth formula:** `0.05 × amount × (1 - amount/1000)` = logistic (slow when depleted)

---

### 6. WORLD CONFIGURATION

**File:** `/home/user/evo-wasm/crates/evo-core/src/config.rs` (lines 6-39)

```rust
pub struct WorldConfig {
    pub width: i32 = 256,
    pub height: i32 = 256,
    pub resource_density: f32 = 0.3,             // 30% resource tiles
    pub max_resource_per_tile: i32 = 1000,       // Max capacity
    pub resource_regen_rate: f32 = 0.05,         // 5% logistic rate
    pub obstacle_density: f32 = 0.05,            // 5% obstacles
    pub hazard_density: f32 = 0.02,              // 2% hazards
    pub hazard_damage: i32 = 10,                 // Damage per tick on hazard
}
```

**GRID MATH:**
- Total tiles: 256 × 256 = 65,536
- Resource tiles: ~19,661 (30%)
- Max total resources: 19.6M units = 15.68M energy equivalent
- Per organism (10 initial): ~1.56M each theoretically
- But organisms cluster → actual effective resources ~100k each

---

### 7. MUTATION RATES (WITHIN-SIM)

**File:** `/home/user/evo-wasm/crates/evo-ir/src/mutation.rs` (lines 9-42)

```rust
pub struct MutationConfig {
    pub point_mutation_rate: f32 = 0.01,         // 1% per instruction
    pub insertion_rate: f32 = 0.005,             // 0.5% per instruction
    pub deletion_rate: f32 = 0.005,              // 0.5% per instruction
    pub block_duplication_rate: f32 = 0.001,     // 0.1% per block
    pub function_addition_rate: f32 = 0.0001,    // 0.01% per function
    pub max_instructions_per_function: usize = 100,
    pub max_functions: usize = 10,
}
```

**CALLED ON:** Each reproduction (lines 304-306)
```rust
let mut offspring_genome = parent.genome.clone();
self.mutator.mutate(&mut offspring_genome, &mut self.rng);
```

**PROBLEM:** With only 5 offspring in 10k ticks, mutations are RARE and random.

---

### 8. SERVER-SIDE SELECTION (BETWEEN-SIM)

**File:** `/home/user/evo-wasm/crates/evo-server/src/evolution.rs`

**Selection trigger (line 82):**
```rust
let num_lineages = self.lineage_stats.read().len();
if num_lineages >= 20 {
    self.perform_selection().await?;  // Only 10 initial = zero selection until later
}
```

**Selection logic (lines 139-207):**
```rust
async fn perform_selection(&self) -> Result<()> {
    // Line 146-157: Sort by fitness (weak multi-objective)
    let mut lineages: Vec<(LineageId, &LineageStats)> = stats.iter()
        .map(|(id, stat)| (*id, stat))
        .collect();
    
    lineages.sort_by(|a, b| {
        b.1.best_fitness.scalar_fitness()
            .partial_cmp(&a.1.best_fitness.scalar_fitness())
            .unwrap()
    });
    
    // Line 160-165: Keep top 50%
    let keep_count = lineages.len() / 2;
    let survivors: Vec<LineageId> = lineages
        .iter()
        .take(keep_count)
        .map(|(id, _)| *id)
        .collect();
    
    // Line 175-204: Create 10 offspring via crossover+mutation
    for _ in 0..offspring_count {
        // Select two random parents, crossover, mutate
    }
}
```

**CRITICAL:** No selection pressure in first 2000+ ticks of first simulation.

---

### 9. MAIN SIMULATION LOOP

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 69-116)

```rust
pub fn run(&mut self) -> Result<SimulationResult> {
    info!("Starting simulation for {} ticks", self.config.num_ticks);
    
    for tick in 0..self.config.num_ticks {  // Default: 10,000
        self.tick = tick;
        self.step()?;
        
        if tick % 1000 == 0 {
            info!("Tick {}/{}: {} organisms alive", 
                tick, self.config.num_ticks, self.organisms.len());
        }
    }
    
    Ok(self.collect_results())
}

fn step(&mut self) -> Result<()> {
    // 1. Regenerate resources (line 93-95)
    self.grid.write()
        .regenerate_resources(self.config.world_config.resource_regen_rate);
    
    // 2. Process organisms in RANDOM order (line 97-107)
    let organism_ids: Vec<OrganismId> = self.organisms.keys().copied().collect();
    let mut shuffled_ids = organism_ids.clone();
    shuffled_ids.shuffle(&mut self.rng);
    
    for id in shuffled_ids {
        self.process_organism(id)?;  // Basal cost, action, mutations
    }
    
    // 3. Apply hazard damage (line 110)
    self.apply_hazards();  // 10 damage per tick
    
    // 4. Remove dead organisms (line 113)
    self.remove_dead_organisms();
    
    Ok(())
}
```

**PROCESS_ORGANISM (lines 118-188):**
```rust
fn process_organism(&mut self, id: OrganismId) -> Result<()> {
    // Line 128-130: BASAL COST (first thing, kills if insufficient)
    if !organism.consume_energy(basal_cost) {
        organism.finalize_metrics(initial_energy);
        return Ok(());  // Will be removed
    }
    
    organism.tick();  // Line 133: increment age
    
    // Line 136-161: Compile and execute WASM
    if organism.instance.is_none() {
        let wasm_bytes = self.compiler.compile(&organism.genome)?;
        // Create context, instantiate, init
    }
    
    // Line 164-171: Execute step, get actions
    let instance = organism.instance.as_mut().unwrap();
    let (_, actions) = instance.step(0)?;
    
    // Line 174-177: Apply instruction cost
    let fuel_used = instance.fuel_consumed();
    let instruction_cost = (fuel_used / 1000) as i32 * instruction_cost_per_k;
    organism.consume_energy(instruction_cost);
    
    // Line 183-185: Process actions (Move, Eat, Attack, Reproduce, EmitSignal)
    for action in actions {
        self.apply_action(id, action, ...)?;
    }
}
```

---

### 10. HAZARD DAMAGE

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 346-361)

```rust
fn apply_hazards(&mut self) {
    let hazard_damage = self.config.world_config.hazard_damage;  // 10
    
    for (pos, id) in self.organism_positions.iter() {
        let is_hazard = {
            let grid = self.grid.read();
            grid.get(*pos).tile_type == TileType::Hazard
        };
        
        if is_hazard {
            if let Some(organism) = self.organisms.get_mut(id) {
                organism.consume_energy(hazard_damage);  // -10 per tick
            }
        }
    }
}
```

**Problem:** 10 damage/tick on hazards, 2% of grid = hazardous zones are death zones.

---

### 11. POPULATION CAP

**File:** `/home/user/evo-wasm/crates/evo-core/src/config.rs` (line 141)

```rust
pub struct DynamicRules {
    pub max_population: usize = 1000,  // Hard cap
}
```

**ENFORCED AT:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:292-294`

```rust
if self.organisms.len() >= self.config.dynamic_rules.max_population {
    return Ok(());  // Reproduction attempt fails
}
```

---

## SUMMARY TABLE: WHERE TO MAKE CHANGES

| What to Fix | Where | Current | Suggested | Impact |
|-------------|-------|---------|-----------|--------|
| **Low reproduction rate** | `/home/user/evo-wasm/crates/evo-core/src/config.rs:70` | 500 | 200-300 | 2-3x more offspring |
| **High reproduction threshold** | `/home/user/evo-wasm/crates/evo-core/src/config.rs:72` | 600 | 300-400 | Easier to breed |
| **Low food value** | `/home/user/evo-wasm/crates/evo-core/src/config.rs:71` | 0.8 | 1.2-1.5 | More energy from food |
| **Low starting energy** | `/home/user/evo-wasm/crates/evo-core/src/config.rs:65` | 1000 | 2000 | Longer lifespan |
| **Slow resource regen** | `/home/user/evo-wasm/crates/evo-core/src/config.rs:17` | 0.05 | 0.15-0.20 | Less depletion |
| **Few initial organisms** | `/home/user/evo-wasm/crates/evo-server/src/evolution.rs:40` | 10 | 50-100 | More diversity |
| **Delayed selection** | `/home/user/evo-wasm/crates/evo-server/src/evolution.rs:82` | 20 | 10 | Earlier selection |
| **Unfair fitness weights** | `/home/user/evo-wasm/crates/evo-core/src/fitness.rs:40` | 100 | 10 | Less offspring bias |

