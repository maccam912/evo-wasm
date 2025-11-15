//! Configuration types for the simulation.

use serde::{Deserialize, Serialize};

/// World configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    /// Width of the world grid
    pub width: i32,
    /// Height of the world grid
    pub height: i32,
    /// Resource density (0.0 to 1.0)
    pub resource_density: f32,
    /// Maximum resource per tile
    pub max_resource_per_tile: i32,
    /// Resource regeneration rate
    pub resource_regen_rate: f32,
    /// Obstacle density (0.0 to 1.0)
    pub obstacle_density: f32,
    /// Hazard density (0.0 to 1.0)
    pub hazard_density: f32,
    /// Hazard damage per tick
    pub hazard_damage: i32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            resource_density: 0.3,
            max_resource_per_tile: 1000,
            resource_regen_rate: 0.15,  // Increased from 0.05 to support larger populations
            obstacle_density: 0.05,
            hazard_density: 0.02,
            hazard_damage: 10,
        }
    }
}

/// Energy and cost configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyConfig {
    /// Starting energy for new organisms
    pub initial_energy: i32,
    /// Basal metabolic cost per tick
    pub basal_cost: i32,
    /// Cost per WASM instruction executed (scaled)
    pub instruction_cost_per_k: i32,
    /// Energy cost to move
    pub move_cost: i32,
    /// Energy cost to attack
    pub attack_cost: i32,
    /// Energy cost to reproduce
    pub reproduce_cost: i32,
    /// Energy gained from eating (multiplier of resource consumed)
    pub eat_efficiency: f32,
    /// Minimum energy required to reproduce
    pub min_reproduce_energy: i32,
}

impl Default for EnergyConfig {
    fn default() -> Self {
        Self {
            initial_energy: 1500,  // Increased from 1000 to give organisms better starting chances
            basal_cost: 1,
            instruction_cost_per_k: 1,
            move_cost: 3,  // Reduced from 5 to make movement more affordable
            attack_cost: 10,
            reproduce_cost: 300,  // Reduced from 500 to encourage more reproduction
            eat_efficiency: 1.5,  // Increased from 0.8 to create positive energy economy
            min_reproduce_energy: 400,  // Reduced from 600 to allow earlier reproduction
        }
    }
}

/// Organism execution limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Maximum WASM instructions per step
    pub max_fuel_per_step: u64,
    /// Maximum memory per organism (bytes)
    pub max_memory_bytes: usize,
    /// Sensor radius (how far organisms can see)
    pub sensor_radius: i32,
    /// Maximum number of signals an organism can emit per step
    pub max_signals_per_step: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_fuel_per_step: 10_000,
            max_memory_bytes: 65536, // 64 KiB
            sensor_radius: 3,
            max_signals_per_step: 5,
        }
    }
}

/// Simulation job configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    /// Number of ticks to run the simulation
    pub num_ticks: u64,
    /// Random seed for reproducibility
    pub seed: u64,
    /// World configuration
    pub world_config: WorldConfig,
    /// Energy configuration
    pub energy_config: EnergyConfig,
    /// Execution configuration
    pub exec_config: ExecutionConfig,
    /// Dynamic rules (server-defined behavior)
    pub dynamic_rules: DynamicRules,
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            num_ticks: 10_000,
            seed: 0,
            world_config: WorldConfig::default(),
            energy_config: EnergyConfig::default(),
            exec_config: ExecutionConfig::default(),
            dynamic_rules: DynamicRules::default(),
        }
    }
}

/// Dynamic rules that can be updated on the server without client changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRules {
    /// Allow organisms to attack each other
    pub allow_combat: bool,
    /// Allow organisms to reproduce
    pub allow_reproduction: bool,
    /// Mutation rate for offspring (0.0 to 1.0)
    pub mutation_rate: f32,
    /// Maximum number of organisms in the simulation
    pub max_population: usize,
    /// Custom parameters for experimental features
    pub custom_params: std::collections::HashMap<String, f32>,
}

impl Default for DynamicRules {
    fn default() -> Self {
        Self {
            allow_combat: true,
            allow_reproduction: true,
            mutation_rate: 0.01,
            max_population: 1000,
            custom_params: std::collections::HashMap::new(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_address: String,
    /// Server port
    pub port: u16,
    /// Database path (SQLite)
    pub database_path: String,
    /// Checkpoint directory
    pub checkpoint_dir: String,
    /// Checkpoint interval (seconds)
    pub checkpoint_interval_secs: u64,
    /// OpenTelemetry endpoint
    pub otel_endpoint: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            database_path: "./data/evo.db".to_string(),
            checkpoint_dir: "./data/checkpoints".to_string(),
            checkpoint_interval_secs: 300, // 5 minutes
            otel_endpoint: None,
        }
    }
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Central server URL
    pub server_url: String,
    /// Worker identifier
    pub worker_id: Option<String>,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: usize,
    /// Poll interval (milliseconds)
    pub poll_interval_ms: u64,
    /// OpenTelemetry endpoint
    pub otel_endpoint: Option<String>,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            server_url: "https://evo-wasm.rackspace.koski.co".to_string(),
            worker_id: None,
            max_concurrent_jobs: 1,
            poll_interval_ms: 5000,
            otel_endpoint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configs() {
        let world_config = WorldConfig::default();
        assert_eq!(world_config.width, 256);
        assert_eq!(world_config.height, 256);

        let energy_config = EnergyConfig::default();
        assert_eq!(energy_config.initial_energy, 1500);

        let exec_config = ExecutionConfig::default();
        assert_eq!(exec_config.max_fuel_per_step, 10_000);

        let job_config = JobConfig::default();
        assert_eq!(job_config.num_ticks, 10_000);
    }

    #[test]
    fn test_dynamic_rules_serialization() {
        let rules = DynamicRules::default();
        let json = serde_json::to_string(&rules).unwrap();
        let deserialized: DynamicRules = serde_json::from_str(&json).unwrap();
        assert_eq!(rules.allow_combat, deserialized.allow_combat);
        assert_eq!(rules.mutation_rate, deserialized.mutation_rate);
    }
}
