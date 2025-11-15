# Quick Reference: Organism Mechanics & Code Locations

## Energy Economy Flow

```
ORGANISM START: 1000 energy
                    |
      +---------+---+---+---------+
      |         |       |         |
    BASAL      MOVE   ATTACK   INSTRUCT
     (-1)      (-5)    (-10)    (varies)
      |         |       |         |
      +---> ACTION_COST_BUDGET <--+
            (per tick)
            
                    |
      +---------+--------+
      |                  |
   SURVIVE          FAIL (die)
   (+0 net if                  
    no food)
      |
      | EAT (-100 resources = +80 energy)
      |    Cost to reach food: MOVE (-5)
      |    Net per eat: +75 energy
      |
      +---> 600 ENERGY
      |     THRESHOLD
      |
      +---> REPRODUCE (-500, offspring +500)
            Back to ~100 energy
            Need to accumulate to 600 again
```

**Key Files:**
- Death check: `/home/user/evo-wasm/crates/evo-world/src/organism.rs:46-48`
- Energy drain: `/home/user/evo-wasm/crates/evo-core/src/config.rs:62-75`
- Consumption logic: `/home/user/evo-wasm/crates/evo-world/src/organism.rs:54-62`

---

## Population Dynamics Over Time

```
TICK 0-500: BOOM PHASE
  ├─ 10 organisms spawn
  ├─ Abundant local resources
  ├─ Reproduction: 5-15 per organism possible
  ├─ Population growth: linear/exponential → 1000 max
  └─ Genetic diversity: HIGH (random mutations on every offspring)

TICK 500-2000: TRANSITION PHASE  
  ├─ Population cap reached (1000 organisms)
  ├─ Local resource depletion accelerates
  ├─ Reproduction rate: drops sharply
  ├─ Competition for food: intense
  ├─ Death rate: rising (starvation)
  └─ No selection pressure yet! (<20 lineages)

TICK 2000-3000: EQUILIBRIUM/COLLAPSE
  ├─ Population: steady state or declining
  ├─ Survivors: mostly first-gen mutants
  ├─ Reproduction: nearly halted
  ├─ Resource regeneration: SLOWER than consumption
  ├─ Fitness stagnation: weak organisms cloned
  └─ Stochastic extinction: bad luck kills lineages

TICK 3000+: BOTTLENECK
  ├─ Few organisms left (10-100)
  ├─ Extremely high competition
  ├─ Hazards: now lethal (10/tick on 2% of tiles)
  └─ Result: Game over, evolution failed
```

**Key Files:**
- Main loop: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:69-87`
- Step function: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:89-116`
- Reproduction condition: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:285-294`

---

## Fitness Calculation (Why Selection Fails)

```
SCALAR_FITNESS = lifetime*1.0 + net_energy*0.5 + offspring*100 + tiles*0.1 + kills*50

Example Organism (lived 1000 ticks):
├─ lifetime: 1000 × 1.0 = 1000
├─ net_energy: +100 × 0.5 = 50
├─ offspring: 2 × 100 = 200
├─ tiles_explored: 50 × 0.1 = 5
└─ kills: 0 × 50 = 0
    TOTAL = 1255

To beat this with JUST offspring:
├─ Need 11+ offspring (11 × 100 = 1100)
├─ Cost: 11 × 500 = 5500 energy to produce
├─ But starting energy: 1000
├─ Net gain needed: 4500 energy
├─ With max 80 energy/eat + 5 cost/move
└─ IMPOSSIBLE to achieve in 1000 ticks

RESULT: Selection pressure → energy hoarding, not reproduction
        Evolved organisms: cautious, passive, energy-conservative
        Problem: zero new behaviors, zero fitness improvement
```

**Key File:**
- Fitness calc: `/home/user/evo-wasm/crates/evo-core/src/fitness.rs:36-45`
- Selection: `/home/user/evo-wasm/crates/evo-server/src/evolution.rs:139-207`

---

## Resource Regeneration (Logistic Model)

```
TILE STATE:
resource_amount: current (0-1000)
max_resource: 1000

REGENERATION per tick:
growth = rate × resource_amount × (1 - resource_amount/max)
       = 0.05 × amount × (1 - amount/1000)

EXAMPLES:
├─ amount=500 (half full): growth = 0.05 × 500 × 0.5 = 12.5 → 1 unit/tick
├─ amount=100 (depleted): growth = 0.05 × 100 × 0.9 = 4.5 → 1 unit/tick  
└─ amount=900 (almost full): growth = 0.05 × 900 × 0.1 = 4.5 → 1 unit/tick

