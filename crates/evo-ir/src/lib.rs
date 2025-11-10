//! Intermediate Representation (IR) for organism genomes.
//!
//! This module defines a custom instruction set that organisms use as their genotype.
//! The IR is designed to be:
//! - Mutation-friendly: changes preserve validity
//! - Expressive: can encode diverse behaviors
//! - Compact: efficient storage and transmission
//! - Compilable: deterministic translation to WASM

pub mod instruction;
pub mod program;
pub mod compiler;
pub mod mutation;
pub mod validation;

pub use instruction::{Instruction, Opcode, Value, Register};
pub use program::{Program, Function, BasicBlock};
pub use compiler::Compiler;
pub use mutation::{Mutator, MutationConfig};
pub use validation::validate_program;
