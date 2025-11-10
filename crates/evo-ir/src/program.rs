//! Program structure for organism genomes.

use serde::{Deserialize, Serialize};
use crate::instruction::Instruction;

/// A basic block is a sequence of instructions with no internal control flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub instructions: Vec<Instruction>,
}

impl BasicBlock {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }

    pub fn with_instructions(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    pub fn add_instruction(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.instructions.len()
    }
}

impl Default for BasicBlock {
    fn default() -> Self {
        Self::new()
    }
}

/// A function contains multiple basic blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub num_params: usize,
    pub num_locals: usize,
    pub blocks: Vec<BasicBlock>,
    pub return_type: ReturnType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReturnType {
    Void,
    Int,
}

impl Function {
    pub fn new(name: String, num_params: usize, return_type: ReturnType) -> Self {
        Self {
            name,
            num_params,
            num_locals: 0,
            blocks: vec![BasicBlock::new()],
            return_type,
        }
    }

    pub fn add_block(&mut self, block: BasicBlock) -> u32 {
        self.blocks.push(block);
        (self.blocks.len() - 1) as u32
    }

    pub fn get_block_mut(&mut self, index: usize) -> Option<&mut BasicBlock> {
        self.blocks.get_mut(index)
    }

    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// Count total instructions in the function
    pub fn instruction_count(&self) -> usize {
        self.blocks.iter().map(|b| b.len()).sum()
    }
}

/// A complete organism program (genome)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub functions: Vec<Function>,
    pub memory_size: usize,
    pub version: u32,
}

impl Program {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            memory_size: 256, // Default memory size in 4-byte words
            version: 1,
        }
    }

    pub fn with_functions(functions: Vec<Function>) -> Self {
        Self {
            functions,
            memory_size: 256,
            version: 1,
        }
    }

    pub fn add_function(&mut self, function: Function) -> u32 {
        self.functions.push(function);
        (self.functions.len() - 1) as u32
    }

    pub fn get_function(&self, index: usize) -> Option<&Function> {
        self.functions.get(index)
    }

    pub fn get_function_mut(&mut self, index: usize) -> Option<&mut Function> {
        self.functions.get_mut(index)
    }

    pub fn num_functions(&self) -> usize {
        self.functions.len()
    }

    /// Find the main step function (required)
    pub fn get_step_function(&self) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == "step")
    }

    /// Find the init function (required)
    pub fn get_init_function(&self) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == "init")
    }

    /// Count total instructions in the program
    pub fn total_instructions(&self) -> usize {
        self.functions.iter().map(|f| f.instruction_count()).sum()
    }

    /// Serialize the program to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, evo_core::Error> {
        bincode::serialize(self).map_err(|e| evo_core::Error::Serialization(e.to_string()))
    }

    /// Deserialize a program from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, evo_core::Error> {
        bincode::deserialize(bytes).map_err(|e| evo_core::Error::Serialization(e.to_string()))
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::{Instruction, Opcode, Register};

    #[test]
    fn test_basic_block() {
        let mut block = BasicBlock::new();
        assert!(block.is_empty());

        block.add_instruction(Instruction::return_void());
        assert_eq!(block.len(), 1);
        assert!(!block.is_empty());
    }

    #[test]
    fn test_function() {
        let mut func = Function::new("test".to_string(), 0, ReturnType::Void);
        assert_eq!(func.num_blocks(), 1);

        let block = BasicBlock::new();
        func.add_block(block);
        assert_eq!(func.num_blocks(), 2);
    }

    #[test]
    fn test_program() {
        let mut program = Program::new();
        assert_eq!(program.num_functions(), 0);

        let func = Function::new("step".to_string(), 1, ReturnType::Int);
        program.add_function(func);
        assert_eq!(program.num_functions(), 1);

        assert!(program.get_step_function().is_some());
        assert!(program.get_init_function().is_none());
    }

    #[test]
    fn test_program_serialization() {
        let program = Program::new();
        let bytes = program.to_bytes().unwrap();
        let deserialized = Program::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.num_functions(), program.num_functions());
    }

    #[test]
    fn test_instruction_count() {
        let mut func = Function::new("test".to_string(), 0, ReturnType::Int);
        func.get_block_mut(0).unwrap().add_instruction(
            Instruction::load_const(Register(0), crate::instruction::Value::Int(42))
        );
        func.get_block_mut(0).unwrap().add_instruction(
            Instruction::return_value(Register(0))
        );

        assert_eq!(func.instruction_count(), 2);

        let mut program = Program::new();
        program.add_function(func);
        assert_eq!(program.total_instructions(), 2);
    }
}
