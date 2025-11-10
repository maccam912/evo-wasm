//! 2D grid for the world.

use evo_core::{Position, Tile, TileType, WorldConfig};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// A 2D toroidal grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
}

impl Grid {
    pub fn new(width: i32, height: i32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            tiles: vec![Tile::empty(); size],
        }
    }

    /// Create a grid from world configuration
    pub fn from_config(config: &WorldConfig, rng: &mut ChaCha8Rng) -> Self {
        let mut grid = Self::new(config.width, config.height);

        for y in 0..config.height {
            for x in 0..config.width {
                let pos = Position::new(x, y);
                let roll = rng.gen::<f32>();

                if roll < config.obstacle_density {
                    grid.set(pos, Tile::obstacle());
                } else if roll < config.obstacle_density + config.hazard_density {
                    grid.set(pos, Tile::hazard());
                } else if roll < config.obstacle_density + config.hazard_density + config.resource_density {
                    grid.set(
                        pos,
                        Tile::resource(
                            config.max_resource_per_tile,
                            config.max_resource_per_tile,
                        ),
                    );
                }
            }
        }

        grid
    }

    /// Get tile at position (with toroidal wrapping)
    pub fn get(&self, pos: Position) -> &Tile {
        let wrapped = pos.wrap(self.width, self.height);
        let index = self.pos_to_index(wrapped);
        &self.tiles[index]
    }

    /// Get mutable tile at position
    pub fn get_mut(&mut self, pos: Position) -> &mut Tile {
        let wrapped = pos.wrap(self.width, self.height);
        let index = self.pos_to_index(wrapped);
        &mut self.tiles[index]
    }

    /// Set tile at position
    pub fn set(&mut self, pos: Position, tile: Tile) {
        let wrapped = pos.wrap(self.width, self.height);
        let index = self.pos_to_index(wrapped);
        self.tiles[index] = tile;
    }

    /// Regenerate resources on all resource tiles
    pub fn regenerate_resources(&mut self, rate: f32) {
        for tile in &mut self.tiles {
            tile.regenerate(rate);
        }
    }

    /// Get neighbors of a position (returns cloned tiles to avoid lifetime issues)
    pub fn neighbors(&self, pos: Position, radius: i32) -> Vec<(Position, Tile)> {
        let mut neighbors = Vec::new();

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let neighbor_pos = pos.add(dx, dy);
                neighbors.push((neighbor_pos, self.get(neighbor_pos).clone()));
            }
        }

        neighbors
    }

    fn pos_to_index(&self, pos: Position) -> usize {
        (pos.y * self.width + pos.x) as usize
    }

    /// Get position from index
    pub fn index_to_pos(&self, index: usize) -> Position {
        let x = (index as i32) % self.width;
        let y = (index as i32) / self.width;
        Position::new(x, y)
    }

    /// Iterator over all positions
    pub fn positions(&self) -> impl Iterator<Item = Position> + '_ {
        (0..self.tiles.len()).map(move |i| self.index_to_pos(i))
    }

    /// Iterator over all tiles with positions
    pub fn iter(&self) -> impl Iterator<Item = (Position, &Tile)> + '_ {
        self.tiles
            .iter()
            .enumerate()
            .map(move |(i, tile)| (self.index_to_pos(i), tile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_grid_creation() {
        let grid = Grid::new(10, 10);
        assert_eq!(grid.width, 10);
        assert_eq!(grid.height, 10);
        assert_eq!(grid.tiles.len(), 100);
    }

    #[test]
    fn test_toroidal_wrapping() {
        let grid = Grid::new(10, 10);

        let pos = Position::new(-1, -1);
        let tile = grid.get(pos);
        // Should wrap to (9, 9)
        assert_eq!(tile.tile_type, TileType::Empty);

        let pos = Position::new(10, 10);
        let tile = grid.get(pos);
        // Should wrap to (0, 0)
        assert_eq!(tile.tile_type, TileType::Empty);
    }

    #[test]
    fn test_grid_from_config() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let config = WorldConfig {
            width: 20,
            height: 20,
            resource_density: 0.5,
            obstacle_density: 0.1,
            ..Default::default()
        };

        let grid = Grid::from_config(&config, &mut rng);
        assert_eq!(grid.width, 20);
        assert_eq!(grid.height, 20);

        // Count tile types
        let mut resource_count = 0;
        let mut obstacle_count = 0;

        for (_, tile) in grid.iter() {
            match tile.tile_type {
                TileType::Resource => resource_count += 1,
                TileType::Obstacle => obstacle_count += 1,
                _ => {}
            }
        }

        assert!(resource_count > 0);
        assert!(obstacle_count > 0);
    }

    #[test]
    fn test_neighbors() {
        let grid = Grid::new(10, 10);
        let pos = Position::new(5, 5);
        let neighbors = grid.neighbors(pos, 1);

        // Should have 8 neighbors
        assert_eq!(neighbors.len(), 8);
    }

    #[test]
    fn test_regenerate_resources() {
        let mut grid = Grid::new(10, 10);
        grid.set(Position::new(5, 5), Tile::resource(50, 100));

        let initial_amount = grid.get(Position::new(5, 5)).resource_amount;
        grid.regenerate_resources(0.1);
        let new_amount = grid.get(Position::new(5, 5)).resource_amount;

        assert!(new_amount > initial_amount);
    }
}
