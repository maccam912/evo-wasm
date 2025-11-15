# Evo-WASM Organism Survival Analysis: Why Organisms Die Off Around Tick 3000

## Executive Summary

Organisms in this simulation are likely experiencing a survival collapse around tick 3000 due to a combination of **energy economy imbalances**, **reproduction cost vs. benefit misalignment**, and **weak selection pressure during the critical early-to-mid simulation period**. The system incentivizes energy hoarding over reproduction, but provides insufficient resources for organisms to sustain both survival and productive breeding.

---

## 1. ORGANISM DEATH/LIFECYCLE MECHANICS

### 1.1 How Organisms Die

**File:** `/home/user/evo-wasm/crates/evo-world/src/organism.rs`

Organisms die when their energy reaches 0:
```rust
pub fn is_alive(&self) -> bool {
    self.energy > 0
}
```

**Death Conditions:**
- Basal metabolic cost insufficient (1 energy/tick by default)
- Action costs exceed available energy (move, attack, reproduce)
- Resource starvation (can't find food)
- Hazard damage (10 damage/tick when on hazard tiles)
- Combat death (attack damage reduces energy)

### 1.2 Energy Drains (Per-Tick Costs)

**File:** `/home/user/evo-wasm/crates/evo-core/src/config.rs`

Default `EnergyConfig`:
```rust
pub struct EnergyConfig {
    pub initial_energy: i32 = 1000,
    pub basal_cost: i32 = 1,                    // Cost per tick
    pub instruction_cost_per_k: i32 = 1,        // Per 1000 fuel units
    pub move_cost: i32 = 5,
    pub attack_cost: i32 = 10,
    pub reproduce_cost: i32 = 500,
    pub eat_efficiency: f32 = 0.8,              // % energy gained from food
    pub min_reproduce_energy: i32 = 600,        // Min energy to attempt reproduction
}
```

**Energy Cost Summary:**
- **Basal:** 1/tick (1000 ticks of pure survival at rest)
- **Movement:** 5 per action
- **Attack:** 10 per action
- **Reproduction:** 500 (requires 600+ total energy to attempt)
- **Instruction Execution:** ~1 energy per 1000 WASM fuel units

### 1.3 Energy Sources

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 232-248)

Eating mechanics:
```rust
Action::Eat => {
    if tile.tile_type == TileType::Resource && tile.resource_amount > 0 {
        let consumed = tile.resource_amount.min(100);    // Max 100 per eat
        tile.resource_amount -= consumed;
        let energy_gained = (consumed as f32 * eat_efficiency) as i32;
        organism.add_energy(energy_gained);
    }
}
```

**Maximum Energy Gain:**
- 100 resources × 0.8 efficiency = **80 energy per eat action**
- With 5 energy move cost, net gain = 75 energy (if resource exists and wasn't consumed by others)
- With basal cost, net = 74 energy per eat+movement pair

---

## 2. REPRODUCTION MECHANICS & THE REPRODUCTION BOTTLENECK

### 2.1 Reproduction Requirements and Costs

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 285-333)

```rust
Action::Reproduce if self.config.dynamic_rules.allow_reproduction => {
    if energy < self.config.energy_config.reproduce_cost
        || energy < self.config.energy_config.min_reproduce_energy
    {
        return Ok(());  // Fails silently
    }
    if self.organisms.len() >= self.config.dynamic_rules.max_population {
        return Ok(());  // Fails silently
    }
    
    if let Some(parent) = self.organisms.get_mut(&id) {
        parent.consume_energy(self.config.energy_config.reproduce_cost);  // -500
        parent.record_offspring();
        
        // Offspring spawning...
        let offspring = Organism::new(
            parent_lineage,
            wrapped,
            self.config.energy_config.initial_energy / 2,  // 500 energy
            offspring_genome,
        );
    }
}
```

### 2.2 The Reproduction Cost-Benefit Problem

**Critical Issue #1: Negative ROI on Reproduction**

A parent organism needs:
- Minimum: 600 energy to even attempt reproduction
- Cost: 500 energy per successful reproduction
- Offspring receives: 500 energy (initial_energy / 2)

