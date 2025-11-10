//! Fitness and statistics tracking for organisms.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::LineageId;

/// Fitness metrics for an organism
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FitnessMetrics {
    /// Number of ticks survived
    pub lifetime: u64,
    /// Net energy gained (energy at death - initial energy)
    pub net_energy: i64,
    /// Total offspring produced
    pub offspring_count: u32,
    /// Number of distinct tiles visited
    pub tiles_explored: u32,
    /// Number of successful attacks
    pub kills: u32,
    /// Number of times eaten
    pub times_eaten: u32,
    /// Total damage dealt
    pub damage_dealt: i64,
    /// Total damage received
    pub damage_received: i64,
    /// Custom metrics
    pub custom: HashMap<String, f64>,
}

impl FitnessMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute a simple scalar fitness (for basic ranking)
    pub fn scalar_fitness(&self) -> f64 {
        // Weighted combination of metrics
        let lifetime_score = self.lifetime as f64 * 1.0;
        let energy_score = self.net_energy.max(0) as f64 * 0.5;
        let offspring_score = self.offspring_count as f64 * 100.0;
        let exploration_score = self.tiles_explored as f64 * 0.1;
        let combat_score = self.kills as f64 * 50.0;

        lifetime_score + energy_score + offspring_score + exploration_score + combat_score
    }

    /// Check if this organism dominates another (for Pareto ranking)
    pub fn dominates(&self, other: &FitnessMetrics) -> bool {
        let mut better_in_any = false;
        let mut worse_in_any = false;

        // Compare key objectives
        if self.lifetime > other.lifetime {
            better_in_any = true;
        } else if self.lifetime < other.lifetime {
            worse_in_any = true;
        }

        if self.net_energy > other.net_energy {
            better_in_any = true;
        } else if self.net_energy < other.net_energy {
            worse_in_any = true;
        }

        if self.offspring_count > other.offspring_count {
            better_in_any = true;
        } else if self.offspring_count < other.offspring_count {
            worse_in_any = true;
        }

        if self.tiles_explored > other.tiles_explored {
            better_in_any = true;
        } else if self.tiles_explored < other.tiles_explored {
            worse_in_any = true;
        }

        better_in_any && !worse_in_any
    }
}

/// Lineage statistics aggregated across all organisms in a lineage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageStats {
    pub lineage_id: LineageId,
    pub total_organisms: u32,
    pub avg_fitness: FitnessMetrics,
    pub best_fitness: FitnessMetrics,
    pub generation: u32,
}

impl LineageStats {
    pub fn new(lineage_id: LineageId) -> Self {
        Self {
            lineage_id,
            total_organisms: 0,
            avg_fitness: FitnessMetrics::new(),
            best_fitness: FitnessMetrics::new(),
            generation: 0,
        }
    }

    /// Update statistics with a new organism's metrics
    pub fn update(&mut self, metrics: &FitnessMetrics) {
        let n = self.total_organisms as f64;
        let new_n = n + 1.0;

        // Update average (incremental mean)
        self.avg_fitness.lifetime = ((self.avg_fitness.lifetime as f64 * n
            + metrics.lifetime as f64)
            / new_n) as u64;
        self.avg_fitness.net_energy = ((self.avg_fitness.net_energy as f64 * n
            + metrics.net_energy as f64)
            / new_n) as i64;
        self.avg_fitness.offspring_count = ((self.avg_fitness.offspring_count as f64 * n
            + metrics.offspring_count as f64)
            / new_n) as u32;
        self.avg_fitness.tiles_explored = ((self.avg_fitness.tiles_explored as f64 * n
            + metrics.tiles_explored as f64)
            / new_n) as u32;
        self.avg_fitness.kills = ((self.avg_fitness.kills as f64 * n + metrics.kills as f64)
            / new_n) as u32;

        // Update best
        if metrics.scalar_fitness() > self.best_fitness.scalar_fitness() {
            self.best_fitness = metrics.clone();
        }

        self.total_organisms += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_fitness() {
        let mut metrics = FitnessMetrics::new();
        metrics.lifetime = 100;
        metrics.net_energy = 500;
        metrics.offspring_count = 5;
        metrics.tiles_explored = 50;
        metrics.kills = 2;

        let fitness = metrics.scalar_fitness();
        assert!(fitness > 0.0);
    }

    #[test]
    fn test_dominance() {
        let mut m1 = FitnessMetrics::new();
        m1.lifetime = 100;
        m1.net_energy = 500;
        m1.offspring_count = 5;

        let mut m2 = FitnessMetrics::new();
        m2.lifetime = 50;
        m2.net_energy = 300;
        m2.offspring_count = 3;

        assert!(m1.dominates(&m2));
        assert!(!m2.dominates(&m1));
    }

    #[test]
    fn test_lineage_stats_update() {
        let lineage_id = LineageId::new();
        let mut stats = LineageStats::new(lineage_id);

        let mut m1 = FitnessMetrics::new();
        m1.lifetime = 100;
        m1.offspring_count = 5;

        let mut m2 = FitnessMetrics::new();
        m2.lifetime = 200;
        m2.offspring_count = 10;

        stats.update(&m1);
        stats.update(&m2);

        assert_eq!(stats.total_organisms, 2);
        assert_eq!(stats.avg_fitness.lifetime, 150);
        assert_eq!(stats.best_fitness.offspring_count, 10);
    }
}
