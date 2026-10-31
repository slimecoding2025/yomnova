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
}

impl SemaError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.to_string(), Some(self.span()))
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
        analyzer.register_builtin("IPv6", vec![("addr".to_string(), Type::Text)], Type::IPv6);
        analyzer.register_builtin("Port", vec![("port".to_string(), Type::Int)], Type::Port);
        analyzer.register_builtin(
            "Endpoint",
            vec![("ip".to_string(), Type::IPv4), ("port".to_string(), Type::Port)],
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

        // Second pass: analyze function bodies and top-level statements
        for item in &program.items {
            match item {
                TopLevelItem::Function(func) => self.analyze_function(func)?,
                TopLevelItem::Statement(stmt) => self.analyze_statement(stmt)?,
                TopLevelItem::Const(c) => {
                    let val_ty = self.analyze_expression(&c.value)?;
                    if let Some(ref anno) = c.ty {
                        let expected = Type::from_annotation(anno);
                        if !val_ty.is_assignable_to(&expected) {
                            return Err(SemaError::TypeMismatch {
                                expected: expected.to_string(),
                                found: val_ty.to_string(),
                                span: c.value.span,
                            });
                        }
                    }
                    self.declare_symbol(c.name.clone(), val_ty, false, true, c.span)?;
                }
                TopLevelItem::Struct(_) | TopLevelItem::Enum(_) => {}
            }
        }

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
            Stmt::Let { name, ty, init, span } => {
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
            Stmt::Mut { name, ty, init, span } => {
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
            Stmt::Assign { target, value, span } => {
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
            Stmt::If { condition, then_branch, else_branch, span: _ } => {
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
            Stmt::While { condition, body, span: _ } => {
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
            Stmt::Flow { name, source, stages: _, span } => {
                let src_ty = self.analyze_expression(source)?;
                // Flow stage output retains array or transformed collection
                self.declare_symbol(name.clone(), src_ty, false, false, *span)?;
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
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                        if left_ty == Type::Int && right_ty == Type::Int {
                            Type::Int
                        } else if left_ty == Type::Float && right_ty == Type::Float {
                            Type::Float
                        } else if *op == BinaryOp::Add && left_ty == Type::Text && right_ty == Type::Text {
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
                        if left_ty.is_assignable_to(&right_ty) || right_ty.is_assignable_to(&left_ty) {
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
                        if left_ty.is_numeric() && right_ty.is_numeric() {
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
                let callee_name = if let ExprKind::Identifier(ref name) = callee.kind {
                    Some(name.clone())
                } else {
                    None
                };

                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.analyze_expression(arg)?);
                }

                if let Some(name) = callee_name {
                    if let Some(sig) = self.functions.get(&name).cloned() {
                        if name != "emit" && sig.params.len() != args.len() {
                            return Err(SemaError::ArgumentCountMismatch {
                                name,
                                expected: sig.params.len(),
                                found: args.len(),
                                span: expr.span,
                            });
                        }
                        sig.return_type
                    } else if let Ok(sym) = self.lookup_symbol(&name, expr.span) {
                        if let Type::Function { return_type, .. } = sym.ty {
                            *return_type
                        } else {
                            Type::Unknown
                        }
                    } else {
                        return Err(SemaError::UndefinedIdentifier {
                            name,
                            span: expr.span,
                        });
                    }
                } else {
                    Type::Unknown
                }
            }
            ExprKind::MethodCall { receiver, method: _, args } => {
                let _recv_ty = self.analyze_expression(receiver)?;
                for arg in args {
                    self.analyze_expression(arg)?;
                }
                Type::Unknown
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
                    _ => Type::Unknown,
                }
            }
            ExprKind::FieldAccess { object, .. } => {
                self.analyze_expression(object)?;
                Type::Unknown
            }
            ExprKind::Pipe { expr: source, stages: _ } => {
                self.analyze_expression(source)?
            }
            ExprKind::Lambda { params, body } => {
                self.enter_scope();
                for p in params {
                    self.declare_symbol(p.clone(), Type::Unknown, false, false, expr.span)?;
                }
                let body_ty = self.analyze_expression(body)?;
                self.leave_scope();
                Type::Function {
                    params: vec![Type::Unknown; params.len()],
                    return_type: Box::new(body_ty),
                }
            }
        };

        self.expression_types.insert(expr.span, ty.clone());
        Ok(ty)
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
