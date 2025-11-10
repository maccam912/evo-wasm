//! Organism state and management.

use evo_core::{FitnessMetrics, LineageId, OrganismId, Position};
use evo_ir::Program;
use evo_runtime::{OrganismContext, OrganismInstance, RuntimeConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// An organism in the simulation
pub struct Organism {
    pub id: OrganismId,
    pub lineage_id: LineageId,
    pub position: Position,
    pub energy: i32,
    pub age: u64,
    pub genome: Program,
    pub instance: Option<OrganismInstance>,
    pub metrics: FitnessMetrics,
    pub visited_tiles: HashSet<Position>,
}

impl Organism {
    pub fn new(
        lineage_id: LineageId,
        position: Position,
        energy: i32,
        genome: Program,
    ) -> Self {
        let mut visited = HashSet::new();
        visited.insert(position);

        Self {
            id: OrganismId::new(),
            lineage_id,
            position,
            energy,
            age: 0,
            genome,
            instance: None,
            metrics: FitnessMetrics::new(),
            visited_tiles: visited,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.energy > 0
    }

    pub fn add_energy(&mut self, amount: i32) {
        self.energy += amount;
    }

    pub fn consume_energy(&mut self, amount: i32) -> bool {
        if self.energy >= amount {
            self.energy -= amount;
            true
        } else {
            self.energy = 0;
            false
        }
    }

    pub fn move_to(&mut self, new_position: Position) {
        self.position = new_position;
        self.visited_tiles.insert(new_position);
        self.metrics.tiles_explored = self.visited_tiles.len() as u32;
    }

    pub fn tick(&mut self) {
        self.age += 1;
        self.metrics.lifetime = self.age;
    }

    pub fn record_kill(&mut self) {
        self.metrics.kills += 1;
    }

    pub fn record_damage_dealt(&mut self, amount: i32) {
        self.metrics.damage_dealt += amount as i64;
    }

    pub fn record_damage_received(&mut self, amount: i32) {
        self.metrics.damage_received += amount as i64;
    }

    pub fn record_offspring(&mut self) {
        self.metrics.offspring_count += 1;
    }

    /// Finalize metrics when the organism dies
    pub fn finalize_metrics(&mut self, initial_energy: i32) {
        self.metrics.net_energy = self.energy as i64 - initial_energy as i64;
    }
}

/// Serializable organism data (without WASM instance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganismData {
    pub id: OrganismId,
    pub lineage_id: LineageId,
    pub position: Position,
    pub energy: i32,
    pub age: u64,
    pub genome: Program,
    pub metrics: FitnessMetrics,
}

impl From<&Organism> for OrganismData {
    fn from(org: &Organism) -> Self {
        Self {
            id: org.id,
            lineage_id: org.lineage_id,
            position: org.position,
            energy: org.energy,
            age: org.age,
            genome: org.genome.clone(),
            metrics: org.metrics.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evo_ir::Program;

    #[test]
    fn test_organism_creation() {
        let lineage_id = LineageId::new();
        let position = Position::new(5, 5);
        let energy = 1000;
        let genome = Program::new();

        let organism = Organism::new(lineage_id, position, energy, genome);

        assert_eq!(organism.lineage_id, lineage_id);
        assert_eq!(organism.position, position);
        assert_eq!(organism.energy, energy);
        assert!(organism.is_alive());
    }

    #[test]
    fn test_energy_management() {
        let organism = &mut Organism::new(
            LineageId::new(),
            Position::new(0, 0),
            100,
            Program::new(),
        );

        assert!(organism.consume_energy(50));
        assert_eq!(organism.energy, 50);

        assert!(!organism.consume_energy(100));
        assert_eq!(organism.energy, 0);
        assert!(!organism.is_alive());
    }

    #[test]
    fn test_movement_tracking() {
        let mut organism = Organism::new(
            LineageId::new(),
            Position::new(0, 0),
            100,
            Program::new(),
        );

        assert_eq!(organism.visited_tiles.len(), 1);

        organism.move_to(Position::new(1, 1));
        assert_eq!(organism.visited_tiles.len(), 2);
        assert_eq!(organism.metrics.tiles_explored, 2);

        // Move to same tile again
        organism.move_to(Position::new(1, 1));
        assert_eq!(organism.visited_tiles.len(), 2);
    }

    #[test]
    fn test_metrics_tracking() {
        let mut organism = Organism::new(
            LineageId::new(),
            Position::new(0, 0),
            1000,
            Program::new(),
        );

        organism.tick();
        assert_eq!(organism.metrics.lifetime, 1);

        organism.record_kill();
        assert_eq!(organism.metrics.kills, 1);

        organism.record_damage_dealt(10);
        assert_eq!(organism.metrics.damage_dealt, 10);

        organism.record_offspring();
        assert_eq!(organism.metrics.offspring_count, 1);
    }

    #[test]
    fn test_organism_serialization() {
        let organism = Organism::new(
            LineageId::new(),
            Position::new(5, 5),
            1000,
            Program::new(),
        );

        let data = OrganismData::from(&organism);
        assert_eq!(data.id, organism.id);
        assert_eq!(data.lineage_id, organism.lineage_id);
        assert_eq!(data.position, organism.position);
    }
}