CRITICAL: Depleted tiles regen SLOWLY (≤1/tick)
├─ To go from 0→100: 100+ ticks if no eating
├─ Organisms consume 100/eat: faster than regen
└─ LOCAL DEPLETION ZONES form and persist

GRID STATS:
├─ Width/Height: 256 × 256 = 65,536 tiles
├─ Resource density: 30% = ~19,661 resource tiles
├─ With 1000 max per tile: 19.6M total resources
├─ With 10 organisms: 1.96M per organism theoretically
└─ BUT: organisms cluster, can't traverse whole grid (too expensive)
        → Effective resources available: ~100k per organism
        → Easily depleted by population of 1000
```

**Key Files:**
- Regeneration: `/home/user/evo-wasm/crates/evo-core/src/types.rs:186-192`
- Grid config: `/home/user/evo-wasm/crates/evo-core/src/config.rs:26-39`
- Eating: `/home/user/evo-wasm/crates/evo-world/src/simulation.rs:232-248`

---

## Mutation: Within-Sim vs. Between-Sim

### Within-Simulation (per offspring)

**Files:** `/home/user/evo-wasm/crates/evo-ir/src/mutation.rs`

```
point_mutation_rate:        0.01  (1% per instruction)
insertion_rate:             0.005 (0.5% per instruction)
deletion_rate:              0.005 (0.5% per instruction)
block_duplication_rate:     0.001 (0.1%)
function_addition_rate:     0.0001(0.01%)

PROBLEM: With 5 offspring total in 10k ticks
├─ Mutation events: ~5 genomes × very low rates
├─ Expected mutations per genome: 0-2
└─ Genetic diversity explored: MINIMAL

WORSE: No selection pressure on these mutations!
       They're random → likely deleterious
       Failed to explore effective mutation space
```

### Between-Simulation (server-side)

**Files:** `/home/user/evo-wasm/crates/evo-server/src/evolution.rs`

```
TRIGGER: "if num_lineages >= 20"
         └─ Only 10 initial genomes provided
            Need 10+ spontaneous lineages from random mutation
            First sim has ZERO selection!

WHEN triggered:
├─ Keep top 50% by fitness
├─ Create 10 offspring via crossover+mutation
├─ Store new lineages
└─ Next sim seeds from these

PROBLEM: 
├─ Delays real evolution by 1000s of ticks
├─ "Top 50%" of random mutations ≠ "good solutions"
├─ Only 10 new genomes per generation
└─ Evolution is glacially slow
```

---

## The Collapse Timeline

```
TICK        POPULATION   REPRODUCTION   RESOURCES     STATUS
─────────────────────────────────────────────────────────────
0           10           High           Full          STARTING
100         50           High           Depleting     BOOM
500         900          High→Medium    Local gaps    PEAK
1000        1000         Medium         Equilibrium   TRANSITION
2000        800-1000     Low→Stop       Scarce        STRESS
3000        100-300      Stop           Critical      COLLAPSE
4000        10-50        None           N/A           ENDGAME
5000+       0-5          None           N/A           EXTINCT
```

**Why 3000?**
- Population boom (0-500) saturates resources
- Population peak (500-2000) exhausts regeneration capacity
- Around 2000-3000: starvation begins
- By 3000: population crash is inevitable
- Remaining organisms: too few to evolve, too weak to recover

---

## FIVE CRITICAL KNOBS TO ADJUST

If you want to fix the tick-3000 collapse:

1. **reproduction_cost: 500 → 200**
   └─ Organisms can reproduce 2-3× more often
   └─ More mutations tested, faster evolution

2. **min_reproduce_energy: 600 → 300**
   └─ Easier to trigger reproduction
   └─ Smaller gap to recover from cost

3. **eat_efficiency: 0.8 → 1.5**
   └─ More energy per food
   └─ Reproduction becomes sustainable

4. **initial_energy: 1000 → 2000**
   └─ Longer survival window
   └─ More time for adaptation

5. **resource_regen_rate: 0.05 → 0.15**
   └─ Resources regenerate faster
   └─ Less local depletion
   └─ Sustains larger population

**ALSO: Consider starting with 50+ initial organisms, not 10**
└─ More genetic diversity from day 1
└─ Faster competition for selection

