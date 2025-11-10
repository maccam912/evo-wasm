//! Compiler from IR to WebAssembly.

use crate::instruction::{Instruction, Opcode, Operand, Value};
use crate::program::{Function, Program, ReturnType};
use evo_core::Error;
use wasm_encoder::*;

pub struct Compiler {
    config: CompilerConfig,
}

#[derive(Debug, Clone)]
pub struct CompilerConfig {
    pub max_memory_pages: u32,
    pub import_memory: bool,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            max_memory_pages: 1, // 64 KiB
            import_memory: false,
        }
    }
}

impl Compiler {
    pub fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    pub fn compile(&self, program: &Program) -> Result<Vec<u8>, Error> {
        let mut module = Module::new();

        // Type section: define function signatures
        let mut types = TypeSection::new();

        // init function: (param i64) -> void
        types.function([ValType::I64], []);

        // step function: (param i32) -> i32
        types.function([ValType::I32], [ValType::I32]);

        // Host import signatures
        self.add_host_import_types(&mut types);

        module.section(&types);

        // Import section: host functions
        let mut imports = ImportSection::new();
        self.add_host_imports(&mut imports);
        module.section(&imports);

        // Function section: declare our functions
        let mut functions = FunctionSection::new();
        for func in &program.functions {
            let type_idx = self.get_function_type_index(func);
            functions.function(type_idx);
        }
        module.section(&functions);

        // Memory section
        let mut memories = MemorySection::new();
        memories.memory(MemoryType {
            minimum: 1,
            maximum: Some(self.config.max_memory_pages as u64),
            memory64: false,
            shared: false,
        });
        module.section(&memories);

        // Export section: export init and step functions
        let mut exports = ExportSection::new();
        let num_imports = self.num_host_imports();

        for (i, func) in program.functions.iter().enumerate() {
            let func_idx = num_imports + i as u32;
            exports.export(&func.name, ExportKind::Func, func_idx);
        }
        exports.export("memory", ExportKind::Memory, 0);
        module.section(&exports);

        // Code section: function bodies
        let mut code = CodeSection::new();
        for func in &program.functions {
            let func_body = self.compile_function(func)?;
            code.function(&func_body);
        }
        module.section(&code);

        Ok(module.finish())
    }

    fn compile_function(&self, func: &Function) -> Result<wasm_encoder::Function, Error> {
        use wasm_encoder::Instruction as WI;

        // Declare local variables properly
        let mut locals = vec![];
        if func.num_locals > 0 {
            locals.push((func.num_locals as u32, ValType::I32));
        }
        let mut wasm_func = wasm_encoder::Function::new(locals);

        // Compile each basic block
        for (block_idx, block) in func.blocks.iter().enumerate() {
            // Add block label
            if block_idx > 0 {
                // For non-entry blocks, we need to handle jumps
                // This is simplified - a real implementation would use proper control flow
            }

            for inst in &block.instructions {
                self.compile_instruction(&mut wasm_func, inst)?;
            }
        }

        // Add END instruction to close the function body
        wasm_func.instruction(&WI::End);

        Ok(wasm_func)
    }

