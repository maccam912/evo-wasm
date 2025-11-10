//! Host function implementations for the organism ABI.

use crate::context::{Action, OrganismContext};
use std::sync::Arc;
use wasmtime::*;

/// Host functions provided to organism WASM modules
#[derive(Clone)]
pub struct HostFunctions {
    pub context: Arc<OrganismContext>,
}

impl HostFunctions {
    pub fn new(context: Arc<OrganismContext>) -> Self {
        Self { context }
    }

    /// Add host function imports to a linker
    pub fn add_to_linker(&self, linker: &mut Linker<Self>) -> Result<(), anyhow::Error> {
        // env_read: (x: i32, y: i32) -> i32
        linker.func_wrap("env", "env_read", |mut caller: Caller<'_, Self>, x: i32, y: i32| {
            let host = caller.data();
            host.context.query_environment(x, y)
        })?;

        // get_energy: () -> i32
        linker.func_wrap("env", "get_energy", |mut caller: Caller<'_, Self>| {
            let host = caller.data();
            host.context.get_energy()
        })?;

        // get_age: () -> i32
        linker.func_wrap("env", "get_age", |mut caller: Caller<'_, Self>| {
            let host = caller.data();
            host.context.get_age() as i32
        })?;

        // move_dir: (dx: i32, dy: i32) -> i32
        linker.func_wrap(
            "env",
            "move_dir",
            |mut caller: Caller<'_, Self>, dx: i32, dy: i32| {
                let host = caller.data();
                host.context.add_action(Action::Move { dx, dy });
                1 // Success
            },
        )?;

        // eat: () -> i32
        linker.func_wrap("env", "eat", |mut caller: Caller<'_, Self>| {
            let host = caller.data();
            host.context.add_action(Action::Eat);
            1 // Success
        })?;

        // attack: (slot: i32, amount: i32) -> i32
        linker.func_wrap(
            "env",
            "attack",
            |mut caller: Caller<'_, Self>, target_slot: i32, amount: i32| {
                let host = caller.data();
                host.context.add_action(Action::Attack {
                    target_slot,
                    amount,
                });
                1 // Success
            },
        )?;

        // sense_neighbor: (slot: i32) -> i32
        linker.func_wrap(
            "env",
            "sense_neighbor",
            |mut caller: Caller<'_, Self>, slot: i32| {
                // For now, return 0 (no neighbor)
                // TODO: Implement proper neighbor sensing
                0
            },
        )?;

        // try_reproduce: () -> i32
        linker.func_wrap("env", "try_reproduce", |mut caller: Caller<'_, Self>| {
            let host = caller.data();
            host.context.add_action(Action::Reproduce);
            1 // Success
        })?;

        // emit_signal: (channel: i32, value: i32) -> void
        linker.func_wrap(
            "env",
            "emit_signal",
            |mut caller: Caller<'_, Self>, channel: i32, value: i32| {
                let host = caller.data();
                host.context.add_action(Action::EmitSignal { channel, value });
            },
        )?;

        Ok(())
    }
}
