//! Instruction set for the organism IR.

use serde::{Deserialize, Serialize};

/// Register identifier (local variable)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Register(pub u8);

/// Immediate value
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i32),
    Float(f32),
    Bool(bool),
}

impl Value {
    pub fn as_i32(&self) -> i32 {
        match self {
            Value::Int(v) => *v,
            Value::Float(v) => *v as i32,
            Value::Bool(v) => *v as i32,
        }
    }

    pub fn as_f32(&self) -> f32 {
        match self {
            Value::Int(v) => *v as f32,
            Value::Float(v) => *v,
            Value::Bool(v) => *v as i32 as f32,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            Value::Bool(v) => *v,
        }
    }
}

/// IR Opcode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Opcode {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Abs,
    Min,
    Max,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,
    Not,
    Xor,

    // Memory
    Load,
    Store,
    LoadConst,

    // Control flow
    Branch,       // Unconditional jump
    BranchIf,     // Conditional jump
    Call,         // Function call
    Return,       // Return from function

    // Host calls (organism ABI)
    SenseEnv,     // Read environment
    SenseNeighbor,// Sense neighbor
    GetEnergy,    // Get current energy
    GetAge,       // Get age
    Move,         // Move in direction
    Eat,          // Eat resource
    Attack,       // Attack neighbor
    Reproduce,    // Try to reproduce
    EmitSignal,   // Emit signal
}

impl Opcode {
    /// Returns true if this opcode is a control flow instruction
    pub fn is_control_flow(&self) -> bool {
        matches!(
            self,
            Opcode::Branch | Opcode::BranchIf | Opcode::Call | Opcode::Return
        )
    }

    /// Returns true if this opcode is a host call (organism ABI)
    pub fn is_host_call(&self) -> bool {
        matches!(
            self,
            Opcode::SenseEnv
                | Opcode::SenseNeighbor
                | Opcode::GetEnergy
                | Opcode::GetAge
                | Opcode::Move
                | Opcode::Eat
                | Opcode::Attack
                | Opcode::Reproduce
                | Opcode::EmitSignal
        )
    }

    /// Returns the number of operands this opcode expects
    pub fn num_operands(&self) -> usize {
        match self {
            // Unary operations
            Opcode::Neg | Opcode::Not | Opcode::Abs => 1,
            // Binary operations
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div | Opcode::Mod => 2,
            Opcode::Eq | Opcode::Ne | Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => 2,
            Opcode::And | Opcode::Or | Opcode::Xor => 2,
            Opcode::Min | Opcode::Max => 2,
            // Memory operations
            Opcode::Load | Opcode::Store => 1,
            Opcode::LoadConst => 0,
            // Control flow
            Opcode::Branch => 0,
            Opcode::BranchIf => 1,
            Opcode::Call => 0, // Variable
            Opcode::Return => 0,
            // Host calls
            Opcode::SenseEnv => 2,       // x, y
            Opcode::SenseNeighbor => 1,  // slot
            Opcode::GetEnergy => 0,
            Opcode::GetAge => 0,
            Opcode::Move => 2,           // dx, dy
            Opcode::Eat => 0,
            Opcode::Attack => 2,         // slot, amount
            Opcode::Reproduce => 0,
            Opcode::EmitSignal => 2,     // channel, value
        }
    }
}

/// A single instruction in the IR
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    pub opcode: Opcode,
    pub dest: Option<Register>,
    pub operands: Vec<Operand>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operand {
    Register(Register),
    Immediate(Value),
    BlockIndex(u32),
    FunctionIndex(u32),
}

impl Instruction {
    pub fn new(opcode: Opcode) -> Self {
        Self {
            opcode,
            dest: None,
            operands: Vec::new(),
        }
    }

    pub fn with_dest(mut self, reg: Register) -> Self {
        self.dest = Some(reg);
        self
    }

    pub fn with_operand(mut self, operand: Operand) -> Self {
        self.operands.push(operand);
        self
    }

    pub fn with_operands(mut self, operands: Vec<Operand>) -> Self {
        self.operands = operands;
        self
    }

    /// Create a simple arithmetic instruction
    pub fn arithmetic(opcode: Opcode, dest: Register, a: Register, b: Register) -> Self {
        Self {
            opcode,
            dest: Some(dest),
            operands: vec![Operand::Register(a), Operand::Register(b)],
        }
    }

    /// Create a load constant instruction
    pub fn load_const(dest: Register, value: Value) -> Self {
        Self {
            opcode: Opcode::LoadConst,
            dest: Some(dest),
            operands: vec![Operand::Immediate(value)],
        }
    }

    /// Create a branch instruction
    pub fn branch(block: u32) -> Self {
        Self {
            opcode: Opcode::Branch,
            dest: None,
            operands: vec![Operand::BlockIndex(block)],
        }
    }

    /// Create a conditional branch instruction
    pub fn branch_if(condition: Register, block: u32) -> Self {
        Self {
            opcode: Opcode::BranchIf,
            dest: None,
            operands: vec![Operand::Register(condition), Operand::BlockIndex(block)],
        }
    }

    /// Create a return instruction
    pub fn return_void() -> Self {
        Self {
            opcode: Opcode::Return,
            dest: None,
            operands: Vec::new(),
        }
    }

    pub fn return_value(reg: Register) -> Self {
        Self {
            opcode: Opcode::Return,
            dest: None,
            operands: vec![Operand::Register(reg)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversions() {
        let v = Value::Int(42);
        assert_eq!(v.as_i32(), 42);
        assert_eq!(v.as_f32(), 42.0);
        assert!(v.as_bool());

        let v = Value::Bool(false);
        assert_eq!(v.as_i32(), 0);
        assert!(!v.as_bool());
    }

    #[test]
    fn test_opcode_properties() {
        assert!(Opcode::Branch.is_control_flow());
        assert!(!Opcode::Add.is_control_flow());

        assert!(Opcode::GetEnergy.is_host_call());
        assert!(!Opcode::Add.is_host_call());

        assert_eq!(Opcode::Add.num_operands(), 2);
        assert_eq!(Opcode::Not.num_operands(), 1);
        assert_eq!(Opcode::GetEnergy.num_operands(), 0);
    }

    #[test]
    fn test_instruction_builders() {
        let inst = Instruction::arithmetic(Opcode::Add, Register(0), Register(1), Register(2));
        assert_eq!(inst.opcode, Opcode::Add);
        assert_eq!(inst.dest, Some(Register(0)));
        assert_eq!(inst.operands.len(), 2);

        let inst = Instruction::load_const(Register(0), Value::Int(42));
        assert_eq!(inst.opcode, Opcode::LoadConst);
        assert_eq!(inst.dest, Some(Register(0)));

        let inst = Instruction::branch_if(Register(0), 1);
        assert_eq!(inst.opcode, Opcode::BranchIf);
        assert_eq!(inst.operands.len(), 2);
    }
}
