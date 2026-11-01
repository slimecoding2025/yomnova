use std::collections::HashMap;
use yomnova_ast::*;
use yomnova_diagnostics::{Diagnostic, Span};
use yomnova_types::Type;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OwnershipError {
    #[error("Use of moved value '{name}' at {used_at} (previously moved at {moved_at})")]
    UseAfterMove {
        name: String,
        moved_at: Span,
        used_at: Span,
    },
    #[error("Cannot move out of borrowed context for '{name}' at {span}")]
    CannotMoveOut { name: String, span: Span },
    #[error("Value '{name}' is moved inside a loop at {moved_at} but declared outside it")]
    MovedInLoop { name: String, moved_at: Span },
}

impl OwnershipError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            OwnershipError::UseAfterMove {
                name,
                moved_at,
                used_at,
            } => {
                Diagnostic::error(format!("Use of moved value '{name}'"), Some(*used_at)).with_hint(
                    format!("Value was previously moved at line {}", moved_at.line),
                )
            }
            OwnershipError::MovedInLoop { name, moved_at } => Diagnostic::error(
                format!("Value '{name}' is moved inside a loop but declared outside it"),
                Some(*moved_at),
            )
            .with_hint(
                "The second iteration would use a moved value; reassign it before the loop repeats",
            ),
            OwnershipError::CannotMoveOut { name, span } => Diagnostic::error(
                format!("Cannot move out of borrowed context for '{name}'"),
                Some(*span),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariableState {
    Owned,
    Moved { at: Span },
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub name: String,
    pub ty: Type,
    pub state: VariableState,
    pub is_copy: bool,
}

pub struct OwnershipChecker {
    scopes: Vec<HashMap<String, Binding>>,
    fn_returns: HashMap<String, Type>,
}

impl OwnershipChecker {
    pub fn new() -> Self {
        let mut fn_returns = HashMap::new();
        fn_returns.insert("emit".to_string(), Type::Unit);
        fn_returns.insert("IPv4".to_string(), Type::IPv4);
        fn_returns.insert("Port".to_string(), Type::Port);
        fn_returns.insert("Endpoint".to_string(), Type::Endpoint);
        Self {
            scopes: vec![HashMap::new()],
            fn_returns,
        }
    }
}

impl Default for OwnershipChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnershipChecker {
    pub fn check_program(&mut self, program: &Program) -> Result<(), OwnershipError> {
        for item in &program.items {
            if let TopLevelItem::Function(f) = item {
                let ret = f
                    .return_type
                    .as_ref()
                    .map(Type::from_annotation)
                    .unwrap_or(Type::Unit);
                self.fn_returns.insert(f.name.clone(), ret);
            }
        }
        for item in &program.items {
            if let TopLevelItem::Const(c) = item {
                let ty = match &c.ty {
                    Some(a) => Type::from_annotation(a),
                    None => self.infer(&c.value),
                };
                let is_copy = Self::is_copy_type(&ty);
                self.declare(c.name.clone(), ty, is_copy);
            }
        }
        for item in &program.items {
            match item {
                TopLevelItem::Function(func) => self.check_function(func)?,
                TopLevelItem::Statement(stmt) => self.check_statement(stmt)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn check_function(&mut self, func: &FunctionDecl) -> Result<(), OwnershipError> {
        self.enter_scope();
        for param in &func.params {
            let ty = Type::from_annotation(&param.ty);
            let is_copy = Self::is_copy_type(&ty);
            self.declare(param.name.clone(), ty, is_copy);
        }
        self.check_block(&func.body)?;
        self.leave_scope();
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<(), OwnershipError> {
        self.enter_scope();
        for stmt in &block.statements {
            self.check_statement(stmt)?;
        }
        self.leave_scope();
        Ok(())
    }

    /// Best-effort static type of an expression, used only to decide copy vs move.
    fn infer(&self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Integer(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Text(_) => Type::Text,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Unit => Type::Unit,
            ExprKind::Identifier(n) => self
                .lookup(n)
                .map(|b| b.ty.clone())
                .unwrap_or(Type::Unknown),
            ExprKind::Binary { op, left, .. } => match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    self.infer(left)
                }
                _ => Type::Bool,
            },
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::Neg => self.infer(operand),
                UnaryOp::Not => Type::Bool,
            },
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Identifier(n) => self.fn_returns.get(n).cloned().unwrap_or(Type::Unknown),
                _ => Type::Unknown,
            },
            ExprKind::Array(els) => Type::Array(Box::new(
                els.first().map(|e| self.infer(e)).unwrap_or(Type::Unknown),
            )),
            ExprKind::Index { collection, .. } => match self.infer(collection) {
                Type::Array(inner) => *inner,
                _ => Type::Unknown,
            },
            ExprKind::Pipe { expr: src, stages } => self.infer_pipeline(src, stages),
            _ => Type::Unknown,
        }
    }

    fn infer_pipeline(&self, src: &Expr, stages: &[PipeStage]) -> Type {
        match stages.last() {
            Some(last) => self
                .fn_returns
                .get(&last.op_name)
                .cloned()
                .unwrap_or(Type::Unknown),
            None => self.infer(src),
        }
    }

    fn check_statement(&mut self, stmt: &Stmt) -> Result<(), OwnershipError> {
        match stmt {
            Stmt::Let { name, ty, init, .. } | Stmt::Mut { name, ty, init, .. } => {
                let ty = match ty {
                    Some(a) => Type::from_annotation(a),
                    None => self.infer(init),
                };
                // `let b = a` moves `a` (unless it is a copy type).
                self.check_expr_move(init)?;
                let is_copy = Self::is_copy_type(&ty);
                self.declare(name.clone(), ty, is_copy);
            }
            Stmt::Assign { target, value, .. } => {
                self.check_expr_move(value)?;
                // Reassignment re-initialises the variable.
                if let Some(binding) = self.lookup_mut(target) {
                    binding.state = VariableState::Owned;
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.check_expr_use(condition)?;
                let before = self.scopes.clone();
                self.check_block(then_branch)?;
                let after_then = std::mem::replace(&mut self.scopes, before);
                if let Some(el) = else_branch {
                    self.check_block(el)?;
                }
                // A value is "maybe moved" after the `if` when either branch moved it.
                for (scope, then_scope) in self.scopes.iter_mut().zip(after_then.iter()) {
                    for (name, binding) in scope.iter_mut() {
                        if let Some(t) = then_scope.get(name) {
                            if let (VariableState::Owned, VariableState::Moved { at }) =
                                (&binding.state, &t.state)
                            {
                                binding.state = VariableState::Moved { at: *at };
                            }
                        }
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.check_expr_use(condition)?;
                let before = self.scopes.clone();
                self.check_block(body)?;
                // Anything declared outside the loop that was owned before the body
                // and is moved after it would be used again on the next iteration.
                for (b_scope, a_scope) in before.iter().zip(self.scopes.iter()) {
                    for (name, b) in b_scope {
                        if b.state == VariableState::Owned {
                            if let Some(Binding {
                                state: VariableState::Moved { at },
                                ..
                            }) = a_scope.get(name)
                            {
                                return Err(OwnershipError::MovedInLoop {
                                    name: name.clone(),
                                    moved_at: *at,
                                });
                            }
                        }
                    }
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr_move(v)?;
                }
            }
            Stmt::Flow {
                name,
                source,
                stages,
                ..
            } => {
                let ty = self.infer_pipeline(source, stages);
                self.check_pipeline(source, stages)?;
                let is_copy = Self::is_copy_type(&ty);
                self.declare(name.clone(), ty, is_copy);
            }
            Stmt::Expr(expr) => {
                self.check_expr_use(expr)?;
            }
        }
        Ok(())
    }

    /// A pipeline consumes its source (and identifier stage arguments) unless it only emits it.
    fn check_pipeline(
        &mut self,
        source: &Expr,
        stages: &[PipeStage],
    ) -> Result<(), OwnershipError> {
        let borrows_source = matches!(stages.first(), Some(s) if s.op_name == "emit");
        if borrows_source {
            self.check_expr_use(source)?;
        } else {
            self.check_expr_move(source)?;
        }
        for st in stages {
            if let Some(arg) = &st.arg {
                self.check_expr_move(arg)?;
            }
        }
        Ok(())
    }

    /// Mark `expr` as consumed when it is a bare identifier of a non-copy type.
    fn check_expr_move(&mut self, expr: &Expr) -> Result<(), OwnershipError> {
        if let ExprKind::Identifier(name) = &expr.kind {
            if let Some(binding) = self.lookup(name) {
                if let VariableState::Moved { at } = binding.state {
                    return Err(OwnershipError::UseAfterMove {
                        name: name.clone(),
                        moved_at: at,
                        used_at: expr.span,
                    });
                }
                if !binding.is_copy && binding.ty != Type::Unknown {
                    if let Some(b) = self.lookup_mut(name) {
                        b.state = VariableState::Moved { at: expr.span };
                    }
                }
            }
            Ok(())
        } else {
            self.check_expr_use(expr)
        }
    }

    fn check_expr_use(&mut self, expr: &Expr) -> Result<(), OwnershipError> {
        match &expr.kind {
            ExprKind::Identifier(name) => {
                if let Some(binding) = self.lookup(name) {
                    if let VariableState::Moved { at } = binding.state {
                        return Err(OwnershipError::UseAfterMove {
                            name: name.clone(),
                            moved_at: at,
                            used_at: expr.span,
                        });
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.check_expr_use(left)?;
                self.check_expr_use(right)?;
            }
            ExprKind::Unary { operand, .. } => self.check_expr_use(operand)?,
            ExprKind::Call { callee, args } => {
                self.check_expr_use(callee)?;
                // `emit` only borrows its argument; every other call consumes identifier arguments.
                let borrows = matches!(&callee.kind, ExprKind::Identifier(n) if n == "emit");
                for arg in args {
                    if borrows {
                        self.check_expr_use(arg)?;
                    } else {
                        self.check_expr_move(arg)?;
                    }
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.check_expr_use(receiver)?;
                for arg in args {
                    self.check_expr_use(arg)?;
                }
            }
            ExprKind::Array(elements) => {
                for el in elements {
                    self.check_expr_move(el)?;
                }
            }
            ExprKind::Index { collection, index } => {
                self.check_expr_use(collection)?;
                self.check_expr_use(index)?;
            }
            ExprKind::Pipe { expr: src, stages } => self.check_pipeline(src, stages)?,
            _ => {}
        }
        Ok(())
    }

    /// Only these types are implicitly copied; everything else moves.
    fn is_copy_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Int | Type::Float | Type::Bool | Type::Unit | Type::Unknown
        )
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn leave_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: String, ty: Type, is_copy: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.clone(),
                Binding {
                    name,
                    ty,
                    state: VariableState::Owned,
                    is_copy,
                },
            );
        }
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Binding> {
        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
    }
}
