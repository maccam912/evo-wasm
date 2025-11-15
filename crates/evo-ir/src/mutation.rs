//! Mutation operators for IR programs.

use crate::instruction::{Instruction, Opcode, Operand, Register, Value};
use crate::program::{BasicBlock, Function, Program, ReturnType};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationConfig {
    /// Probability of point mutation per instruction
    pub point_mutation_rate: f32,
    /// Probability of adding a new instruction
    pub insertion_rate: f32,
    /// Probability of deleting an instruction
    pub deletion_rate: f32,
    /// Probability of duplicating a basic block
    pub block_duplication_rate: f32,
    /// Probability of adding a new function
    pub function_addition_rate: f32,
    /// Maximum number of instructions per function
    pub max_instructions_per_function: usize,
    /// Maximum number of functions per program
    pub max_functions: usize,
    /// Maximum number of local variables
    pub max_locals: usize,
}

impl Default for MutationConfig {
    fn default() -> Self {
        Self {
            point_mutation_rate: 0.01,
            insertion_rate: 0.005,
            deletion_rate: 0.005,
            block_duplication_rate: 0.001,
            function_addition_rate: 0.0001,
            max_instructions_per_function: 100,
            max_functions: 10,
            max_locals: 16,
        }
    }
}

pub struct Mutator {
    config: MutationConfig,
}

impl Mutator {
    pub fn new(config: MutationConfig) -> Self {
        Self { config }
    }

    /// Mutate a program in place
    pub fn mutate(&self, program: &mut Program, rng: &mut ChaCha8Rng) {
        // Mutate each function
        for func in &mut program.functions {
            self.mutate_function(func, rng);
        }

        // Maybe add a new function
        if rng.gen::<f32>() < self.config.function_addition_rate
            && program.num_functions() < self.config.max_functions
        {
            let new_func = self.generate_random_function(rng);
            program.add_function(new_func);
        }
    }

    fn mutate_function(&self, func: &mut Function, rng: &mut ChaCha8Rng) {
        for block in &mut func.blocks {
            self.mutate_block(block, rng);
        }

        // Maybe add a new block
        if rng.gen::<f32>() < self.config.block_duplication_rate && !func.blocks.is_empty() {
            let block_to_dup = rng.gen_range(0..func.blocks.len());
            let new_block = func.blocks[block_to_dup].clone();
            func.add_block(new_block);
        }

        // Maybe add a new local variable
        if rng.gen::<f32>() < 0.01 && func.num_locals < self.config.max_locals {
            func.num_locals += 1;
        }
    }

    fn mutate_block(&self, block: &mut BasicBlock, rng: &mut ChaCha8Rng) {
        let mut i = 0;
        while i < block.instructions.len() {
            // Point mutation
            if rng.gen::<f32>() < self.config.point_mutation_rate {
                self.point_mutate(&mut block.instructions[i], rng);
            }

            // Deletion
            if rng.gen::<f32>() < self.config.deletion_rate && block.instructions.len() > 1 {
                block.instructions.remove(i);
                continue;
            }

            // Insertion
            if rng.gen::<f32>() < self.config.insertion_rate
                && block.instructions.len() < self.config.max_instructions_per_function
            {
                let new_inst = self.generate_random_instruction(rng);
                block.instructions.insert(i, new_inst);
                i += 1;
            }

            i += 1;
        }
    }

    fn point_mutate(&self, inst: &mut Instruction, rng: &mut ChaCha8Rng) {
        match rng.gen_range(0..3) {
            0 => {
                // Mutate opcode (within same category)
                inst.opcode = self.mutate_opcode(inst.opcode, rng);
            }
            1 => {
                // Mutate operands
                for operand in &mut inst.operands {
                    if rng.gen::<f32>() < 0.5 {
                        self.mutate_operand(operand, rng);
                    }
                }
            }
            2 => {
                // Mutate destination register
                if let Some(ref mut dest) = inst.dest {
                    dest.0 = rng.gen_range(0..16);
                }
            }
            _ => {}
        }
    }