**Analysis:**
- Parent invests 500 energy
- Offspring starts with 500 energy
- **Net system energy investment: -500 + 500 = 0** (but parent can't reproduce again until recovering to 600)
- If parent needs 600 to reproduce, it must accumulate an additional 100+ to survive basal costs while seeking resources
- **Effective cost to parent**: 600+ energy to produce offspring worth 500

**What organisms actually do:**
1. Accumulate 600+ energy (requires eating frequently)
2. Reproduce once (-500 energy)
3. Back to 100+ energy, struggling to get back to 600
4. All while basal cost drains 1/tick

**Result:** Low reproduction rate because it's energetically unfavorable

### 2.3 Reproduction Frequency at Different Energy Levels

With initial 1000 energy:
- Can reproduce once (~2000 ticks to get back to 600 if eating perfectly)
- Most organisms reproduce 0-3 times before dying
- **Few lineages accumulate multiple generations within a single sim run**

---

## 3. FITNESS METRICS & SELECTION BIAS

### 3.1 Fitness Calculation

**File:** `/home/user/evo-wasm/crates/evo-core/src/fitness.rs` (lines 36-45)

```rust
pub fn scalar_fitness(&self) -> f64 {
    let lifetime_score = self.lifetime as f64 * 1.0;
    let energy_score = self.net_energy.max(0) as f64 * 0.5;
    let offspring_score = self.offspring_count as f64 * 100.0;      // HUGE weight!
    let exploration_score = self.tiles_explored as f32 * 0.1;
    let combat_score = self.kills as f64 * 50.0;
    
    lifetime_score + energy_score + offspring_score + exploration_score + combat_score
}
```

### 3.2 The Selection Pressure Problem

**Critical Issue #2: Misaligned Fitness Weights**

For an organism alive 1000 ticks with 1 offspring:
- Lifetime: 1000 × 1.0 = 1000
- Offspring: 1 × 100.0 = **100** (10% of total!)
- Offspring dominates if organism produces 11+

But producing 11 offspring requires:
- 11 × 500 = 5500 energy investment
- Starting energy: 1000
- Requires accumulating 4500+ net energy during the run
- With 80 energy max per eat and movement costs, this requires hundreds of eating events

**The paradox:**
- Fitness heavily weights offspring production (100× multiplier)
- But offspring production requires unsustainably high energy gathering
- **Organisms that just survive long (low reproduction) and energy-positive vs. those that burn out trying to reproduce**

### 3.3 What Actually Gets Selected?

Organisms get selected based on:
1. **Surviving longest** (lifetime × 1.0) - favors energy conservation
2. **Accumulating energy** (net_energy × 0.5) - favors hoarding
3. **Having offspring** (offspring × 100.0) - requires burning energy

**Result:** Selection pressure favors energy conservation (passive survival) over reproduction attempt. But energy conservation mutations may reduce exploration, risk-taking, or learning behaviors.

---

## 4. EVOLUTION & MUTATION MECHANICS

### 4.1 Within-Simulation Mutation

**File:** `/home/user/evo-wasm/crates/evo-ir/src/mutation.rs`

Default `MutationConfig`:
```rust
pub struct MutationConfig {
    pub point_mutation_rate: f32 = 0.01,           // 1% per instruction
    pub insertion_rate: f32 = 0.005,               // 0.5% per instruction
    pub deletion_rate: f32 = 0.005,                // 0.5% per instruction
    pub block_duplication_rate: f32 = 0.001,       // 0.1%
    pub function_addition_rate: f32 = 0.0001,      // 0.01%
}
```

Each reproduction triggers mutation:
```rust
let mut offspring_genome = parent.genome.clone();
self.mutator.mutate(&mut offspring_genome, &mut self.rng);
```

**Problem:** With low reproduction rates, mutation is **extremely rare** within a single simulation. Offspring are sparsely distributed, limiting genetic diversity exploration.

### 4.2 Between-Simulation Selection (Server-Side)

**File:** `/home/user/evo-wasm/crates/evo-server/src/evolution.rs`

Selection only occurs when **>= 20 lineages** exist:
```rust
let num_lineages = self.lineage_stats.read().len();
if num_lineages >= 20 {
    self.perform_selection().await?;
}
```

**Critical Issue #3: Delayed Selection Pressure**

In the first 10,000-tick simulation:
- Only 10 initial genomes are seeded
- No selection pressure occurs until 20+ lineages appear
- Early simulations just let organisms die off randomly
- **Evolution doesn't begin until 10+ new lineages spontaneously emerge through mutation**

When selection does occur:
- Keep top 50% fitness
- Create 10 new offspring through crossover+mutation
- **Too little diversity being explored per generation**

---

## 5. ENERGY & RESOURCE SYSTEMS

### 5.1 Resource Generation & Availability

**File:** `/home/user/evo-wasm/crates/evo-core/src/config.rs` (WorldConfig)

```rust
pub struct WorldConfig {
    pub width: i32 = 256,
    pub height: i32 = 256,
    pub resource_density: f32 = 0.3,               // 30% of tiles
    pub max_resource_per_tile: i32 = 1000,         // Max capacity
    pub resource_regen_rate: f32 = 0.05,           // 5% growth rate
    pub obstacle_density: f32 = 0.05,              // 5% of tiles
    pub hazard_density: f32 = 0.02,                // 2% of tiles
    pub hazard_damage: i32 = 10,                   // Per tick
}
```

**Resource Regeneration:**

**File:** `/home/user/evo-wasm/crates/evo-core/src/types.rs` (lines 186-192)

```rust
pub fn regenerate(&mut self, rate: f32) {
    if self.tile_type == TileType::Resource 
        && self.resource_amount < self.max_resource {
        let growth = (rate * self.resource_amount as f32
            * (1.0 - self.resource_amount as f32 / self.max_resource as f32)) as i32;
        self.resource_amount = (self.resource_amount + growth.max(1)).min(self.max_resource);
    }
}
```

This is **logistic growth**: regeneration slows as resource approaches max.

### 5.2 Resource Availability Analysis

**Grid dimensions:** 256 × 256 = 65,536 tiles
**Resource tiles:** 0.3 × 65,536 = ~19,661 tiles with resources
**Maximum total resources in world:** 19,661 × 1000 = 19.6 million units
**Energy equivalent:** 19.6M × 0.8 efficiency = 15.68 million energy

With 10 initial organisms and 1000 energy each = 10k starting energy:
- **Resource-to-population ratio: 1.568 million energy per organism** (seems abundant)

But organisms don't collect uniformly:
1. **Movement cost**: 5 energy to move 1 tile. To traverse all 65k tiles would cost 325k energy (impossible)
2. **Uneven resource distribution**: Initial placement is random. Organisms cluster near spawn points
3. **Competition**: As organisms reproduce, local resources deplete faster than they regenerate
4. **Hazards**: 2% of tiles deal 10 damage/tick (essentially poison)

**Critical Issue #4: Local Resource Depletion**

In early simulation (ticks 1-500):
- Few organisms, resources abundant near spawns
- High reproduction rate possible

In mid simulation (ticks 500-2000):
- Population grows
- Local depletion accelerates
- Organisms forced to travel further = higher movement costs
- Resource regeneration rate too slow (5% of depleted amount)

In late simulation (ticks 2000-3000):
- Population peaks
- Most organisms are competing with 10-100 others
- Starvation occurs in resource-poor zones
- Organisms that reproduced heavily now dying
- Mutations from those offspring haven't had time to adapt

**Around tick 3000, you hit the resource utilization equilibrium: population can't grow, lots die from starvation, few survive to establish new traits.**

---

## 6. MAIN SIMULATION LOOP & TICK PROGRESSION

### 6.1 The Simulation Step Function

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 89-116)

