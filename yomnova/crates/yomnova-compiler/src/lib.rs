//! YOMNOVA Multi-Stage Compiler Driver and Bytecode Generator.

use std::collections::HashMap;
use yomnova_ast::*;
use yomnova_bytecode::{Chunk, OpCode, Value};
use yomnova_diagnostics::Diagnostic;
use yomnova_lexer::Lexer;
use yomnova_ownership::OwnershipChecker;
use yomnova_parser::Parser;
use yomnova_sema::SemanticAnalyzer;
use yomnova_vm::Vm;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompilerError {
    #[error("Lexer error: {0}")]
    Lexer(String),
    #[error("Parser error: {0}")]
    Parser(String),
    #[error("Semantic error: {0}")]
    Semantic(String),
    #[error("Ownership error: {0}")]
    Ownership(String),
    #[error("Codegen error: {0}")]
    Codegen(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
}

pub struct CompilerOutput {
    pub ast: Program,
    pub chunk: Chunk,
    pub execution_result: Option<Value>,
    pub output_lines: Vec<String>,
}

pub struct Compiler {
    locals: Vec<HashMap<String, u16>>,
    next_slot: u16,
    chunk: Chunk,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            locals: vec![HashMap::new()],
            next_slot: 0,
            chunk: Chunk::new(),
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {

    /// Complete pipeline: Lex -> Parse -> Semantic Analysis -> Ownership Check -> Bytecode Gen -> Run in VM
    pub fn compile_and_run(source: &str) -> Result<CompilerOutput, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // 1. Lexing
        let mut lexer = Lexer::new(source);
        let tokens = match lexer.tokenize() {
            Ok(tokens) => tokens,
            Err(e) => {
                diagnostics.push(e.to_diagnostic());
                return Err(diagnostics);
            }
        };

        // 2. Parsing
        let mut parser = Parser::new(tokens);
        let program = match parser.parse_program() {
            Ok(prog) => prog,
            Err(e) => {
                diagnostics.push(e.to_diagnostic());
                return Err(diagnostics);
            }
        };

        // 3. Semantic Analysis & Type Checking
        let mut sema = SemanticAnalyzer::new();
        if let Err(e) = sema.analyze_program(&program) {
            diagnostics.push(e.to_diagnostic());
            return Err(diagnostics);
        }

        // 4. Ownership Checking
        let mut ownership = OwnershipChecker::new();
        if let Err(e) = ownership.check_program(&program) {
            diagnostics.push(e.to_diagnostic());
            return Err(diagnostics);
        }

        // 5. Virtual Machine Setup & User Function Compilation
        let mut vm = Vm::new();

        for item in &program.items {
            if let TopLevelItem::Function(f) = item {
                if f.name != "main" {
                    let mut fn_compiler = Compiler::new();
                    for p in &f.params {
                        fn_compiler.declare_local(&p.name);
                    }
                    for stmt in &f.body.statements {
                        if let Err(e) = fn_compiler.compile_statement(stmt) {
                            diagnostics.push(Diagnostic::error(e.to_string(), None));
                            return Err(diagnostics);
                        }
                    }
                    fn_compiler.chunk.write_op(OpCode::Nil, 0);
                    fn_compiler.chunk.write_op(OpCode::Return, 0);
                    let chunk_idx = vm.chunks.len();
                    vm.chunks.push(fn_compiler.chunk);
                    vm.globals.insert(
                        f.name.clone(),
                        Value::UserFunction {
                            name: f.name.clone(),
                            chunk_index: chunk_idx,
                            arity: f.params.len(),
                        },
                    );
                }
            }
        }

        // 6. Bytecode Generation for Main / Top-Level
        let mut compiler = Compiler::new();
        let chunk = match compiler.generate_bytecode(&program) {
            Ok(c) => c,
            Err(e) => {
                diagnostics.push(Diagnostic::error(e.to_string(), None));
                return Err(diagnostics);
            }
        };

        // 7. Virtual Machine Execution
        let exec_result = match vm.run_chunk(chunk.clone()) {
            Ok(val) => Some(val),
            Err(e) => {
                diagnostics.push(Diagnostic::error(format!("Runtime error: {e}"), None));
                return Err(diagnostics);
            }
        };

        Ok(CompilerOutput {
            ast: program,
            chunk,
            execution_result: exec_result,
            output_lines: vm.output,
        })
    }

    /// Verification only (check mode): Lex -> Parse -> Semantic Analysis -> Ownership Check
    pub fn check(source: &str) -> Result<Program, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        let mut lexer = Lexer::new(source);
        let tokens = match lexer.tokenize() {
            Ok(tokens) => tokens,
            Err(e) => {
                diagnostics.push(e.to_diagnostic());
                return Err(diagnostics);
            }
        };

        let mut parser = Parser::new(tokens);
        let program = match parser.parse_program() {
            Ok(prog) => prog,
            Err(e) => {
                diagnostics.push(e.to_diagnostic());
                return Err(diagnostics);
            }
        };

        let mut sema = SemanticAnalyzer::new();
        if let Err(e) = sema.analyze_program(&program) {
            diagnostics.push(e.to_diagnostic());
            return Err(diagnostics);
        }

        let mut ownership = OwnershipChecker::new();
        if let Err(e) = ownership.check_program(&program) {
            diagnostics.push(e.to_diagnostic());
            return Err(diagnostics);
        }

        Ok(program)
    }

    pub fn generate_bytecode(&mut self, program: &Program) -> Result<Chunk, CompilerError> {
        // Find if there is a main function or top-level statements
        let mut main_func = None;
        let mut top_level_stmts = Vec::new();

        for item in &program.items {
            match item {
                TopLevelItem::Function(f) if f.name == "main" => {
                    main_func = Some(f);
                }
                TopLevelItem::Statement(s) => {
                    top_level_stmts.push(s);
                }
                _ => {}
            }
        }

        if let Some(main) = main_func {
            // Compile main function body
            for stmt in &main.body.statements {
                self.compile_statement(stmt)?;
            }
        } else {
            // Compile top-level statements directly
            for stmt in top_level_stmts {
                self.compile_statement(stmt)?;
            }
        }

        self.chunk.write_op(OpCode::Halt, 0);
        Ok(self.chunk.clone())
    }

    fn compile_statement(&mut self, stmt: &Stmt) -> Result<(), CompilerError> {
        match stmt {
            Stmt::Let { name, init, span, .. } | Stmt::Mut { name, init, span, .. } => {
                self.compile_expression(init)?;
                let slot = self.declare_local(name);
                self.chunk.write_op(OpCode::SetLocal, span.line);
                self.chunk.write_u16(slot, span.line);
            }
            Stmt::Assign { target, value, span } => {
                self.compile_expression(value)?;
                if let Some(slot) = self.resolve_local(target) {
                    self.chunk.write_op(OpCode::SetLocal, span.line);
                    self.chunk.write_u16(slot, span.line);
                } else {
                    let name_val = Value::Text(target.clone());
                    let const_idx = self.chunk.add_constant(name_val);
                    self.chunk.write_op(OpCode::SetGlobal, span.line);
                    self.chunk.write_u16(const_idx, span.line);
                    self.chunk.write_op(OpCode::Pop, span.line);
                }
            }
            Stmt::If { condition, then_branch, else_branch, span } => {
                self.compile_expression(condition)?;

                // Emit JumpIfFalse placeholder
                self.chunk.write_op(OpCode::JumpIfFalse, span.line);
                let jump_false_offset = self.chunk.code.len();
                self.chunk.write_u16(0, span.line);

                // Then branch
                for s in &then_branch.statements {
                    self.compile_statement(s)?;
                }

                if let Some(ref el) = else_branch {
                    self.chunk.write_op(OpCode::Jump, span.line);
                    let jump_else_offset = self.chunk.code.len();
                    self.chunk.write_u16(0, span.line);

                    // Patch JumpIfFalse to jump to else branch start
                    let else_start = self.chunk.code.len() as u16;
                    self.patch_u16(jump_false_offset, else_start);

                    for s in &el.statements {
                        self.compile_statement(s)?;
                    }

                    // Patch Jump to end of else branch
                    let end_offset = self.chunk.code.len() as u16;
                    self.patch_u16(jump_else_offset, end_offset);
                } else {
                    let end_offset = self.chunk.code.len() as u16;
                    self.patch_u16(jump_false_offset, end_offset);
                }
            }
            Stmt::While { condition, body, span } => {
                let loop_start = self.chunk.code.len() as u16;
                self.compile_expression(condition)?;

                self.chunk.write_op(OpCode::JumpIfFalse, span.line);
                let exit_jump = self.chunk.code.len();
                self.chunk.write_u16(0, span.line);

                for s in &body.statements {
                    self.compile_statement(s)?;
                }

                self.chunk.write_op(OpCode::Jump, span.line);
                self.chunk.write_u16(loop_start, span.line);

                let loop_end = self.chunk.code.len() as u16;
                self.patch_u16(exit_jump, loop_end);
            }
            Stmt::Return { value, span } => {
                if let Some(ref v) = value {
                    self.compile_expression(v)?;
                } else {
                    self.chunk.write_op(OpCode::Nil, span.line);
                }
                self.chunk.write_op(OpCode::Return, span.line);
            }
            Stmt::Flow { name, source, stages: _, span } => {
                self.compile_expression(source)?;
                let slot = self.declare_local(name);
                self.chunk.write_op(OpCode::SetLocal, span.line);
                self.chunk.write_u16(slot, span.line);
            }
            Stmt::Expr(expr) => {
                self.compile_expression(expr)?;
                self.chunk.write_op(OpCode::Pop, expr.span.line);
            }
        }
        Ok(())
    }

    fn compile_expression(&mut self, expr: &Expr) -> Result<(), CompilerError> {
        let line = expr.span.line;
        match &expr.kind {
            ExprKind::Integer(n) => {
                let idx = self.chunk.add_constant(Value::Int(*n));
                self.chunk.write_op(OpCode::Constant, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Float(f) => {
                let idx = self.chunk.add_constant(Value::Float(*f));
                self.chunk.write_op(OpCode::Constant, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Text(s) => {
                let idx = self.chunk.add_constant(Value::Text(s.clone()));
                self.chunk.write_op(OpCode::Constant, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Bool(b) => {
                self.chunk.write_op(if *b { OpCode::True } else { OpCode::False }, line);
            }
            ExprKind::Unit => {
                self.chunk.write_op(OpCode::Nil, line);
            }
            ExprKind::Identifier(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(slot, line);
                } else {
                    let idx = self.chunk.add_constant(Value::Text(name.clone()));
                    self.chunk.write_op(OpCode::GetGlobal, line);
                    self.chunk.write_u16(idx, line);
                }
            }
            ExprKind::Binary { op, left, right } => {
                self.compile_expression(left)?;
                self.compile_expression(right)?;
                match op {
                    BinaryOp::Add => self.chunk.write_op(OpCode::Add, line),
                    BinaryOp::Sub => self.chunk.write_op(OpCode::Subtract, line),
                    BinaryOp::Mul => self.chunk.write_op(OpCode::Multiply, line),
                    BinaryOp::Div => self.chunk.write_op(OpCode::Divide, line),
                    BinaryOp::Mod => self.chunk.write_op(OpCode::Modulo, line),
                    BinaryOp::Eq => self.chunk.write_op(OpCode::Equal, line),
                    BinaryOp::Neq => self.chunk.write_op(OpCode::NotEqual, line),
                    BinaryOp::Lt => self.chunk.write_op(OpCode::Less, line),
                    BinaryOp::Lte => self.chunk.write_op(OpCode::LessEqual, line),
                    BinaryOp::Gt => self.chunk.write_op(OpCode::Greater, line),
                    BinaryOp::Gte => self.chunk.write_op(OpCode::GreaterEqual, line),
                    BinaryOp::And => self.chunk.write_op(OpCode::And, line),
                    BinaryOp::Or => self.chunk.write_op(OpCode::Or, line),
                }
            }
            ExprKind::Unary { op, operand } => {
                self.compile_expression(operand)?;
                match op {
                    UnaryOp::Neg => self.chunk.write_op(OpCode::Negate, line),
                    UnaryOp::Not => self.chunk.write_op(OpCode::Not, line),
                }
            }
            ExprKind::Call { callee, args } => {
                if let ExprKind::Identifier(ref name) = callee.kind {
                    if name == "emit" {
                        if !args.is_empty() {
                            self.compile_expression(&args[0])?;
                        } else {
                            self.chunk.write_op(OpCode::Nil, line);
                        }
                        self.chunk.write_op(OpCode::Emit, line);
                        return Ok(());
                    }
                }

                // Ordinary call: push callee, push arguments, OpCall
                self.compile_expression(callee)?;
                for arg in args {
                    self.compile_expression(arg)?;
                }
                self.chunk.write_op(OpCode::Call, line);
                self.chunk.write_byte(args.len() as u8, line);
            }
            ExprKind::Array(elements) => {
                for el in elements {
                    self.compile_expression(el)?;
                }
                self.chunk.write_op(OpCode::Array, line);
                self.chunk.write_u16(elements.len() as u16, line);
            }
            ExprKind::Index { collection, index } => {
                self.compile_expression(collection)?;
                self.compile_expression(index)?;
                self.chunk.write_op(OpCode::Index, line);
            }
            ExprKind::Pipe { expr: source, stages: _ } => {
                self.compile_expression(source)?;
            }
            _ => {
                self.chunk.write_op(OpCode::Nil, line);
            }
        }
        Ok(())
    }

    fn declare_local(&mut self, name: &str) -> u16 {
        let slot = self.next_slot;
        self.next_slot += 1;
        if let Some(scope) = self.locals.last_mut() {
            scope.insert(name.to_string(), slot);
        }
        slot
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        for scope in self.locals.iter().rev() {
            if let Some(slot) = scope.get(name) {
                return Some(*slot);
            }
        }
        None
    }

    fn patch_u16(&mut self, offset: usize, value: u16) {
        self.chunk.code[offset] = (value >> 8) as u8;
        self.chunk.code[offset + 1] = (value & 0xFF) as u8;
    }
}