    fn mutate_opcode(&self, opcode: Opcode, rng: &mut ChaCha8Rng) -> Opcode {
        // Mutate to a similar opcode
        match opcode {
            // Binary arithmetic ops
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div | Opcode::Mod => {
                *[Opcode::Add, Opcode::Sub, Opcode::Mul, Opcode::Div, Opcode::Mod]
                    .iter()
                    .nth(rng.gen_range(0..5))
                    .unwrap()
            }
            // Unary math ops
            Opcode::Neg | Opcode::Abs => {
                *[Opcode::Neg, Opcode::Abs]
                    .iter()
                    .nth(rng.gen_range(0..2))
                    .unwrap()
            }
            // Min/Max ops
            Opcode::Min | Opcode::Max => {
                *[Opcode::Min, Opcode::Max]
                    .iter()
                    .nth(rng.gen_range(0..2))
                    .unwrap()
            }
            // Comparison ops
            Opcode::Eq | Opcode::Ne | Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => {
                *[
                    Opcode::Eq,
                    Opcode::Ne,
                    Opcode::Lt,
                    Opcode::Le,
                    Opcode::Gt,
                    Opcode::Ge,
                ]
                .iter()
                .nth(rng.gen_range(0..6))
                .unwrap()
            }
            // Binary logical ops (2 operands)
            Opcode::And | Opcode::Or | Opcode::Xor => {
                *[Opcode::And, Opcode::Or, Opcode::Xor]
                    .iter()
                    .nth(rng.gen_range(0..3))
                    .unwrap()
            }
            // Note: Not is unary (1 operand) so it doesn't mutate with binary logical ops
            // Note: SenseEnv (2 operands) and SenseNeighbor (1 operand) have different arities,
            // so they are left unchanged to avoid creating invalid instructions
            // Energy reading ops
            Opcode::GetEnergy | Opcode::GetAge => {
                *[Opcode::GetEnergy, Opcode::GetAge]
                    .iter()
                    .nth(rng.gen_range(0..2))
                    .unwrap()
            }
            // Leave action opcodes (Move, Eat, Attack, Reproduce, EmitSignal) unchanged
            // to preserve their specific behaviors
            _ => opcode,
        }
    }

    fn mutate_operand(&self, operand: &mut Operand, rng: &mut ChaCha8Rng) {
        match operand {
            Operand::Register(ref mut reg) => {
                reg.0 = rng.gen_range(0..16);
            }
            Operand::Immediate(ref mut val) => match val {
                Value::Int(ref mut v) => {
                    // Small mutation to the value
                    let delta = rng.gen_range(-10..=10);
                    *v = v.saturating_add(delta);
                }
                Value::Float(ref mut v) => {
                    let delta = rng.gen_range(-1.0..=1.0);
                    *v += delta;
                }
                Value::Bool(ref mut v) => {
                    if rng.gen::<f32>() < 0.5 {
                        *v = !*v;
                    }
                }
            },
            Operand::BlockIndex(ref mut idx) => {
                // Random block index (bounded by validation later)
                *idx = rng.gen_range(0..8);
            }
            Operand::FunctionIndex(ref mut idx) => {
                *idx = rng.gen_range(0..8);
            }
        }
    }

