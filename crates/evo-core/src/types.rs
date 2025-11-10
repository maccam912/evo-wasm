//! Core type definitions for the simulation.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for an organism lineage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineageId(pub Uuid);

impl LineageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for LineageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LineageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a simulation job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for an organism instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrganismId(pub Uuid);

impl OrganismId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OrganismId {
    fn default() -> Self {
        Self::new()
    }
}

/// 2D position in the world
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn add(&self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    /// Apply toroidal wrapping for given world dimensions
    pub fn wrap(&self, width: i32, height: i32) -> Self {
        Self {
            x: ((self.x % width) + width) % width,
            y: ((self.y % height) + height) % height,
        }
    }

    /// Manhattan distance to another position
    pub fn manhattan_distance(&self, other: &Position) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

/// Direction for movement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Direction {
    pub fn to_delta(&self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
            Direction::NorthEast => (1, -1),
            Direction::NorthWest => (-1, -1),
            Direction::SouthEast => (1, 1),
            Direction::SouthWest => (-1, 1),
        }
    }

    pub fn all() -> [Direction; 8] {
        [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
            Direction::NorthEast,
            Direction::NorthWest,
            Direction::SouthEast,
            Direction::SouthWest,
        ]
    }
}

/// Tile type in the world
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileType {
    Empty,
    Resource,
    Obstacle,
    Hazard,
}

/// Tile state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub tile_type: TileType,
    pub resource_amount: i32,
    pub max_resource: i32,
}

impl Tile {
    pub fn empty() -> Self {
        Self {
            tile_type: TileType::Empty,
            resource_amount: 0,
            max_resource: 0,
        }
    }

    pub fn resource(amount: i32, max: i32) -> Self {
        Self {
            tile_type: TileType::Resource,
            resource_amount: amount,
            max_resource: max,
        }
    }

    pub fn obstacle() -> Self {
        Self {
            tile_type: TileType::Obstacle,
            resource_amount: 0,
            max_resource: 0,
        }
    }

    pub fn hazard() -> Self {
        Self {
            tile_type: TileType::Hazard,
            resource_amount: 0,
            max_resource: 0,
        }
    }

    /// Regenerate resources using logistic growth
    pub fn regenerate(&mut self, rate: f32) {
        if self.tile_type == TileType::Resource && self.resource_amount < self.max_resource {
            let growth = (rate * self.resource_amount as f32
                * (1.0 - self.resource_amount as f32 / self.max_resource as f32)) as i32;
            self.resource_amount = (self.resource_amount + growth.max(1)).min(self.max_resource);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_wrap() {
        let pos = Position::new(5, 5);
        let wrapped = pos.wrap(10, 10);
        assert_eq!(wrapped, Position::new(5, 5));

        let pos = Position::new(-1, -1);
        let wrapped = pos.wrap(10, 10);
        assert_eq!(wrapped, Position::new(9, 9));

        let pos = Position::new(10, 10);
        let wrapped = pos.wrap(10, 10);
        assert_eq!(wrapped, Position::new(0, 0));
    }

    #[test]
    fn test_manhattan_distance() {
        let pos1 = Position::new(0, 0);
        let pos2 = Position::new(3, 4);
        assert_eq!(pos1.manhattan_distance(&pos2), 7);
    }

    #[test]
    fn test_tile_regeneration() {
        let mut tile = Tile::resource(50, 100);
        tile.regenerate(0.1);
        assert!(tile.resource_amount > 50);
        assert!(tile.resource_amount <= 100);
    }

    #[test]
    fn test_direction_delta() {
        assert_eq!(Direction::North.to_delta(), (0, -1));
        assert_eq!(Direction::South.to_delta(), (0, 1));
        assert_eq!(Direction::East.to_delta(), (1, 0));
        assert_eq!(Direction::West.to_delta(), (-1, 0));
    }
}
