Here’s a concrete design you could actually build.

---

## 1. Core idea

* Each organism = a sandboxed WebAssembly module.
* The module gets:

  * Read-only **sensors** (local environment, neighbors, internal energy).
  * Read–write **actuators** (move, eat, attack, reproduce, emit signals).
* The world runs on **volunteer workers**:

  * A central server hands out “island jobs” (bundles of organisms + world config).
  * Workers run simulations, then report **lineage + performance stats** back.
  * Central server does **selection + global evolution** and issues new islands.

Think “Tierra / Avida ecology” + “Bibites world” + “Folding@home” job flow + wasm.

---

## 2. Organism design: WASM ABI

### 2.1 ABI surface

Each organism module must export a fixed set of functions, for example:

```text
export fn init(seed: u64)
export fn step(ctx_ptr: i32) -> i32   // returns action code
```

Imports from the host (the simulator runtime):

```text
// Sensors
import fn env_read(x: i32, y: i32) -> i32   // tile info
import fn sense_neighbor(slot: i32, buf: i32) -> i32
import fn get_energy() -> i32
import fn get_age() -> i32

// Actions (subject to host limits)
import fn move_dir(dx: i32, dy: i32) -> i32
import fn eat() -> i32
import fn attack(slot: i32, amount: i32) -> i32
import fn try_reproduce(payload_ptr: i32, len: i32) -> i32
import fn emit_signal(channel: i32, value: i32)
```

Rules:

* Fixed **cycle budget** per `step` (e.g. 10k wasm ops). Exceed → forced yield or kill.
* Fixed **memory limit** (e.g. 64 KiB / organism).
* No host imports beyond the ABI. Use wasmtime/wasmer with a custom, very narrow import object.

The host decides whether actions succeed, how much they cost, and resolves conflicts.

### 2.2 Internal organism state

The module treats its linear memory as “brain state”:

* Neural nets, heuristic tables, hand-rolled logic, whatever the genome encodes.
* The host passes a small “context struct” pointer into `step` that includes:

  * packed sensors,
  * a random seed,
  * maybe a pointer to some shared read-only tables.

This keeps the ABI small while allowing rich internal behavior.

---

## 3. Genome representation & mutation

“Raw wasm bytes + random flips” will mostly fail validation. You need a structure with validity-preserving operators.

### 3.1 Two-layer approach (recommended)

* **Genotype:** a custom IR describing an organism’s “program”.

  * Think: small typed instruction set (arithmetic, comparisons, load/store, conditional jump).
  * Structured basic blocks; explicit function signatures.
  * Stored as a compact binary format.
* **Phenotype:** wasm bytecode compiled from that IR.

  * Deterministic compiler from IR → wasm.
  * Compiler is *not* evolved; it’s part of the system.

Mutation & crossover happen on the IR:

* **Point mutation:** tweak opcodes, constants, or branch targets within type constraints.
* **Structural mutation:**

  * Add/remove a basic block.
  * Duplicate a function and mutate it.
  * Add a new local variable or state slot.
* **Crossover:**

  * Splice functions or blocks between two parents that share compatible signatures.
* **Constraints:**

  * Hard caps on instruction count, functions, locals to prevent code-bloat.
  * Penalize very large programs in fitness (energy upkeep, e.g. “brain cost”).

The server stores the **IR genome**. Workers receive IR → compile to wasm → simulate.

### 3.2 Reproduction pipeline

Two patterns, and you can mix them:

1. **In-world reproduction (ecological):**

   * Organism calls `try_reproduce(payload_ptr, len)`.
   * Payload = either:

     * a “mutation request” (host mutates its stored IR and spawns an offspring),
     * or parameter tweaks to a known template.
   * Host enforces mutation rate/caps.

2. **Meta-level evolution (server-driven):**

   * At the end of an island run, workers report back survivors + stats.
   * Server runs a selection algorithm (e.g. NSGA-II style multiobjective), breeds new IRs, and sends those as next generation seeds.

I’d do both:

* Local ecology drives short-term competition.
* Global selection steers long-term direction and prevents local traps.

---

## 4. World & resource model

### 4.1 World layout

* 2D toroidal grid (say 256×256 tiles per island run).
* Tile types:

  * Empty
  * Resource patches (regenerating “food”)
  * Obstacles / walls
  * Hazards (damage over time)

Time is discrete ticks.

### 4.2 Energy economy

* Each organism has **energy**.
* Costs:

  * Basal metabolic cost per tick.
  * Extra cost per instruction executed (encourages simple programs).
  * Actions: move, attack, reproduce all consume energy.
* Gains:

  * Eating resource tiles.
  * Possibly eating dead bodies or weaker organisms.
* Death:

  * Energy ≤ 0 → organism dies, maybe leaves a resource “corpse” on the tile.

Resources:

* Tiles have a max resource value Rmax.
* Regeneration by logistic-like growth, slow enough that **resource scarcity** is real.
* Initial seeding: central server defines density (sparse vs rich) per experiment.

### 4.3 Interaction & competition

