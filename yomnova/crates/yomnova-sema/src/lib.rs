use std::collections::HashMap;
use yomnova_ast::*;
use yomnova_diagnostics::{Diagnostic, Span};
use yomnova_types::Type;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SemaError {
    #[error("Undefined identifier '{name}' at {span}")]
    UndefinedIdentifier { name: String, span: Span },
    #[error("Identifier '{name}' already declared in current scope at {span}")]
    DuplicateDeclaration { name: String, span: Span },
    #[error("Cannot assign to immutable variable '{name}' at {span}")]
    CannotAssignImmutable { name: String, span: Span },
    #[error("Type mismatch at {span}: expected {expected}, found {found}")]
    TypeMismatch {
        expected: String,
        found: String,
        span: Span,
    },
    #[error("Function '{name}' requires {expected} arguments, found {found} at {span}")]
    ArgumentCountMismatch {
        name: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    #[error("Invalid return type at {span}: expected {expected}, found {found}")]
    InvalidReturnType {
        expected: String,
        found: String,
        span: Span,
    },
    #[error("Cannot apply operator {op} to type {ty} at {span}")]
    InvalidUnaryOp { op: String, ty: String, span: Span },
    #[error("Cannot apply binary operator {op} between {left} and {right} at {span}")]
    InvalidBinaryOp {
        op: String,
        left: String,
        right: String,
        span: Span,
    },
    #[error("{feature} is not supported yet (at {span})")]
    Unsupported { feature: String, span: Span },
    #[error("{message} (at {span})")]
    Invalid { message: String, span: Span },
    #[error("Function '{name}' may finish without returning a value (at {span})")]
    MissingReturn { name: String, span: Span },
}

impl SemaError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        let d = Diagnostic::error(self.to_string(), Some(self.span()));
        match self {
            SemaError::Unsupported { .. } => {
                d.with_hint("This syntax parses but has no implementation; see docs/PROGRESS.md")
            }
            SemaError::MissingReturn { .. } => d.with_hint("Add a `return` on every path"),
            _ => d,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            SemaError::UndefinedIdentifier { span, .. } => *span,
            SemaError::DuplicateDeclaration { span, .. } => *span,
            SemaError::CannotAssignImmutable { span, .. } => *span,
            SemaError::TypeMismatch { span, .. } => *span,
            SemaError::ArgumentCountMismatch { span, .. } => *span,
            SemaError::InvalidReturnType { span, .. } => *span,
            SemaError::InvalidUnaryOp { span, .. } => *span,
            SemaError::InvalidBinaryOp { span, .. } => *span,
            SemaError::Unsupported { span, .. } => *span,
            SemaError::Invalid { span, .. } => *span,
            SemaError::MissingReturn { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub ty: Type,
    pub is_mut: bool,
    pub is_constant: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
    pub effects: Vec<String>,
    pub span: Span,
}

pub struct Scope {
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SemanticAnalyzer {
    scopes: Vec<Scope>,
    functions: HashMap<String, FunctionSignature>,
    current_return_type: Option<Type>,
    pub expression_types: HashMap<Span, Type>,
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            scopes: vec![Scope::new()],
            functions: HashMap::new(),
            current_return_type: None,
            expression_types: HashMap::new(),
        };

        // Register built-in functions
        analyzer.register_builtin("emit", vec![("val".to_string(), Type::Unknown)], Type::Unit);
        analyzer.register_builtin("IPv4", vec![("addr".to_string(), Type::Text)], Type::IPv4);
        analyzer.register_builtin("Port", vec![("port".to_string(), Type::Int)], Type::Port);
        analyzer.register_builtin(
            "Endpoint",
            vec![
                ("ip".to_string(), Type::IPv4),
                ("port".to_string(), Type::Unknown),
            ],
            Type::Endpoint,
        );

        analyzer
    }

    fn register_builtin(&mut self, name: &str, params: Vec<(String, Type)>, return_type: Type) {
        self.functions.insert(
            name.to_string(),
            FunctionSignature {
                name: name.to_string(),
                params,
                return_type,
                effects: Vec::new(),
                span: Span::zero(),
            },
        );
    }

    pub fn analyze_program(&mut self, program: &Program) -> Result<(), SemaError> {
        // First pass: collect function signatures
        for item in &program.items {
            if let TopLevelItem::Function(func) = item {
                let params = func
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), Type::from_annotation(&p.ty)))
                    .collect();
                let return_type = func
                    .return_type
                    .as_ref()
                    .map(Type::from_annotation)
                    .unwrap_or(Type::Unit);

                if self.functions.contains_key(&func.name) {
                    return Err(SemaError::DuplicateDeclaration {
                        name: func.name.clone(),
                        span: func.span,
                    });
                }

                self.functions.insert(
                    func.name.clone(),
                    FunctionSignature {
                        name: func.name.clone(),
                        params,
                        return_type,
                        effects: func.effects.clone(),
                        span: func.span,
                    },
                );
            }
        }

        // Reject declarations that have no implementation instead of silently ignoring them.
        for item in &program.items {
            match item {
                TopLevelItem::Struct(d) => {
                    return Err(SemaError::Unsupported {
                        feature: format!("struct '{}'", d.name),
                        span: d.span,
                    })
                }
                TopLevelItem::Enum(d) => {
                    return Err(SemaError::Unsupported {
                        feature: format!("enum '{}'", d.name),
                        span: d.span,
                    })
                }
                _ => {}
            }
        }

        let main_fn = program.items.iter().find_map(|i| match i {
            TopLevelItem::Function(f) if f.name == "main" => Some(f),
            _ => None,
        });
        let has_top_level_stmts = program
            .items
            .iter()
            .any(|i| matches!(i, TopLevelItem::Statement(_)));
        if let Some(m) = main_fn {
            if !m.params.is_empty() {
                return Err(SemaError::Invalid {
                    message: "`main` must not take parameters".to_string(),
                    span: m.span,
                });
            }
            if has_top_level_stmts {
                return Err(SemaError::Invalid {
                    message: "top-level statements cannot be combined with `fn main`".to_string(),
                    span: m.span,
                });
            }
        }

        // Second pass: constants, in source order, so every function body can use them.
        for item in &program.items {
            if let TopLevelItem::Const(c) = item {
                let val_ty = self.analyze_expression(&c.value)?;
                let ty = if let Some(ref anno) = c.ty {
                    let expected = Type::from_annotation(anno);
                    if !val_ty.is_assignable_to(&expected) {
                        return Err(SemaError::TypeMismatch {
                            expected: expected.to_string(),
                            found: val_ty.to_string(),
                            span: c.value.span,
                        });
                    }
                    expected
                } else {
                    val_ty
                };
                self.declare_symbol(c.name.clone(), ty, false, true, c.span)?;
            }
        }

        // Third pass: function bodies.
        for item in &program.items {
            if let TopLevelItem::Function(func) = item {
                self.analyze_function(func)?;
            }
        }

        // Finally top-level statements, in their own scope (functions cannot see them).
        self.enter_scope();
        for item in &program.items {
            if let TopLevelItem::Statement(stmt) = item {
                self.analyze_statement(stmt)?;
            }
        }
        self.leave_scope();

        Ok(())
    }

    fn analyze_function(&mut self, func: &FunctionDecl) -> Result<(), SemaError> {
        let expected_ret = func
            .return_type
            .as_ref()
            .map(Type::from_annotation)
            .unwrap_or(Type::Unit);

        let prev_ret = self.current_return_type.take();
        self.current_return_type = Some(expected_ret.clone());

        self.enter_scope();
        for param in &func.params {
            let ty = Type::from_annotation(&param.ty);
            self.declare_symbol(param.name.clone(), ty, param.is_mut, false, param.span)?;
        }

        self.analyze_block(&func.body)?;
        self.leave_scope();

        self.current_return_type = prev_ret;

        if expected_ret != Type::Unit && !block_always_returns(&func.body) {
            return Err(SemaError::MissingReturn {
                name: func.name.clone(),
                span: func.span,
            });
        }
        Ok(())
    }

    fn analyze_block(&mut self, block: &Block) -> Result<(), SemaError> {
        self.enter_scope();
        for stmt in &block.statements {
            self.analyze_statement(stmt)?;
        }
        self.leave_scope();
        Ok(())
    }

    fn analyze_statement(&mut self, stmt: &Stmt) -> Result<(), SemaError> {
        match stmt {
            Stmt::Let {
                name,
                ty,
                init,
                span,
            } => {
                let init_ty = self.analyze_expression(init)?;
                let final_ty = if let Some(ref anno) = ty {
                    let expected = Type::from_annotation(anno);
                    if !init_ty.is_assignable_to(&expected) {
                        return Err(SemaError::TypeMismatch {
                            expected: expected.to_string(),
                            found: init_ty.to_string(),
                            span: init.span,
                        });
                    }
                    expected
                } else {
                    init_ty
                };
                self.declare_symbol(name.clone(), final_ty, false, false, *span)?;
            }
            Stmt::Mut {
                name,
                ty,
                init,
                span,
            } => {
                let init_ty = self.analyze_expression(init)?;
                let final_ty = if let Some(ref anno) = ty {
                    let expected = Type::from_annotation(anno);
                    if !init_ty.is_assignable_to(&expected) {
                        return Err(SemaError::TypeMismatch {
                            expected: expected.to_string(),
                            found: init_ty.to_string(),
                            span: init.span,
                        });
                    }
                    expected
                } else {
                    init_ty
                };
                self.declare_symbol(name.clone(), final_ty, true, false, *span)?;
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                let sym = self.lookup_symbol(target, *span)?;
                if !sym.is_mut {
                    return Err(SemaError::CannotAssignImmutable {
                        name: target.clone(),
                        span: *span,
                    });
                }
                let val_ty = self.analyze_expression(value)?;
                if !val_ty.is_assignable_to(&sym.ty) {
                    return Err(SemaError::TypeMismatch {
                        expected: sym.ty.to_string(),
                        found: val_ty.to_string(),
                        span: value.span,
                    });
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span: _,
            } => {
                let cond_ty = self.analyze_expression(condition)?;
                if !cond_ty.is_assignable_to(&Type::Bool) {
                    return Err(SemaError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_ty.to_string(),
                        span: condition.span,
                    });
                }
                self.analyze_block(then_branch)?;
                if let Some(ref el) = else_branch {
                    self.analyze_block(el)?;
                }
            }
            Stmt::While {
                condition,
                body,
                span: _,
            } => {
                let cond_ty = self.analyze_expression(condition)?;
                if !cond_ty.is_assignable_to(&Type::Bool) {
                    return Err(SemaError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: cond_ty.to_string(),
                        span: condition.span,
                    });
                }
                self.analyze_block(body)?;
            }
            Stmt::Return { value, span } => {
                let val_ty = if let Some(ref v) = value {
                    self.analyze_expression(v)?
                } else {
                    Type::Unit
                };

                if let Some(ref expected) = self.current_return_type {
                    if !val_ty.is_assignable_to(expected) {
                        return Err(SemaError::InvalidReturnType {
                            expected: expected.to_string(),
                            found: val_ty.to_string(),
                            span: *span,
                        });
                    }
                }
            }
            Stmt::Flow {
                name,
                source,
                stages,
                span,
            } => {
                let result_ty = self.analyze_pipeline(source, stages)?;
                self.declare_symbol(name.clone(), result_ty, false, false, *span)?;
            }
            Stmt::Expr(expr) => {
                self.analyze_expression(expr)?;
            }
        }
        Ok(())
    }

    pub fn analyze_expression(&mut self, expr: &Expr) -> Result<Type, SemaError> {
        let ty = match &expr.kind {
            ExprKind::Integer(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Text(_) => Type::Text,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Unit => Type::Unit,
            ExprKind::Identifier(name) => {
                if let Ok(sym) = self.lookup_symbol(name, expr.span) {
                    sym.ty
                } else if self.functions.contains_key(name) {
                    let sig = &self.functions[name];
                    Type::Function {
                        params: sig.params.iter().map(|(_, t)| t.clone()).collect(),
                        return_type: Box::new(sig.return_type.clone()),
                    }
                } else {
                    return Err(SemaError::UndefinedIdentifier {
                        name: name.clone(),
                        span: expr.span,
                    });
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.analyze_expression(left)?;
                let right_ty = self.analyze_expression(right)?;

                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => {
                        if left_ty == Type::Int && right_ty == Type::Int {
                            Type::Int
                        } else if left_ty == Type::Float
                            && right_ty == Type::Float
                            && *op != BinaryOp::Mod
                        {
                            Type::Float
                        } else if *op == BinaryOp::Add
                            && left_ty == Type::Text
                            && right_ty == Type::Text
                        {
                            Type::Text
                        } else {
                            return Err(SemaError::InvalidBinaryOp {
                                op: format!("{op:?}"),
                                left: left_ty.to_string(),
                                right: right_ty.to_string(),
                                span: expr.span,
                            });
                        }
                    }
                    BinaryOp::Eq | BinaryOp::Neq => {
                        if left_ty.is_assignable_to(&right_ty)
                            || right_ty.is_assignable_to(&left_ty)
                        {
                            Type::Bool
                        } else {
                            return Err(SemaError::InvalidBinaryOp {
                                op: format!("{op:?}"),
                                left: left_ty.to_string(),
                                right: right_ty.to_string(),
                                span: expr.span,
                            });
                        }
                    }
                    BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte => {
                        if left_ty.is_numeric() && right_ty.is_numeric() && left_ty == right_ty {
                            Type::Bool
                        } else {
                            return Err(SemaError::InvalidBinaryOp {
                                op: format!("{op:?}"),
                                left: left_ty.to_string(),
                                right: right_ty.to_string(),
                                span: expr.span,
                            });
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if left_ty == Type::Bool && right_ty == Type::Bool {
                            Type::Bool
                        } else {
                            return Err(SemaError::InvalidBinaryOp {
                                op: format!("{op:?}"),
                                left: left_ty.to_string(),
                                right: right_ty.to_string(),
                                span: expr.span,
                            });
                        }
                    }
                }
            }
            ExprKind::Unary { op, operand } => {
                let operand_ty = self.analyze_expression(operand)?;
                match op {
                    UnaryOp::Neg => {
                        if operand_ty.is_numeric() {
                            operand_ty
                        } else {
                            return Err(SemaError::InvalidUnaryOp {
                                op: "-".to_string(),
                                ty: operand_ty.to_string(),
                                span: expr.span,
                            });
                        }
                    }
                    UnaryOp::Not => {
                        if operand_ty == Type::Bool {
                            Type::Bool
                        } else {
                            return Err(SemaError::InvalidUnaryOp {
                                op: "!".to_string(),
                                ty: operand_ty.to_string(),
                                span: expr.span,
                            });
                        }
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                let ExprKind::Identifier(name) = &callee.kind else {
                    return Err(SemaError::Unsupported {
                        feature: "calling a non-identifier expression".to_string(),
                        span: expr.span,
                    });
                };
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.analyze_expression(arg)?);
                }
                let Some(sig) = self.functions.get(name).cloned() else {
                    return Err(if self.lookup_symbol(name, expr.span).is_ok() {
                        SemaError::Invalid {
                            message: format!("'{name}' is a variable, not a function"),
                            span: expr.span,
                        }
                    } else {
                        SemaError::UndefinedIdentifier {
                            name: name.clone(),
                            span: expr.span,
                        }
                    });
                };
                if sig.params.len() != args.len() {
                    return Err(SemaError::ArgumentCountMismatch {
                        name: name.clone(),
                        expected: sig.params.len(),
                        found: args.len(),
                        span: expr.span,
                    });
                }
                for (i, (arg_ty, (_, param_ty))) in
                    arg_types.iter().zip(sig.params.iter()).enumerate()
                {
                    // Endpoint(ip, port) also accepts an Int port; the VM range-checks it.
                    let ok = arg_ty.is_assignable_to(param_ty)
                        || (name == "Endpoint"
                            && i == 1
                            && matches!(arg_ty, Type::Int | Type::Port));
                    if !ok {
                        return Err(SemaError::TypeMismatch {
                            expected: param_ty.to_string(),
                            found: arg_ty.to_string(),
                            span: args[i].span,
                        });
                    }
                }
                sig.return_type
            }
            ExprKind::MethodCall { .. } => {
                return Err(SemaError::Unsupported {
                    feature: "method calls".to_string(),
                    span: expr.span,
                });
            }
            ExprKind::Array(elements) => {
                if elements.is_empty() {
                    Type::Array(Box::new(Type::Unknown))
                } else {
                    let first_ty = self.analyze_expression(&elements[0])?;
                    for el in &elements[1..] {
                        let el_ty = self.analyze_expression(el)?;
                        if !el_ty.is_assignable_to(&first_ty) {
                            return Err(SemaError::TypeMismatch {
                                expected: first_ty.to_string(),
                                found: el_ty.to_string(),
                                span: el.span,
                            });
                        }
                    }
                    Type::Array(Box::new(first_ty))
                }
            }
            ExprKind::Index { collection, index } => {
                let col_ty = self.analyze_expression(collection)?;
                let idx_ty = self.analyze_expression(index)?;
                if idx_ty != Type::Int {
                    return Err(SemaError::TypeMismatch {
                        expected: "Int".to_string(),
                        found: idx_ty.to_string(),
                        span: index.span,
                    });
                }
                match col_ty {
                    Type::Array(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        return Err(SemaError::TypeMismatch {
                            expected: "Array<_>".to_string(),
                            found: other.to_string(),
                            span: collection.span,
                        })
                    }
                }
            }
            ExprKind::FieldAccess { .. } => {
                return Err(SemaError::Unsupported {
                    feature: "field access".to_string(),
                    span: expr.span,
                });
            }
            ExprKind::Pipe {
                expr: source,
                stages,
            } => self.analyze_pipeline(source, stages)?,
            ExprKind::Lambda { .. } => {
                return Err(SemaError::Unsupported {
                    feature: "lambda expressions".to_string(),
                    span: expr.span,
                });
            }
        };

        self.expression_types.insert(expr.span, ty.clone());
        Ok(ty)
    }

    /// `x |> f |> g(a)` is typed as `g(f(x), a)`; `|> emit` yields Unit.
    fn analyze_pipeline(&mut self, source: &Expr, stages: &[PipeStage]) -> Result<Type, SemaError> {
        let mut current = self.analyze_expression(source)?;
        for stage in stages {
            let Some(sig) = self.functions.get(&stage.op_name).cloned() else {
                return Err(SemaError::UndefinedIdentifier {
                    name: stage.op_name.clone(),
                    span: stage.span,
                });
            };
            let mut arg_types = vec![current.clone()];
            if let Some(arg) = &stage.arg {
                arg_types.push(self.analyze_expression(arg)?);
            }
            if sig.params.len() != arg_types.len() {
                return Err(SemaError::ArgumentCountMismatch {
                    name: stage.op_name.clone(),
                    expected: sig.params.len(),
                    found: arg_types.len(),
                    span: stage.span,
                });
            }
            for (arg_ty, (_, param_ty)) in arg_types.iter().zip(sig.params.iter()) {
                if !arg_ty.is_assignable_to(param_ty) {
                    return Err(SemaError::TypeMismatch {
                        expected: param_ty.to_string(),
                        found: arg_ty.to_string(),
                        span: stage.span,
                    });
                }
            }
            current = sig.return_type;
        }
        Ok(current)
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn leave_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_symbol(
        &mut self,
        name: String,
        ty: Type,
        is_mut: bool,
        is_constant: bool,
        span: Span,
    ) -> Result<(), SemaError> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.symbols.contains_key(&name) {
                return Err(SemaError::DuplicateDeclaration { name, span });
            }
            scope.symbols.insert(
                name.clone(),
                Symbol {
                    name,
                    ty,
                    is_mut,
                    is_constant,
                    span,
                },
            );
        }
        Ok(())
    }

    fn lookup_symbol(&self, name: &str, span: Span) -> Result<Symbol, SemaError> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.symbols.get(name) {
                return Ok(sym.clone());
            }
        }
        Err(SemaError::UndefinedIdentifier {
            name: name.to_string(),
            span,
        })
    }
}

/// True when every path through the block ends in `return`.
fn block_always_returns(block: &Block) -> bool {
    block.statements.iter().any(|s| match s {
        Stmt::Return { .. } => true,
        Stmt::If {
            then_branch,
            else_branch: Some(el),
            ..
        } => block_always_returns(then_branch) && block_always_returns(el),
        _ => false,
    })
}
