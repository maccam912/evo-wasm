//! Core types and utilities for the Evo-WASM distributed evolutionary simulation system.

pub mod types;
pub mod config;
pub mod error;
pub mod fitness;

pub use error::{Error, Result};
pub use types::*;
pub use config::*;
pub use fitness::*;