    fn generate_random_instruction(&self, rng: &mut ChaCha8Rng) -> Instruction {
        let opcodes = [
            // Arithmetic
            Opcode::Add,
            Opcode::Sub,
            Opcode::Mul,
            Opcode::Div,
            Opcode::Mod,
            Opcode::Neg,
            Opcode::Abs,
            Opcode::Min,
            Opcode::Max,
            // Comparison
            Opcode::Eq,
            Opcode::Ne,
            Opcode::Lt,
            Opcode::Le,
            Opcode::Gt,
            Opcode::Ge,
            // Logical
            Opcode::And,
            Opcode::Or,
            Opcode::Xor,
            Opcode::Not,
            // Constants
            Opcode::LoadConst,
            // Host calls
            Opcode::GetEnergy,
            Opcode::GetAge,
            Opcode::Move,
            Opcode::Eat,
            Opcode::SenseEnv,
            Opcode::SenseNeighbor,
            Opcode::Attack,
            Opcode::Reproduce,
            Opcode::EmitSignal,
        ];

        let opcode = *opcodes.iter().nth(rng.gen_range(0..opcodes.len())).unwrap();

        match opcode {
            // Binary arithmetic/comparison/logical operations
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div | Opcode::Mod
            | Opcode::Eq | Opcode::Ne | Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge
            | Opcode::And | Opcode::Or | Opcode::Xor
            | Opcode::Min | Opcode::Max => Instruction::arithmetic(
                opcode,
                Register(rng.gen_range(0..8)),
                Register(rng.gen_range(0..8)),
                Register(rng.gen_range(0..8)),
            ),
            // Unary operations
            Opcode::Not | Opcode::Neg | Opcode::Abs => Instruction::new(opcode)
                .with_operand(Operand::Register(Register(rng.gen_range(0..8))))
                .with_dest(Register(rng.gen_range(0..8))),
            // Load constant
            Opcode::LoadConst => Instruction::load_const(
                Register(rng.gen_range(0..8)),
                Value::Int(rng.gen_range(-100..100)),
            ),
            // No-parameter host calls
            Opcode::GetEnergy | Opcode::GetAge | Opcode::Eat =>
                Instruction::new(opcode).with_dest(Register(rng.gen_range(0..8))),
            // Move: 2 direction parameters (dx, dy)
            Opcode::Move => Instruction::new(opcode)
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-1..=1))))
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-1..=1)))),
            // SenseEnv: 2 direction parameters (dx, dy)
            Opcode::SenseEnv => Instruction::new(opcode)
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-1..=1))))
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-1..=1))))
                .with_dest(Register(rng.gen_range(0..8))),
            // SenseNeighbor: 1 direction parameter
            Opcode::SenseNeighbor => Instruction::new(opcode)
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(0..8))))
                .with_dest(Register(rng.gen_range(0..8))),
            // Attack: 2 direction parameters (dx, dy)
            Opcode::Attack => Instruction::new(opcode)
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-1..=1))))
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-1..=1))))
                .with_dest(Register(rng.gen_range(0..8))),
            // Reproduce: no parameters
            Opcode::Reproduce => Instruction::new(opcode)
                .with_dest(Register(rng.gen_range(0..8))),
            // EmitSignal: 2 parameters (signal type, value)
            Opcode::EmitSignal => Instruction::new(opcode)
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(0..10))))
                .with_operand(Operand::Immediate(Value::Int(rng.gen_range(-100..100)))),
            _ => Instruction::new(opcode),
        }
    }

    fn generate_random_function(&self, rng: &mut ChaCha8Rng) -> Function {
        let name = format!("func_{}", rng.gen::<u32>());
        let mut func = Function::new(name, 0, ReturnType::Int);

        let num_instructions = rng.gen_range(3..20);
        for _ in 0..num_instructions {
            let inst = self.generate_random_instruction(rng);
            func.get_block_mut(0).unwrap().add_instruction(inst);
        }

        // Add return instruction
        func.get_block_mut(0)
            .unwrap()
            .add_instruction(Instruction::return_value(Register(0)));

        func
    }

    /// Perform crossover between two programs
    pub fn crossover(
        &self,
        parent1: &Program,
        parent2: &Program,
        rng: &mut ChaCha8Rng,
    ) -> Program {
        let mut child = Program::new();

        // Take functions from both parents
        let num_funcs = (parent1.num_functions() + parent2.num_functions()) / 2;

        for i in 0..num_funcs.min(self.config.max_functions) {
            let func = if rng.gen::<bool>() {
                if i < parent1.num_functions() {
                    parent1.functions[i].clone()
                } else {
                    parent2.functions[i % parent2.num_functions()].clone()
                }
            } else {
                if i < parent2.num_functions() {
                    parent2.functions[i].clone()
                } else {
                    parent1.functions[i % parent1.num_functions()].clone()
                }
            };

            child.add_function(func);
        }

        child
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_point_mutation() {
        let mutator = Mutator::new(MutationConfig::default());
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let mut inst = Instruction::arithmetic(
            Opcode::Add,
            Register(0),
            Register(1),
            Register(2),
        );

        mutator.point_mutate(&mut inst, &mut rng);
        // Instruction should still be valid
        assert!(inst.operands.len() >= 2);
    }

    #[test]
    fn test_mutate_program() {
        let mutator = Mutator::new(MutationConfig {
            point_mutation_rate: 1.0, // Force mutation
            ..Default::default()
        });
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let mut func = Function::new("test".to_string(), 0, ReturnType::Int);
        func.get_block_mut(0).unwrap().add_instruction(
            Instruction::load_const(Register(0), Value::Int(42))
        );
        func.get_block_mut(0).unwrap().add_instruction(
            Instruction::return_value(Register(0))
        );

        let mut program = Program::new();
        program.add_function(func);

        let original_instruction_count = program.total_instructions();

        mutator.mutate(&mut program, &mut rng);

        // Program should still have functions
        assert!(program.num_functions() > 0);
    }

    #[test]
    fn test_crossover() {
        let mutator = Mutator::new(MutationConfig::default());
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let mut parent1 = Program::new();
        parent1.add_function(Function::new("func1".to_string(), 0, ReturnType::Int));

        let mut parent2 = Program::new();
        parent2.add_function(Function::new("func2".to_string(), 0, ReturnType::Int));

        let child = mutator.crossover(&parent1, &parent2, &mut rng);

        assert!(child.num_functions() > 0);
    }

    #[test]
    fn test_generate_random_instruction() {
        let mutator = Mutator::new(MutationConfig::default());
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..10 {
            let inst = mutator.generate_random_instruction(&mut rng);
            // Just ensure it doesn't crash
            assert!(inst.operands.len() <= 3);
        }
    }
}
