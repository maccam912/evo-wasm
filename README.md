# Evo-WASM: Distributed Evolutionary Simulation

A distributed artificial life simulation system where organisms are WebAssembly modules that evolve through natural selection. Based on concepts from Tierra, Avida, and distributed computing systems like Folding@home.

## Architecture

### Core Components

- **evo-core**: Shared types, configuration, and fitness tracking
- **evo-ir**: Custom Intermediate Representation for organism genomes
  - Mutation-friendly instruction set
  - Deterministic compilation to WASM
  - Mutation and crossover operators
- **evo-runtime**: WebAssembly execution environment
  - Organism ABI (sensors and actuators)
  - Fuel-based execution limits
  - Memory sandboxing
- **evo-world**: 2D grid world simulation
  - Resource management
  - Energy economy
  - Organism lifecycle
- **evo-server**: Central coordination server
  - Job distribution
  - Global evolution engine
  - Checkpoint/restore functionality
- **evo-worker**: Worker client
  - Executes island simulations
  - Reports results back to server

## Features

### Organism Design

Each organism is a sandboxed WebAssembly module with:

**Sensors** (read-only):
- `env_read(x, y)` - Read environment tiles
- `sense_neighbor(slot)` - Sense neighboring organisms
- `get_energy()` - Current energy level
- `get_age()` - Age in ticks

**Actuators** (actions):
- `move_dir(dx, dy)` - Move in a direction
- `eat()` - Consume resources
- `attack(slot, amount)` - Attack neighbor
- `try_reproduce()` - Attempt reproduction
- `emit_signal(channel, value)` - Communication

### Evolution System

**Genome Representation**:
- Custom IR (not raw WASM) to allow meaningful mutations
- Structured instruction set with type safety
- Validity-preserving operators

**Mutation Operators**:
- Point mutations (opcodes, operands, registers)
- Insertions and deletions
- Block duplication
- Function addition

**Selection**:
- Multi-objective fitness (lifetime, energy, offspring, exploration, combat)
- Pareto-based ranking for diversity
- Tournament selection with crossover

### World Simulation

**2D Toroidal Grid**:
- Resource patches (regenerating food)
- Obstacles
- Hazards (damage over time)

**Energy Economy**:
- Basal metabolic cost
- Instruction execution cost
- Action costs (move, attack, reproduce)
- Resource consumption for energy

**Distributed Islands**:
- Workers execute independent "island" simulations
- Server performs global selection across islands
- Prevents local optima through migration

## Getting Started

### Prerequisites

- Rust 1.75 or later
- Docker and Docker Compose (for containerized deployment)

### Local Development

```bash
# Build all components
cargo build --release

# Run server
cargo run --bin evo-server

# Run worker (in another terminal)
cargo run --bin evo-worker
```

### Docker Deployment

```bash
# Build and start server + 3 workers
docker-compose up --build

# With telemetry stack (Jaeger, Prometheus, Grafana)
docker-compose --profile telemetry up --build

# Scale workers
docker-compose up --scale worker=10
```

### Production Deployment

For deployment to `evo-wasm.rackspace.koski.co`:

```bash
# Server
docker build -f Dockerfile.server -t evo-server:latest .
docker run -d \
  -p 8080:8080 \
  -v /data/evo:/app/data \
  --name evo-server \
  evo-server:latest

# Workers (can run anywhere)
docker build -f Dockerfile.worker -t evo-worker:latest .
docker run -d \
  -e SERVER_URL=https://evo-wasm.rackspace.koski.co \
  --name evo-worker-1 \
  evo-worker:latest
```

## API Endpoints

### Server

- `GET /health` - Health check
- `POST /api/jobs/request` - Request a job (workers)
- `POST /api/jobs/submit` - Submit job results (workers)
- `GET /api/stats` - System statistics
- `GET /api/config` - Current job configuration

## Configuration

### Server Configuration