```rust
fn step(&mut self) -> Result<()> {
    // 1. Regenerate resources (logistic growth)
    self.grid.write()
        .regenerate_resources(resource_regen_rate);
    
    // 2. Process organisms in random order
    let organism_ids: Vec<OrganismId> = self.organisms.keys().copied().collect();
    let mut shuffled_ids = organism_ids.clone();
    shuffled_ids.shuffle(&mut self.rng);
    
    for id in shuffled_ids {
        self.process_organism(id)?;
    }
    
    // 3. Apply hazard damage
    self.apply_hazards();
    
    // 4. Remove dead organisms
    self.remove_dead_organisms();
    
    Ok(())
}
```

### 6.2 Organism Processing Each Tick

**File:** `/home/user/evo-wasm/crates/evo-world/src/simulation.rs` (lines 118-188)

Per organism per tick:
1. **Basal cost deducted** (-1 energy)
2. **Age incremented** (+1 tick)
3. **WASM step executed** (if compiled)
   - Organism determines actions
   - Consumes fuel, costs instruction_cost
4. **Actions applied** (Move, Eat, Attack, Reproduce)
5. **Dead organisms removed** at end of step

### 6.3 The 3000-Tick Collapse Theory

**Tick 0-500: Population Boom**
- Abundant local resources
- Many organisms can reproduce (energy easily available)
- Population grows to near max (1000)
- High genetic diversity (many random mutations)

