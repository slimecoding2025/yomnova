//! YOMNOVA Multi-Stage Compiler Driver and Bytecode Generator.

pub mod format;
pub mod lint;

use std::collections::HashMap;
use yomnova_ast::*;
use yomnova_bytecode::{Chunk, OpCode, Value};
use yomnova_diagnostics::Diagnostic;
use yomnova_lexer::{Lexer, Token};
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

/// Result of compiling a program without executing it.
pub struct CompiledUnit {
    pub ast: Program,
    /// Entry chunk: top-level `const` initialisers followed by `main`'s body.
    pub main: Chunk,
    /// User functions other than `main`, in declaration order. The index in
    /// this vector is the chunk index the VM uses.
    pub functions: Vec<(String, Chunk, usize)>,
}

impl CompiledUnit {
    /// Human-readable disassembly of the entry chunk and every function.
    pub fn disassemble_all(&self) -> String {
        let mut out = self.main.disassemble("<main>");
        for (name, chunk, arity) in &self.functions {
            out.push('\n');
            out.push_str(&chunk.disassemble(&format!("fn {name}/{arity}")));
        }
        out
    }
}

pub struct CompilerOutput {
    pub ast: Program,
    pub chunk: Chunk,
    pub functions: Vec<(String, Chunk, usize)>,
    pub execution_result: Option<Value>,
    pub output_lines: Vec<String>,
}

impl CompilerOutput {
    pub fn disassemble_all(&self) -> String {
        let mut out = self.chunk.disassemble("<main>");
        for (name, chunk, arity) in &self.functions {
            out.push('\n');
            out.push_str(&chunk.disassemble(&format!("fn {name}/{arity}")));
        }
        out
    }
}

