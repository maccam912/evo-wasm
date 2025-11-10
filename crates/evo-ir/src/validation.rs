//! Validation for IR programs.

use crate::program::Program;
use evo_core::{Error, Result};

/// Validate that a program is well-formed
pub fn validate_program(program: &Program) -> Result<()> {
    // Check that required functions exist
    if program.get_init_function().is_none() {
        return Err(Error::Validation("Missing required 'init' function".to_string()));
    }

    if program.get_step_function().is_none() {
        return Err(Error::Validation("Missing required 'step' function".to_string()));
    }

    // Validate each function
    for (idx, func) in program.functions.iter().enumerate() {
        validate_function(func, idx)?;
    }

    Ok(())
}

fn validate_function(func: &crate::program::Function, idx: usize) -> Result<()> {
    // Check that function has at least one block
    if func.blocks.is_empty() {
        return Err(Error::Validation(format!(
            "Function {} has no basic blocks",
            idx
        )));
    }

    // Check that blocks are non-empty (except possibly the last one)
    for (block_idx, block) in func.blocks.iter().enumerate() {
        if block.is_empty() && block_idx < func.blocks.len() - 1 {
            return Err(Error::Validation(format!(
                "Function {} block {} is empty",
                idx, block_idx
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::{Instruction, Register, Value};
    use crate::program::{Function, Program, ReturnType};

    #[test]
    fn test_validate_empty_program() {
        let program = Program::new();
        assert!(validate_program(&program).is_err());
    }

    #[test]
    fn test_validate_missing_step() {
        let mut program = Program::new();
        let mut func = Function::new("init".to_string(), 1, ReturnType::Void);
        func.get_block_mut(0).unwrap().add_instruction(Instruction::return_void());
        program.add_function(func);

        assert!(validate_program(&program).is_err());
    }

    #[test]
    fn test_validate_valid_program() {
        let mut program = Program::new();

        // Add init function
        let mut init = Function::new("init".to_string(), 1, ReturnType::Void);
        init.get_block_mut(0).unwrap().add_instruction(Instruction::return_void());
        program.add_function(init);

        // Add step function
        let mut step = Function::new("step".to_string(), 1, ReturnType::Int);
        step.get_block_mut(0).unwrap().add_instruction(
            Instruction::load_const(Register(0), Value::Int(0))
        );
        step.get_block_mut(0).unwrap().add_instruction(
            Instruction::return_value(Register(0))
        );
        program.add_function(step);

        assert!(validate_program(&program).is_ok());
    }
}