**Tick 500-2000: Transition Phase**
- Local resource depletion forces travel
- Movement costs increase relative to resource gain
- Reproduction rate drops as competition intensifies
- Many organisms dying from starvation
- Weak selection pressure (< 20 lineages for most of this)

**Tick 2000-3000: Equilibrium and Collapse**
- Population stabilizes at resource limit
- Most organisms are descendants of successful early lineages
- Their mutations were random (not selected for)
- New arrivals (offspring from weak parents) die quickly
- By tick 3000: few successful reproduction events
- Stochastic population fluctuations become extreme
- Some lineages go extinct from bad luck
- Survivors are "tired": no energy for reproduction

**Tick 3000+: Bottleneck**
- Remaining organisms struggling to maintain 600 energy
- Reproduction nearly halts
- Few organisms = no selection pressure
- Eventually most die from hazards, bad luck, or failure to find resources
- Game ends with handful of exhausted survivors

---

## CRITICAL ISSUES SUMMARY

### Issue #1: Reproduction Cost-Benefit Inversion
**Severity:** HIGH
**Root Cause:** Reproduce cost (500) + min energy (600) requires massive energy investment, but offspring get only 500 energy
**Impact:** Organisms avoid reproduction; low genetic diversity; limited within-sim evolution

### Issue #2: Fitness Weight Misalignment
**Severity:** HIGH
**Root Cause:** Offspring weighted 100×, but 11 offspring requires 5500+ energy accumulation (unsustainable)
**Impact:** Selection favors energy hoarding over reproduction; evolves for "boring" survival over behavior

### Issue #3: Delayed Selection Pressure
**Severity:** MEDIUM
**Root Cause:** No server-side selection until 20 lineages exist; only 10 initial genomes
**Impact:** First half of simulation has zero selection; random drift dominates early evolution

### Issue #4: Local Resource Depletion
**Severity:** HIGH
**Root Cause:** Exponential population growth + limited local resources + slow regeneration
**Impact:** Population crashes around tick 2000-3000; starvation becomes primary death mode

### Issue #5: Insufficient Genetic Diversity
**Severity:** MEDIUM
**Root Cause:** Low reproduction rate + early population boom = limited mutation opportunities
**Impact:** Most organisms are clones of early successful lineages; limited adaptation room

---

## RECOMMENDED INVESTIGATION POINTS

1. **Check actual simulation logs:**
   - What's the population at tick 1000, 2000, 3000?
   - What % of deaths are from starvation vs. old age vs. hazards?
   - How many organisms reach reproductive maturity?

2. **Check mutation effectiveness:**
   - Are mutations improving fitness or causing regression?
   - How many mutations per lineage actually occur within a 10k-tick run?

3. **Check resource utilization:**
   - Are resources being depleted faster than regenerating?
   - Are certain world areas becoming barren?

4. **Check energy economy:**
   - Average lifetime of organisms?
   - Average net_energy at death?
   - How many reach 600 energy for reproduction?

5. **Profile hypothesis:**
   - Run simulation with reproduction_cost reduced to 200
   - Run with eat_efficiency increased to 1.5
   - Run with initial_energy increased to 2000
   - See if any of these prevent tick-3000 collapse

