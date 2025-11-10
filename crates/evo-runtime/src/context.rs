//! Execution context for organisms.

use evo_core::{Position, OrganismId};
use std::sync::Arc;
use parking_lot::RwLock;

/// Sensor data that can be read by the organism
#[derive(Debug, Clone)]
pub struct SensorData {
    pub energy: i32,
    pub age: u64,
    pub position: Position,
}

/// Actions that an organism can take
#[derive(Debug, Clone)]
pub enum Action {
    None,
    Move { dx: i32, dy: i32 },
    Eat,
    Attack { target_slot: i32, amount: i32 },
    Reproduce,
    EmitSignal { channel: i32, value: i32 },
}

/// Execution context shared between the host and organism
pub struct OrganismContext {
    pub organism_id: OrganismId,
    pub sensors: Arc<RwLock<SensorData>>,
    pub actions: Arc<RwLock<Vec<Action>>>,
    pub environment_query: Arc<dyn Fn(i32, i32) -> i32 + Send + Sync>,
}

impl OrganismContext {
    pub fn new(
        organism_id: OrganismId,
        initial_energy: i32,
        position: Position,
        environment_query: Arc<dyn Fn(i32, i32) -> i32 + Send + Sync>,
    ) -> Self {
        Self {
            organism_id,
            sensors: Arc::new(RwLock::new(SensorData {
                energy: initial_energy,
                age: 0,
                position,
            })),
            actions: Arc::new(RwLock::new(Vec::new())),
            environment_query,
        }
    }

    pub fn update_sensors(&self, energy: i32, age: u64, position: Position) {
        let mut sensors = self.sensors.write();
        sensors.energy = energy;
        sensors.age = age;
        sensors.position = position;
    }

    pub fn get_energy(&self) -> i32 {
        self.sensors.read().energy
    }

    pub fn get_age(&self) -> u64 {
        self.sensors.read().age
    }

    pub fn get_position(&self) -> Position {
        self.sensors.read().position
    }

    pub fn add_action(&self, action: Action) {
        self.actions.write().push(action);
    }

    pub fn take_actions(&self) -> Vec<Action> {
        let mut actions = self.actions.write();
        std::mem::take(&mut *actions)
    }

    pub fn query_environment(&self, x: i32, y: i32) -> i32 {
        (self.environment_query)(x, y)
    }
}
