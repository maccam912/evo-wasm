//! World simulation engine.
//!
//! This module implements the 2D grid world where organisms live, compete, and evolve.

pub mod grid;
pub mod organism;
pub mod simulation;
pub mod island;

pub use grid::Grid;
pub use organism::Organism;
pub use simulation::Simulation;
pub use island::Island;
