//! Organism instance management.

use crate::context::Action;
use crate::host_functions::HostFunctions;
use crate::RuntimeConfig;
use evo_core::{Error, Result};
use wasmtime::*;

/// A running organism instance
pub struct OrganismInstance {
    store: Store<HostFunctions>,
    instance: Instance,
    init_func: TypedFunc<i64, ()>,
    step_func: TypedFunc<i32, i32>,
    config: RuntimeConfig,
}

impl OrganismInstance {
    pub fn new(
        engine: &Engine,
        wasm_bytes: &[u8],
        host_functions: HostFunctions,
        config: RuntimeConfig,
    ) -> Result<Self> {
        let module = Module::new(engine, wasm_bytes)
            .map_err(|e| Error::Wasm(format!("Failed to compile module: {}", e)))?;

        let mut linker = Linker::new(engine);
        host_functions
            .add_to_linker(&mut linker)
            .map_err(|e| Error::Wasm(format!("Failed to add host functions: {}", e)))?;

        let mut store = Store::new(engine, host_functions);
        store.set_fuel(config.max_fuel).map_err(|e| {
            Error::Wasm(format!("Failed to set fuel: {}", e))
        })?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::Wasm(format!("Failed to instantiate: {}", e)))?;

        // Get the exported functions
        let init_func = instance
            .get_typed_func::<i64, ()>(&mut store, "init")
            .map_err(|e| Error::Wasm(format!("Failed to get init function: {}", e)))?;

        let step_func = instance
            .get_typed_func::<i32, i32>(&mut store, "step")
            .map_err(|e| Error::Wasm(format!("Failed to get step function: {}", e)))?;

        Ok(Self {
            store,
            instance,
            init_func,
            step_func,
            config,
        })
    }

    /// Initialize the organism with a seed
    pub fn init(&mut self, seed: u64) -> Result<()> {
        self.store.set_fuel(self.config.max_fuel).map_err(|e| {
            Error::Wasm(format!("Failed to set fuel: {}", e))
        })?;

        self.init_func
            .call(&mut self.store, seed as i64)
            .map_err(|e| Error::Wasm(format!("Init function failed: {}", e)))?;

        Ok(())
    }

    /// Execute one step of the organism
    pub fn step(&mut self, ctx_ptr: i32) -> Result<(i32, Vec<Action>)> {
        // Reset fuel for this step
        self.store.set_fuel(self.config.max_fuel).map_err(|e| {
            Error::Wasm(format!("Failed to set fuel: {}", e))
        })?;

        // Call the step function
        let result = self
            .step_func
            .call(&mut self.store, ctx_ptr)
            .map_err(|e| {
                // Check if we ran out of fuel
                if let Some(trap) = e.downcast_ref::<Trap>() {
                    if matches!(trap, Trap::OutOfFuel) {
                        return Error::ResourceExhausted("Out of fuel".to_string());
                    }
                }
                Error::Wasm(format!("Step function failed: {}", e))
            })?;

        // Get the actions from the context
        let actions = self.store.data().context.take_actions();

        // Get remaining fuel
        let fuel_consumed = self.config.max_fuel
            - self.store.get_fuel().unwrap_or(0);

        tracing::debug!("Step consumed {} fuel", fuel_consumed);

        Ok((result, actions))
    }

    /// Get the fuel consumed in the last execution
    pub fn fuel_consumed(&self) -> u64 {
        self.config.max_fuel - self.store.get_fuel().unwrap_or(0)
    }

    /// Get reference to the host functions
    pub fn host_functions(&self) -> &HostFunctions {
        self.store.data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::OrganismContext;
    use crate::Runtime;
    use evo_core::{OrganismId, Position};
    use std::sync::Arc;

    fn create_simple_wasm() -> Vec<u8> {
        // A minimal WASM module with init and step functions
        // This is a placeholder - in real tests we'd compile from IR
        use evo_ir::*;

        let mut program = Program::new();

        // Init function
        let mut init = program::Function::new("init".to_string(), 1, program::ReturnType::Void);
        init.get_block_mut(0).unwrap().add_instruction(
            instruction::Instruction::return_void()
        );
        program.add_function(init);

        // Step function
        let mut step = program::Function::new("step".to_string(), 1, program::ReturnType::Int);
        step.get_block_mut(0).unwrap().add_instruction(
            instruction::Instruction::load_const(
                instruction::Register(0),
                instruction::Value::Int(0)
            )
        );
        step.get_block_mut(0).unwrap().add_instruction(
            instruction::Instruction::return_value(instruction::Register(0))
        );
        program.add_function(step);

        let compiler = Compiler::new(compiler::CompilerConfig::default());
        compiler.compile(&program).unwrap()
    }

    #[test]
    fn test_create_instance() {
        let runtime = Runtime::new(RuntimeConfig::default()).unwrap();
        let wasm_bytes = create_simple_wasm();

        let context = Arc::new(OrganismContext::new(
            OrganismId::new(),
            1000,
            Position::new(0, 0),
            Arc::new(|_, _| 0),
        ));

        let host_functions = HostFunctions::new(context);
        let instance = runtime.instantiate(&wasm_bytes, host_functions);

        assert!(instance.is_ok());
    }

    #[test]
    fn test_init_and_step() {
        let runtime = Runtime::new(RuntimeConfig::default()).unwrap();
        let wasm_bytes = create_simple_wasm();

        let context = Arc::new(OrganismContext::new(
            OrganismId::new(),
            1000,
            Position::new(0, 0),
            Arc::new(|_, _| 0),
        ));

        let host_functions = HostFunctions::new(context);
        let mut instance = runtime.instantiate(&wasm_bytes, host_functions).unwrap();

        // Initialize
        assert!(instance.init(12345).is_ok());

        // Step
        let result = instance.step(0);
        assert!(result.is_ok());
    }
}