* Max occupancy: one organism per tile.
* Combat:

  * `attack()` takes energy from attacker, can kill/damage a neighbor.
  * Host resolves attacks deterministically (e.g. ordered by some stable ID).
* Local information only:

  * Sensors show limited radius; no global view.
  * Encourages exploration, territorial strategies, swarming, etc.

---

## 5. Fitness and scoring

Rather than a single scalar, track a vector:

* Lifetime (ticks survived).
* Net energy gained.
* Total offspring produced (unique descendants).
* Area explored (distinct tiles visited).
* Interaction stats (kills, assists, cooperation metrics if you add them later).

For selection:

* Use multi-objective ranking (e.g. Pareto front) to keep diverse strategies.
* Optionally assign experiment-specific scalar fitness for particular challenges.

Workers compute these stats locally & send them to the server along with lineage IDs.

---

## 6. Distributed architecture

### 6.1 Roles

**Central coordinator (Rust / Go / Elixir / whatever):**

* Stores:

  * Genome bank: IR + metadata for each lineage.
  * Experiment definitions: world config, duration, resource density, etc.
* Responsibilities:

  * Create jobs:

    * Select a set of genomes for an “island”.
    * Generate seed world & RNG seeds.
  * Accept results from workers:

    * Summaries of each lineage’s performance.
    * Optional sampled genomes from survivors/newly spawned individuals.
  * Run global evolution:

    * Selection + crossover + mutation → next-gen genomes.
  * Assign genomes to new jobs.

**Volunteer worker (Docker container, binary, or web app via WASM):**

* Long-poll or websocket:

  * `GET /job` → receives:

    * world config & seed,
    * list of IR genomes + IDs,
    * run length (ticks).
* Compiles IR → wasm modules.
* Simulates world for N ticks:

  * Schedules organisms, enforces cycle limits.
  * Tracks fitness stats & reproduction events.
* Reports back:

  * Aggregate stats per lineage.
  * Optionally a sample of offspring IRs.
  * Run logs (hashes, checksums) for verification.

### 6.2 Fault tolerance & cheating resistance

* Jobs are **idempotent evaluations**:

  * Same genomes + same seeds → deterministic results.
  * The server can reassign a job if it’s not returned.
* Cheating mitigation:

  * Randomly re-evaluate a small fraction of jobs on trusted or multiple workers.
  * Compare stats; if a worker’s reports diverge consistently, ignore it.
* No obvious incentive to cheat:

  * Don’t make “top scorer” rankings tied directly to user identity in a way that rewards lying.
  * If you gamify it, base rewards on verifiable stats (e.g. only after cross-checked).

---

## 7. “Island” evolution flow

Per job:

1. Server picks K genomes from the global pool + some “baseline” ancestors.
2. Generates 2D world with specific resource parameters & seed.
3. Worker:

   * Places organisms randomly.
   * Runs for T ticks, letting them reproduce/mutate locally.
   * At the end, samples:

     * survivors’ genomes,
     * some of their descendants,
     * all stats.
4. Server:

   * Aggregates across many islands.
   * Selection:

     * Keep top X% by multi-objective ranking.
     * Keep a random Y% to maintain diversity.
   * Variation:

     * Run mutation/crossover to fill back to target population size.
   * Next generation:

     * Assign new genomes into new islands, possibly across different world configs (“niches”).

You get:

* **Ecological dynamics** inside each island.
* **Meta-evolution** across islands, guided by custom objectives.

---

## 8. Game / visualization layer

You can expose this as a “game” for players:

* Web dashboard:

  * Live view of a single island (2D grid with organisms colored by lineage).
  * Graphs:

    * Lineage tree over time.
    * Diversity indices.
    * Fitness distributions.
* User knobs:

  * Resource density.
  * World size.
  * Extra hazards (radiation zones that increase mutation rate, etc.).
  * “Scenario of the week” that everyone helps evaluate.

Players could run a worker in their browser (wasm-in-wasm) or via a desktop client.

---

## 9. Implementation sketch (tech choices)

* **Server:**

  * Rust + Axum / Go + chi / Elixir + Phoenix.
  * Persistence: Postgres for genomes & stats; object store for logs.
* **Worker:**

  * Rust with:

    * `wasmtime` for wasm execution.
    * Your own IR representation + compiler to wasm.
* **Protocol:**

  * JSON or MsgPack for job descriptions and result payloads.
  * HTTPS long-poll or websockets for job assignment.

---

## 10. Main open design choices / uncertainties

* Exact IR design:

  * You’ll need to iterate to find an instruction set that’s expressive but mutation-friendly and not too bloated.
* Performance:

  * Millions of wasm steps × thousands of organisms might demand careful batching and JIT settings.
* Safety / DoS:

  * Ensuring untrusted wasm modules can’t stall the worker despite cycle limits is non-trivial; you’ll have to benchmark and tune fuel-based execution or timeouts.
* Evolution “fun factor”:

  * You’ll probably need experiments to find world parameters that produce interesting emergent behaviors rather than trivial extinction or trivial dominance.

If you want next, I can help design the IR instruction set and a minimal Rust struct layout for genomes + a simple compiler-to-wasm pipeline.