    fn compile_instruction(
        &self,
        wasm_func: &mut wasm_encoder::Function,
        inst: &Instruction,
    ) -> Result<(), Error> {
        use wasm_encoder::Instruction as WI;

        match inst.opcode {
            // Arithmetic operations
            Opcode::Add => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Add);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Sub => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Sub);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Mul => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Mul);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Div => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32DivS);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Mod => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32RemS);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }

            // Comparison operations
            Opcode::Eq => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Eq);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Ne => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Ne);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Lt => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32LtS);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Le => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32LeS);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Gt => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32GtS);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Ge => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32GeS);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }

            // Logical operations
            Opcode::And => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32And);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Or => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Or);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Xor => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Xor);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Not => {
                self.load_operands(wasm_func, &inst.operands)?;
                // Logical NOT: x == 0 ? 1 : 0
                wasm_func.instruction(&WI::I32Eqz);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }

            // Unary arithmetic operations
            Opcode::Neg => {
                // Negate: 0 - x
                wasm_func.instruction(&WI::I32Const(0));
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::I32Sub);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Abs => {
                self.load_operands(wasm_func, &inst.operands)?;
                // Absolute value using local variable
                let temp_local = 0; // Use first local as temp
                wasm_func.instruction(&WI::LocalTee(temp_local));
                wasm_func.instruction(&WI::I32Const(31));
                wasm_func.instruction(&WI::I32ShrS); // Sign bit
                wasm_func.instruction(&WI::LocalGet(temp_local));
                wasm_func.instruction(&WI::I32Xor);
                wasm_func.instruction(&WI::LocalGet(temp_local));
                wasm_func.instruction(&WI::I32Const(31));
                wasm_func.instruction(&WI::I32ShrS);
                wasm_func.instruction(&WI::I32Sub);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Min => {
                self.load_operands(wasm_func, &inst.operands)?;
                // Min: a < b ? a : b
                // Stack: a b
                let temp_a = 0;
                let temp_b = 1;
                wasm_func.instruction(&WI::LocalSet(temp_b));
                wasm_func.instruction(&WI::LocalSet(temp_a));
                wasm_func.instruction(&WI::LocalGet(temp_a));
                wasm_func.instruction(&WI::LocalGet(temp_b));
                wasm_func.instruction(&WI::LocalGet(temp_a));
                wasm_func.instruction(&WI::LocalGet(temp_b));
                wasm_func.instruction(&WI::I32LtS);
                wasm_func.instruction(&WI::Select);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Max => {
                self.load_operands(wasm_func, &inst.operands)?;
                // Max: a > b ? a : b
                let temp_a = 0;
                let temp_b = 1;
                wasm_func.instruction(&WI::LocalSet(temp_b));
                wasm_func.instruction(&WI::LocalSet(temp_a));
                wasm_func.instruction(&WI::LocalGet(temp_a));
                wasm_func.instruction(&WI::LocalGet(temp_b));
                wasm_func.instruction(&WI::LocalGet(temp_a));
                wasm_func.instruction(&WI::LocalGet(temp_b));
                wasm_func.instruction(&WI::I32GtS);
                wasm_func.instruction(&WI::Select);
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }

            // Load constant
            Opcode::LoadConst => {
                if let Some(Operand::Immediate(value)) = inst.operands.first() {
                    wasm_func.instruction(&WI::I32Const(value.as_i32()));
                    if let Some(dest) = inst.dest {
                        wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                    }
                }
            }

            // Control flow
            Opcode::Return => {
                if !inst.operands.is_empty() {
                    self.load_operands(wasm_func, &inst.operands)?;
                }
                wasm_func.instruction(&WI::Return);
            }

            // Host calls - these call imported functions
            Opcode::GetEnergy => {
                wasm_func.instruction(&WI::Call(self.get_import_index("get_energy")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::GetAge => {
                wasm_func.instruction(&WI::Call(self.get_import_index("get_age")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Move => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::Call(self.get_import_index("move_dir")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Eat => {
                wasm_func.instruction(&WI::Call(self.get_import_index("eat")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::SenseEnv => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::Call(self.get_import_index("env_read")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::SenseNeighbor => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::Call(self.get_import_index("sense_neighbor")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Attack => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::Call(self.get_import_index("attack")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::Reproduce => {
                wasm_func.instruction(&WI::Call(self.get_import_index("try_reproduce")));
                if let Some(dest) = inst.dest {
                    wasm_func.instruction(&WI::LocalSet(dest.0 as u32));
                }
            }
            Opcode::EmitSignal => {
                self.load_operands(wasm_func, &inst.operands)?;
                wasm_func.instruction(&WI::Call(self.get_import_index("emit_signal")));
                // emit_signal returns void, no destination
            }

            // Control flow and memory operations
            Opcode::Branch | Opcode::BranchIf | Opcode::Call | Opcode::Load | Opcode::Store => {
                // These require more complex control flow structures not yet implemented
                // For now, generate a NOP (no operation) to keep WASM valid
                // These opcodes are not generated by the current mutator anyway
                tracing::warn!("Opcode {:?} requires control flow not yet implemented", inst.opcode);
            }
        }

        Ok(())
    }

    fn load_operands(
        &self,
        wasm_func: &mut wasm_encoder::Function,
        operands: &[Operand],
    ) -> Result<(), Error> {
        use wasm_encoder::Instruction as WI;

        for operand in operands {
            match operand {
                Operand::Register(reg) => {
                    wasm_func.instruction(&WI::LocalGet(reg.0 as u32));
                }
                Operand::Immediate(value) => {
                    wasm_func.instruction(&WI::I32Const(value.as_i32()));
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn add_host_import_types(&self, types: &mut TypeSection) {
        // env_read: (i32, i32) -> i32
        types.function([ValType::I32, ValType::I32], [ValType::I32]);
        // get_energy: () -> i32
        types.function([], [ValType::I32]);
        // get_age: () -> i32
        types.function([], [ValType::I32]);
        // move_dir: (i32, i32) -> i32
        types.function([ValType::I32, ValType::I32], [ValType::I32]);
        // eat: () -> i32
        types.function([], [ValType::I32]);
        // attack: (i32, i32) -> i32
        types.function([ValType::I32, ValType::I32], [ValType::I32]);
        // sense_neighbor: (i32) -> i32
        types.function([ValType::I32], [ValType::I32]);
        // try_reproduce: () -> i32
        types.function([], [ValType::I32]);
        // emit_signal: (i32, i32) -> void
        types.function([ValType::I32, ValType::I32], []);
    }

    fn add_host_imports(&self, imports: &mut ImportSection) {
        let host_imports = [
            ("env_read", 2),
            ("get_energy", 3),
            ("get_age", 4),
            ("move_dir", 5),
            ("eat", 6),
            ("attack", 7),
            ("sense_neighbor", 8),
            ("try_reproduce", 9),
            ("emit_signal", 10),
        ];

        for (name, type_idx) in host_imports {
            imports.import("env", name, EntityType::Function(type_idx));
        }
    }

    fn num_host_imports(&self) -> u32 {
        9 // Number of host imports
    }

    fn get_import_index(&self, name: &str) -> u32 {
        match name {
            "env_read" => 0,
            "get_energy" => 1,
            "get_age" => 2,
            "move_dir" => 3,
            "eat" => 4,
            "attack" => 5,
            "sense_neighbor" => 6,
            "try_reproduce" => 7,
            "emit_signal" => 8,
            _ => 0,
        }
    }

    fn get_function_type_index(&self, func: &Function) -> u32 {
        // Map function signatures to type indices
        match func.name.as_str() {
            "init" => 0,
            "step" => 1,
            _ => 1, // Default to step signature
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::{Instruction, Opcode, Register};
    use crate::program::{BasicBlock, Function, Program};

    #[test]
    fn test_compile_empty_program() {
        let compiler = Compiler::new(CompilerConfig::default());
        let program = Program::new();

        // Empty program won't compile properly, but let's test the structure
        assert_eq!(program.num_functions(), 0);
    }

    #[test]
    fn test_compile_simple_function() {
        let compiler = Compiler::new(CompilerConfig::default());

        let mut func = Function::new("test".to_string(), 0, ReturnType::Int);
        func.get_block_mut(0).unwrap().add_instruction(
            Instruction::load_const(Register(0), Value::Int(42))
        );
        func.get_block_mut(0).unwrap().add_instruction(
            Instruction::return_value(Register(0))
        );

        let mut program = Program::new();
        program.add_function(func);

        let wasm_bytes = compiler.compile(&program);
        assert!(wasm_bytes.is_ok());
    }

    #[test]
    fn test_compile_init_and_step() {
        let compiler = Compiler::new(CompilerConfig::default());

        let mut program = Program::new();

        // Init function
        let mut init = Function::new("init".to_string(), 1, ReturnType::Void);
        init.get_block_mut(0).unwrap().add_instruction(
            Instruction::return_void()
        );
        program.add_function(init);

        // Step function
        let mut step = Function::new("step".to_string(), 1, ReturnType::Int);
        step.get_block_mut(0).unwrap().add_instruction(
            Instruction::load_const(Register(0), Value::Int(0))
        );
        step.get_block_mut(0).unwrap().add_instruction(
            Instruction::return_value(Register(0))
        );
        program.add_function(step);

        let wasm_bytes = compiler.compile(&program);
        assert!(wasm_bytes.is_ok(), "Compilation failed: {:?}", wasm_bytes.err());

        let bytes = wasm_bytes.unwrap();

        // Try to disassemble with wabt to validate
        match wabt::wasm2wat(&bytes) {
            Ok(wat) => {
                println!("Generated WAT:\n{}", wat);
            },
            Err(e) => panic!("WASM validation/disassembly failed: {:?}", e),
        }
    }
}