Environment variables:
- `BIND_ADDRESS` - Server bind address (default: 0.0.0.0)
- `PORT` - Server port (default: 8080)
- `DATABASE_PATH` - SQLite database path
- `CHECKPOINT_DIR` - Checkpoint directory
- `CHECKPOINT_INTERVAL_SECS` - Checkpoint interval
- `OTEL_ENDPOINT` - OpenTelemetry endpoint (optional)

### Worker Configuration

Environment variables:
- `SERVER_URL` - Central server URL (default: https://evo-wasm.rackspace.koski.co)
- `WORKER_ID` - Unique worker identifier (auto-generated if not set)
- `MAX_CONCURRENT_JOBS` - Concurrent jobs per worker (default: 1)
- `POLL_INTERVAL_MS` - Job polling interval (default: 5000)
- `OTEL_ENDPOINT` - OpenTelemetry endpoint (optional)

## Telemetry

The system uses OpenTelemetry for comprehensive observability:

**Metrics**:
- Job execution duration
- Jobs completed/failed
- Organism population statistics
- Resource consumption
- Fitness distributions

**Traces**:
- Job lifecycle (request → execute → submit)
- Simulation steps
- Evolution operations

**Logs**:
- Structured logging with tracing
- Correlation with traces
- Error tracking

### Viewing Telemetry

With the telemetry stack running:
- Jaeger UI: http://localhost:16686 (distributed tracing)
- Prometheus: http://localhost:9090 (metrics)
- Grafana: http://localhost:3000 (dashboards, user: admin, pass: admin)

## Dynamic Rule Delivery

The server can update simulation parameters without requiring worker updates:

- Energy costs
- World configuration
- Mutation rates
- Combat rules
- Population limits

Workers fetch the latest configuration before each job, ensuring consistency while allowing experimentation.

## Fault Tolerance

**Checkpointing**:
- Server periodically saves state
- Automatic restoration on restart
- Genome database backed by SQLite

**Job Reassignment**:
- Jobs timeout if workers don't respond
- Failed jobs are re-queued
- Deterministic simulation (same seed → same result) allows verification

## Development

### Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p evo-ir

# With output
cargo test -- --nocapture
```

### Project Structure

```
evo-wasm/
├── crates/
│   ├── evo-core/      # Shared types and utilities
│   ├── evo-ir/        # Genome IR and compiler
│   ├── evo-runtime/   # WASM execution
│   ├── evo-world/     # Simulation engine
│   ├── evo-server/    # Central server
│   └── evo-worker/    # Worker client
├── Dockerfile.server
├── Dockerfile.worker
├── docker-compose.yml
└── README.md
```

## Performance Considerations

**Server**:
- Uses SQLite for persistence (consider PostgreSQL for production)
- Checkpoints are incremental
- Job queue is in-memory (backed by database)

**Worker**:
- Each job runs in a spawned blocking task
- WASM execution is single-threaded per organism
- Can scale horizontally (many workers)

**Simulation**:
- Configurable fuel limits prevent runaway execution
- Memory sandboxing per organism
- Toroidal wrapping avoids edge effects

## Future Enhancements

- [ ] Web UI for visualization
- [ ] Real-time simulation viewer
- [ ] Lineage tree visualization
- [ ] User-defined fitness functions
- [ ] Multi-island topologies
- [ ] Cooperative behaviors
- [ ] Symbiosis mechanics
- [ ] Neural network integration
- [ ] GPU acceleration for physics

## License

MIT

## References

- [Tierra](http://life.ou.edu/pubs/tierra/) - Digital evolution system
- [Avida](https://avida.devosoft.org/) - Digital evolution research platform
- [The Bibites](https://leocaussan.itch.io/the-bibites) - Artificial life simulation
- [Folding@home](https://foldingathome.org/) - Distributed computing for science

## Contributing

Contributions welcome! Areas of interest:
- Performance optimization
- Alternative genome representations
- Novel fitness functions
- Visualization tools
- Documentation improvements
