//! WASM runtime for executing organism modules.
//!
//! This module provides the execution environment for organisms, including:
//! - Host function implementations (organism ABI)
//! - Fuel-based execution limits
//! - Memory sandboxing

pub mod host_functions;
pub mod instance;
pub mod context;

pub use host_functions::HostFunctions;
pub use instance::OrganismInstance;
pub use context::OrganismContext;

use evo_core::{Error, Result};
use wasmtime::*;

/// Runtime configuration
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum fuel per step
    pub max_fuel: u64,
    /// Maximum memory size (bytes)
    pub max_memory_bytes: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_fuel: 10_000,
            max_memory_bytes: 65536,
        }
    }
}

/// The WASM runtime manager
pub struct Runtime {
    engine: Engine,
    config: RuntimeConfig,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Result<Self> {
        let mut wasm_config = Config::new();
        wasm_config.consume_fuel(true);
        wasm_config.max_wasm_stack(128 * 1024); // 128 KiB stack

        let engine = Engine::new(&wasm_config)
            .map_err(|e| Error::Wasm(format!("Failed to create engine: {}", e)))?;

        Ok(Self { engine, config })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    /// Create a new organism instance from WASM bytes
    pub fn instantiate(
        &self,
        wasm_bytes: &[u8],
        host_functions: HostFunctions,
    ) -> Result<OrganismInstance> {
        OrganismInstance::new(&self.engine, wasm_bytes, host_functions, self.config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_runtime() {
        let runtime = Runtime::new(RuntimeConfig::default());
        assert!(runtime.is_ok());
    }
}