pub struct Compiler {
    locals: Vec<HashMap<String, u16>>,
    next_slot: u16,
    chunk: Chunk,
    /// Struct name -> field names in declaration order (struct literals are laid out in this order).
    structs: HashMap<String, Vec<String>>,
    /// Enum names; variants are validated by semantic analysis before code generation.
    enums: std::collections::HashSet<String>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            locals: vec![HashMap::new()],
            next_slot: 0,
            chunk: Chunk::new(),
            structs: HashMap::new(),
            enums: std::collections::HashSet::new(),
        }
    }

    fn enum_table(program: &Program) -> std::collections::HashSet<String> {
        program
            .items
            .iter()
            .filter_map(|i| match i {
                TopLevelItem::Enum(e) => Some(e.name.clone()),
                _ => None,
            })
            .collect()
    }

    /// `Some(enum name)` for a bare identifier naming an enum that no local variable shadows.
    fn enum_ref(&self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Identifier(n)
                if self.enums.contains(n) && self.resolve_local(n).is_none() =>
            {
                Some(n.clone())
            }
            _ => None,
        }
    }

    fn struct_table(program: &Program) -> HashMap<String, Vec<String>> {
        program
            .items
            .iter()
            .filter_map(|i| match i {
                TopLevelItem::Struct(s) => Some((
                    s.name.clone(),
                    s.fields.iter().map(|f| f.name.clone()).collect(),
                )),
                _ => None,
            })
            .collect()
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

fn lex_and_parse(source: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(|e| vec![e.to_diagnostic()])?;
    let (program, errors) = Parser::new(tokens).parse_program_recovering();
    if errors.is_empty() {
        Ok(program)
    } else {
        Err(errors.iter().map(|e| e.to_diagnostic()).collect())
    }
}

impl Compiler {
    /// Tokenise only (used by `yomnova tokens`).
    pub fn tokens(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
        Lexer::new(source)
            .tokenize()
            .map_err(|e| vec![e.to_diagnostic()])
    }

    /// One token per line: `line:col  Kind`.
    pub fn render_tokens(tokens: &[Token]) -> String {
        let mut out = String::new();
        for t in tokens {
            out.push_str(&format!(
                "{:>4}:{:<3} {}\n",
                t.span.line, t.span.col, t.kind
            ));
        }
        out
    }

    /// Parse only and pretty-print the AST (used by `yomnova ast`).
    pub fn ast_dump(source: &str) -> Result<String, Vec<Diagnostic>> {
        let program = lex_and_parse(source)?;
        Ok(format!("{program:#?}"))
    }

    /// Like [`Compiler::check`], and also returns the warnings (see [`lint`]) for a valid program.
    pub fn check_with_warnings(
        source: &str,
    ) -> Result<(Program, Vec<Diagnostic>), Vec<Diagnostic>> {
        let program = Self::check(source)?;
        let warnings = lint::lint(&program);
        Ok((program, warnings))
    }

    /// Verification only: Lex -> Parse -> Semantic Analysis -> Ownership Check
    pub fn check(source: &str) -> Result<Program, Vec<Diagnostic>> {
        let program = lex_and_parse(source)?;

        // Each stage reports all of its errors; a later stage only runs when the earlier
        // one is clean, because it would otherwise analyse a program that is not valid.
        let mut sema = SemanticAnalyzer::new();
        let errors = sema.analyze_program_all(&program);
        if !errors.is_empty() {
            return Err(errors.iter().map(|e| e.to_diagnostic()).collect());
        }

        let mut ownership = OwnershipChecker::new();
        let errors = ownership.check_program_all(&program);
        if !errors.is_empty() {
            return Err(errors.iter().map(|e| e.to_diagnostic()).collect());
        }

        Ok(program)
    }

    /// Compile to bytecode without running anything.
    pub fn compile_unit(source: &str) -> Result<CompiledUnit, Vec<Diagnostic>> {
        let program = Self::check(source)?;

        let fail = |e: CompilerError| vec![Diagnostic::error(e.to_string(), None)];

        let structs = Self::struct_table(&program);
        let enums = Self::enum_table(&program);
        let mut functions = Vec::new();
        for item in &program.items {
            if let TopLevelItem::Function(f) = item {
                if f.name != "main" {
                    let mut fc = Compiler::new();
                    fc.structs = structs.clone();
                    fc.enums = enums.clone();
                    for p in &f.params {
                        fc.declare_local(&p.name).map_err(fail)?;
                    }
                    fc.compile_block_body(&f.body).map_err(fail)?;
                    let end = f.body.span.line;
                    fc.chunk.write_op(OpCode::Nil, end);
                    fc.chunk.write_op(OpCode::Return, end);
                    functions.push((f.name.clone(), fc.chunk, f.params.len()));
                }
            }
        }

        let mut compiler = Compiler::new();
        compiler.structs = structs;
        let main = compiler.generate_bytecode(&program).map_err(fail)?;
        Ok(CompiledUnit {
            ast: program,
            main,
            functions,
        })
    }

    /// Complete pipeline: Lex -> Parse -> Sema -> Ownership -> Bytecode -> Run in VM
    pub fn compile_and_run(source: &str) -> Result<CompilerOutput, Vec<Diagnostic>> {
        let unit = Self::compile_unit(source)?;

        let mut vm = Vm::new();
        for (name, chunk, arity) in &unit.functions {
            let chunk_index = vm.chunks.len();
            vm.chunks.push(chunk.clone());
            vm.globals.insert(
                name.clone(),
                Value::UserFunction {
                    name: name.clone(),
                    chunk_index,
                    arity: *arity,
                },
            );
        }

        let result = match vm.run_chunk(unit.main.clone()) {
            Ok(val) => val,
            Err(e) => {
                return Err(vec![Diagnostic::error(format!("Runtime error: {e}"), None)]);
            }
        };

        Ok(CompilerOutput {
            ast: unit.ast,
            chunk: unit.main,
            functions: unit.functions,
            execution_result: Some(result),
            output_lines: vm.output,
        })
    }

    pub fn generate_bytecode(&mut self, program: &Program) -> Result<Chunk, CompilerError> {
        if self.structs.is_empty() {
            self.structs = Self::struct_table(program);
        }
        if self.enums.is_empty() {
            self.enums = Self::enum_table(program);
        }
        let mut main_func = None;
        let mut top_level_stmts = Vec::new();

        for item in &program.items {
            match item {
                TopLevelItem::Function(f) if f.name == "main" => main_func = Some(f),
                TopLevelItem::Statement(s) => top_level_stmts.push(s),
                _ => {}
            }
        }

        if main_func.is_none() && top_level_stmts.is_empty() {
            return Err(CompilerError::Codegen(
                "no entry point: define `fn main() -> Int { ... }`".to_string(),
            ));
        }

        // Top-level constants are initialised, in source order, before the entry body.
        for item in &program.items {
            if let TopLevelItem::Const(c) = item {
                self.compile_expression(&c.value)?;
                let idx = self.constant(Value::Text(c.name.clone()))?;
                let line = c.span.line;
                self.chunk.write_op(OpCode::SetGlobal, line);
                self.chunk.write_u16(idx, line);
                self.chunk.write_op(OpCode::Pop, line);
            }
        }

        if let Some(main) = main_func {
            self.compile_block_body(&main.body)?;
        } else {
            for stmt in top_level_stmts {
                self.compile_statement(stmt)?;
            }
        }

        self.chunk.write_op(OpCode::Halt, 0);
        Ok(self.chunk.clone())
    }

    fn compile_block_body(&mut self, block: &Block) -> Result<(), CompilerError> {
        for stmt in &block.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
    }

    /// Compile a nested block in its own lexical scope.
    fn compile_scoped_block(&mut self, block: &Block) -> Result<(), CompilerError> {
        self.locals.push(HashMap::new());
        let result = self.compile_block_body(block);
        self.locals.pop();
        result
    }

    fn constant(&mut self, value: Value) -> Result<u16, CompilerError> {
        self.chunk.try_add_constant(value).ok_or_else(|| {
            CompilerError::Codegen("too many constants in one function (limit 65536)".to_string())
        })
    }

    /// Emit a jump with a placeholder operand; returns the operand offset.
    fn emit_jump(&mut self, op: OpCode, line: usize) -> usize {
        self.chunk.write_op(op, line);
        let at = self.chunk.code.len();
        self.chunk.write_u16(0, line);
        at
    }

    fn patch_jump(&mut self, operand_at: usize) -> Result<(), CompilerError> {
        let target = u16::try_from(self.chunk.code.len()).map_err(|_| {
            CompilerError::Codegen(
                "function too large (jump target exceeds 65535 bytes)".to_string(),
            )
        })?;
        self.chunk.code[operand_at] = (target >> 8) as u8;
        self.chunk.code[operand_at + 1] = (target & 0xFF) as u8;
        Ok(())
    }

    fn here(&self) -> Result<u16, CompilerError> {
        u16::try_from(self.chunk.code.len()).map_err(|_| {
            CompilerError::Codegen("function too large (offset exceeds 65535 bytes)".to_string())
        })
    }

    fn compile_statement(&mut self, stmt: &Stmt) -> Result<(), CompilerError> {
        match stmt {
            Stmt::Let {
                name, init, span, ..
            }
            | Stmt::Mut {
                name, init, span, ..
            } => {
                self.compile_expression(init)?;
                let slot = self.declare_local(name)?;
                self.chunk.write_op(OpCode::SetLocal, span.line);
                self.chunk.write_u16(slot, span.line);
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                self.compile_expression(value)?;
                if let Some(slot) = self.resolve_local(target) {
                    self.chunk.write_op(OpCode::SetLocal, span.line);
                    self.chunk.write_u16(slot, span.line);
                } else {
                    let idx = self.constant(Value::Text(target.clone()))?;
                    self.chunk.write_op(OpCode::SetGlobal, span.line);
                    self.chunk.write_u16(idx, span.line);
                    self.chunk.write_op(OpCode::Pop, span.line);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                self.compile_expression(condition)?;
                let to_else = self.emit_jump(OpCode::JumpIfFalse, span.line);
                self.compile_scoped_block(then_branch)?;
                if let Some(el) = else_branch {
                    let to_end = self.emit_jump(OpCode::Jump, span.line);
                    self.patch_jump(to_else)?;
                    self.compile_scoped_block(el)?;
                    self.patch_jump(to_end)?;
                } else {
                    self.patch_jump(to_else)?;
                }
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let loop_start = self.here()?;
                self.compile_expression(condition)?;
                let exit = self.emit_jump(OpCode::JumpIfFalse, span.line);
                self.compile_scoped_block(body)?;
                self.chunk.write_op(OpCode::Jump, span.line);
                self.chunk.write_u16(loop_start, span.line);
                self.patch_jump(exit)?;
            }
            Stmt::Match {
                scrutinee,
                arms,
                span,
            } => {
                let line = span.line;
                self.locals.push(HashMap::new());
                let result = (|| -> Result<(), CompilerError> {
                    self.compile_expression(scrutinee)?;
                    let hidden = self.declare_local("$match")?;
                    self.chunk.write_op(OpCode::SetLocal, line);
                    self.chunk.write_u16(hidden, line);
                    let mut to_end = Vec::new();
                    for arm in arms {
                        let aline = arm.span.line;
                        match &arm.pattern {
                            Pattern::Wildcard => {
                                self.compile_scoped_block(&arm.body)?;
                                to_end.push(self.emit_jump(OpCode::Jump, aline));
                            }
                            Pattern::Variant {
                                enum_name,
                                variant,
                                binding,
                                ..
                            } => {
                                let idx =
                                    self.constant(Value::Text(format!("{enum_name}.{variant}")))?;
                                self.chunk.write_op(OpCode::GetLocal, aline);
                                self.chunk.write_u16(hidden, aline);
                                self.chunk.write_op(OpCode::EnumIs, aline);
                                self.chunk.write_u16(idx, aline);
                                let next = self.emit_jump(OpCode::JumpIfFalse, aline);
                                self.locals.push(HashMap::new());
                                let arm_result = (|| -> Result<(), CompilerError> {
                                    if let Some(name) = binding {
                                        self.chunk.write_op(OpCode::GetLocal, aline);
                                        self.chunk.write_u16(hidden, aline);
                                        self.chunk.write_op(OpCode::EnumPayload, aline);
                                        let slot = self.declare_local(name)?;
                                        self.chunk.write_op(OpCode::SetLocal, aline);
                                        self.chunk.write_u16(slot, aline);
                                    }
                                    self.compile_block_body(&arm.body)
                                })();
                                self.locals.pop();
                                arm_result?;
                                to_end.push(self.emit_jump(OpCode::Jump, aline));
                                self.patch_jump(next)?;
                            }
                        }
                    }
                    for j in to_end {
                        self.patch_jump(j)?;
                    }
                    Ok(())
                })();
                self.locals.pop();
                result?;
            }
            Stmt::For {
                var,
                iterable,
                body,
                span,
            } => {
                // Two hidden locals (the array and the index) live in a scope of their own.
                let line = span.line;
                self.locals.push(HashMap::new());
                let result = (|| -> Result<(), CompilerError> {
                    self.compile_expression(iterable)?;
                    let arr = self.declare_local("$for_arr")?;
                    self.chunk.write_op(OpCode::SetLocal, line);
                    self.chunk.write_u16(arr, line);
                    let zero = self.constant(Value::Int(0))?;
                    self.chunk.write_op(OpCode::Constant, line);
                    self.chunk.write_u16(zero, line);
                    let idx = self.declare_local("$for_idx")?;
                    self.chunk.write_op(OpCode::SetLocal, line);
                    self.chunk.write_u16(idx, line);

                    let loop_start = self.here()?;
                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(idx, line);
                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(arr, line);
                    self.chunk.write_op(OpCode::Len, line);
                    self.chunk.write_op(OpCode::Less, line);
                    let exit = self.emit_jump(OpCode::JumpIfFalse, line);

                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(arr, line);
                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(idx, line);
                    self.chunk.write_op(OpCode::Index, line);
                    let slot = self.declare_local(var)?;
                    self.chunk.write_op(OpCode::SetLocal, line);
                    self.chunk.write_u16(slot, line);

                    self.compile_scoped_block(body)?;

                    let one = self.constant(Value::Int(1))?;
                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(idx, line);
                    self.chunk.write_op(OpCode::Constant, line);
                    self.chunk.write_u16(one, line);
                    self.chunk.write_op(OpCode::Add, line);
                    self.chunk.write_op(OpCode::SetLocal, line);
                    self.chunk.write_u16(idx, line);
                    self.chunk.write_op(OpCode::Jump, line);
                    self.chunk.write_u16(loop_start, line);
                    self.patch_jump(exit)
                })();
                self.locals.pop();
                result?;
            }
            Stmt::Return { value, span } => {
                if let Some(v) = value {
                    self.compile_expression(v)?;
                } else {
                    self.chunk.write_op(OpCode::Nil, span.line);
                }
                self.chunk.write_op(OpCode::Return, span.line);
            }
            Stmt::Flow {
                name,
                source,
                stages,
                span,
            } => {
                self.compile_pipeline(source, stages, span.line)?;
                let slot = self.declare_local(name)?;
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

    /// `x |> f |> g(a)` compiles as `g(f(x), a)`; `|> emit` emits the value.
    fn compile_pipeline(
        &mut self,
        source: &Expr,
        stages: &[PipeStage],
        line: usize,
    ) -> Result<(), CompilerError> {
        let Some((last, rest)) = stages.split_last() else {
            return self.compile_expression(source);
        };
        if last.op_name == "emit" {
            self.compile_pipeline(source, rest, line)?;
            self.chunk.write_op(OpCode::Emit, line);
            return Ok(());
        }
        let name_idx = self.constant(Value::Text(last.op_name.clone()))?;
        match self.resolve_local(&last.op_name) {
            Some(slot) => {
                self.chunk.write_op(OpCode::GetLocal, line);
                self.chunk.write_u16(slot, line);
            }
            None => {
                self.chunk.write_op(OpCode::GetGlobal, line);
                self.chunk.write_u16(name_idx, line);
            }
        }
        self.compile_pipeline(source, rest, line)?;
        let mut argc = 1u8;
        if let Some(arg) = &last.arg {
            self.compile_expression(arg)?;
            argc = 2;
        }
        self.chunk.write_op(OpCode::Call, line);
        self.chunk.write_byte(argc, line);
        Ok(())
    }

    fn compile_expression(&mut self, expr: &Expr) -> Result<(), CompilerError> {
        let line = expr.span.line;
        match &expr.kind {
            ExprKind::Integer(n) => {
                let idx = self.constant(Value::Int(*n))?;
                self.chunk.write_op(OpCode::Constant, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Float(f) => {
                let idx = self.constant(Value::Float(*f))?;
                self.chunk.write_op(OpCode::Constant, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Text(s) => {
                let idx = self.constant(Value::Text(s.clone()))?;
                self.chunk.write_op(OpCode::Constant, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Bool(b) => {
                self.chunk
                    .write_op(if *b { OpCode::True } else { OpCode::False }, line);
            }
            ExprKind::Unit => {
                self.chunk.write_op(OpCode::Nil, line);
            }
            ExprKind::Identifier(name) => {
                if let Some(slot) = self.resolve_local(name) {
                    self.chunk.write_op(OpCode::GetLocal, line);
                    self.chunk.write_u16(slot, line);
                } else {
                    let idx = self.constant(Value::Text(name.clone()))?;
                    self.chunk.write_op(OpCode::GetGlobal, line);
                    self.chunk.write_u16(idx, line);
                }
            }
            ExprKind::Binary {
                op: BinaryOp::And,
                left,
                right,
            } => {
                // a && b: if a is false the result is false and b is not evaluated.
                self.compile_expression(left)?;
                let to_false = self.emit_jump(OpCode::JumpIfFalse, line);
                self.compile_expression(right)?;
                let to_end = self.emit_jump(OpCode::Jump, line);
                self.patch_jump(to_false)?;
                self.chunk.write_op(OpCode::False, line);
                self.patch_jump(to_end)?;
            }
            ExprKind::Binary {
                op: BinaryOp::Or,
                left,
                right,
            } => {
                // a || b: if a is true the result is true and b is not evaluated.
                self.compile_expression(left)?;
                let to_rhs = self.emit_jump(OpCode::JumpIfFalse, line);
                self.chunk.write_op(OpCode::True, line);
                let to_end = self.emit_jump(OpCode::Jump, line);
                self.patch_jump(to_rhs)?;
                self.compile_expression(right)?;
                self.patch_jump(to_end)?;
            }
            ExprKind::Binary { op, left, right } => {
                self.compile_expression(left)?;
                self.compile_expression(right)?;
                let code = match op {
                    BinaryOp::Add => OpCode::Add,
                    BinaryOp::Sub => OpCode::Subtract,
                    BinaryOp::Mul => OpCode::Multiply,
                    BinaryOp::Div => OpCode::Divide,
                    BinaryOp::Mod => OpCode::Modulo,
                    BinaryOp::Eq => OpCode::Equal,
                    BinaryOp::Neq => OpCode::NotEqual,
                    BinaryOp::Lt => OpCode::Less,
                    BinaryOp::Lte => OpCode::LessEqual,
                    BinaryOp::Gt => OpCode::Greater,
                    BinaryOp::Gte => OpCode::GreaterEqual,
                    BinaryOp::And | BinaryOp::Or => unreachable!("handled above"),
                };
                self.chunk.write_op(code, line);
            }
            ExprKind::Unary { op, operand } => {
                self.compile_expression(operand)?;
                match op {
                    UnaryOp::Neg => self.chunk.write_op(OpCode::Negate, line),
                    UnaryOp::Not => self.chunk.write_op(OpCode::Not, line),
                }
            }
            ExprKind::Call { callee, args } => {
                if let ExprKind::Identifier(name) = &callee.kind {
                    if name == "emit" && self.resolve_local(name).is_none() {
                        if args.len() != 1 {
                            return Err(CompilerError::Codegen(format!(
                                "emit takes exactly 1 argument, got {}",
                                args.len()
                            )));
                        }
                        self.compile_expression(&args[0])?;
                        self.chunk.write_op(OpCode::Emit, line);
                        return Ok(());
                    }
                }
                let argc = u8::try_from(args.len()).map_err(|_| {
                    CompilerError::Codegen("too many call arguments (limit 255)".to_string())
                })?;
                self.compile_expression(callee)?;
                for arg in args {
                    self.compile_expression(arg)?;
                }
                self.chunk.write_op(OpCode::Call, line);
                self.chunk.write_byte(argc, line);
            }
            ExprKind::Array(elements) => {
                let len = u16::try_from(elements.len()).map_err(|_| {
                    CompilerError::Codegen("array literal too long (limit 65535)".to_string())
                })?;
                for el in elements {
                    self.compile_expression(el)?;
                }
                self.chunk.write_op(OpCode::Array, line);
                self.chunk.write_u16(len, line);
            }
            ExprKind::StructLiteral { name, fields } => {
                let order = self.structs.get(name).cloned().ok_or_else(|| {
                    CompilerError::Codegen(format!("unknown struct '{name}' at line {line}"))
                })?;
                // Initialisers are evaluated in declaration order.
                for fname in &order {
                    let Some((_, value)) = fields.iter().find(|(n, _)| n == fname) else {
                        return Err(CompilerError::Codegen(format!(
                            "missing field '{fname}' in '{name}' at line {line}"
                        )));
                    };
                    self.compile_expression(value)?;
                }
                let name_idx = self.constant(Value::Text(name.clone()))?;
                let names = Value::Array(order.into_iter().map(Value::Text).collect());
                let names_idx = self.constant(names)?;
                self.chunk.write_op(OpCode::Struct, line);
                self.chunk.write_u16(name_idx, line);
                self.chunk.write_u16(names_idx, line);
            }
            ExprKind::FieldAccess { object, field } if self.enum_ref(object).is_some() => {
                let ename = self.enum_ref(object).unwrap_or_default();
                let idx = self.constant(Value::Text(format!("{ename}.{field}")))?;
                self.chunk.write_op(OpCode::EnumNew, line);
                self.chunk.write_u16(idx, line);
                self.chunk.write_byte(0, line);
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } if self.enum_ref(receiver).is_some() && args.len() == 1 => {
                let ename = self.enum_ref(receiver).unwrap_or_default();
                self.compile_expression(&args[0])?;
                let idx = self.constant(Value::Text(format!("{ename}.{method}")))?;
                self.chunk.write_op(OpCode::EnumNew, line);
                self.chunk.write_u16(idx, line);
                self.chunk.write_byte(1, line);
            }
            ExprKind::FieldAccess { object, field } => {
                self.compile_expression(object)?;
                let idx = self.constant(Value::Text(field.clone()))?;
                self.chunk.write_op(OpCode::Field, line);
                self.chunk.write_u16(idx, line);
            }
            ExprKind::Index { collection, index } => {
                self.compile_expression(collection)?;
                self.compile_expression(index)?;
                self.chunk.write_op(OpCode::Index, line);
            }
            ExprKind::Pipe {
                expr: source,
                stages,
            } => {
                self.compile_pipeline(source, stages, line)?;
            }
            other => {
                return Err(CompilerError::Codegen(format!(
                    "unsupported expression at line {line}: {}",
                    expr_kind_name(other)
                )));
            }
        }
        Ok(())
    }

    fn declare_local(&mut self, name: &str) -> Result<u16, CompilerError> {
        let slot = self.next_slot;
        self.next_slot = self.next_slot.checked_add(1).ok_or_else(|| {
            CompilerError::Codegen("too many local variables (limit 65535)".to_string())
        })?;
        if let Some(scope) = self.locals.last_mut() {
            scope.insert(name.to_string(), slot);
        }
        Ok(slot)
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
}

fn expr_kind_name(kind: &ExprKind) -> &'static str {
    match kind {
        ExprKind::MethodCall { .. } => "method call",
        ExprKind::Lambda { .. } => "lambda",
        _ => "expression",
    }
}
