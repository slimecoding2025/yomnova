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
}

impl OwnershipError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            OwnershipError::UseAfterMove {
                name,
                moved_at,
                used_at,
            } => Diagnostic::error(
                format!("Use of moved value '{name}'"),
                Some(*used_at),
            )
            .with_hint(format!("Value was previously moved at line {}", moved_at.line)),
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
}

impl OwnershipChecker {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
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
            let is_copy = self.is_copy_type(&ty);
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

    fn check_statement(&mut self, stmt: &Stmt) -> Result<(), OwnershipError> {
        match stmt {
            Stmt::Let { name, ty, init, span: _ } | Stmt::Mut { name, ty, init, span: _ } => {
                let inferred_ty = ty.as_ref().map(Type::from_annotation).unwrap_or(Type::Unknown);
                self.check_expr_use(init)?;
                let is_copy = self.is_copy_type(&inferred_ty);
                self.declare(name.clone(), inferred_ty, is_copy);
            }
            Stmt::Assign { target, value, span: _ } => {
                self.check_expr_use(value)?;
                // Reassignment re-initializes the variable to Owned
                if let Some(binding) = self.lookup_mut(target) {
                    binding.state = VariableState::Owned;
                }
            }
            Stmt::If { condition, then_branch, else_branch, span: _ } => {
                self.check_expr_use(condition)?;
                self.check_block(then_branch)?;
                if let Some(ref el) = else_branch {
                    self.check_block(el)?;
                }
            }
            Stmt::While { condition, body, span: _ } => {
                self.check_expr_use(condition)?;
                self.check_block(body)?;
            }
            Stmt::Return { value, span: _ } => {
                if let Some(ref v) = value {
                    self.check_expr_move(v)?;
                }
            }
            Stmt::Flow { name, source, stages: _, span: _ } => {
                self.check_expr_use(source)?;
                self.declare(name.clone(), Type::Unknown, false);
            }
            Stmt::Expr(expr) => {
                self.check_expr_use(expr)?;
            }
        }
        Ok(())
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
            ExprKind::Unary { operand, .. } => {
                self.check_expr_use(operand)?;
            }
            ExprKind::Call { callee, args } => {
                self.check_expr_use(callee)?;
                for arg in args {
                    // Check if function call consumes arguments (moves non-copy types)
                    if let ExprKind::Identifier(ref var_name) = arg.kind {
                        if let Some(binding) = self.lookup(var_name) {
                            if let VariableState::Moved { at } = binding.state {
                                return Err(OwnershipError::UseAfterMove {
                                    name: var_name.clone(),
                                    moved_at: at,
                                    used_at: arg.span,
                                });
                            }
                            if !binding.is_copy {
                                // Mark as moved!
                                if let Some(mut_binding) = self.lookup_mut(var_name) {
                                    mut_binding.state = VariableState::Moved { at: arg.span };
                                }
                            }
                        }
                    } else {
                        self.check_expr_use(arg)?;
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
                    self.check_expr_use(el)?;
                }
            }
            ExprKind::Index { collection, index } => {
                self.check_expr_use(collection)?;
                self.check_expr_use(index)?;
            }
            ExprKind::Pipe { expr: src, .. } => {
                self.check_expr_use(src)?;
            }
            _ => {}
        }
        Ok(())
    }

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
                if !binding.is_copy {
                    if let Some(mut_b) = self.lookup_mut(name) {
                        mut_b.state = VariableState::Moved { at: expr.span };
                    }
                }
            }
        } else {
            self.check_expr_use(expr)?;
        }
        Ok(())
    }

    fn is_copy_type(&self, ty: &Type) -> bool {
        matches!(ty, Type::Int | Type::Float | Type::Bool | Type::Unit)
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
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.get(name) {
                return Some(b);
            }
        }
        None
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Binding> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(b) = scope.get_mut(name) {
                return Some(b);
            }
        }
        None
    }
}
