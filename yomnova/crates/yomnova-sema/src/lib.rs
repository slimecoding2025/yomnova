use std::collections::{HashMap, HashSet};
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
    /// `Some` while collecting several errors per run (see `analyze_program_all`).
    collected: Option<Vec<SemaError>>,
    /// Declared structs: name -> fields in declaration order.
    structs: HashMap<String, Vec<(String, Type)>>,
    /// Enum name -> variants (name, carried value type).
    enums: HashMap<String, Vec<(String, Option<Type>)>>,
    /// Names whose declaration failed; later "undefined" errors for them are cascades.
    poisoned: HashSet<String>,
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
            collected: None,
            structs: HashMap::new(),
            enums: HashMap::new(),
            poisoned: HashSet::new(),
        };

        // Register built-in functions
        analyzer.register_builtin("emit", vec![("val".to_string(), Type::Unknown)], Type::Unit);
        // Generic over the element type; the exact result types are computed at the call site.
        analyzer.register_builtin("len", vec![("value".to_string(), Type::Unknown)], Type::Int);
        analyzer.register_builtin(
            "push",
            vec![
                ("array".to_string(), Type::Unknown),
                ("value".to_string(), Type::Unknown),
            ],
            Type::Unknown,
        );
        analyzer.register_builtin("IPv4", vec![("addr".to_string(), Type::Text)], Type::IPv4);
        analyzer.register_builtin("IPv6", vec![("addr".to_string(), Type::Text)], Type::IPv6);
        analyzer.register_builtin("Port", vec![("port".to_string(), Type::Int)], Type::Port);
        for (name, params, ret) in yomnova_types::net_builtins() {
            let params = params
                .into_iter()
                .enumerate()
                .map(|(i, t)| (format!("arg{i}"), t))
                .collect();
            analyzer.register_builtin(name, params, ret);
        }
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

    /// Maximum number of errors reported in one run.
    pub const MAX_ERRORS: usize = 20;

    /// Analyse the whole program and return every error found (recovery is per statement).
    ///
    /// A failed `let`/`mut` poisons its name so that later uses do not produce a pile of
    /// follow-on "undefined identifier" errors.
    pub fn analyze_program_all(&mut self, program: &Program) -> Vec<SemaError> {
        self.collected = Some(Vec::new());
        self.poisoned.clear();
        let fatal = self.analyze_program(program).err();
        let mut errors = self.collected.take().unwrap_or_default();
        if let Some(e) = fatal {
            errors.push(e);
        }
        errors.truncate(Self::MAX_ERRORS);
        errors
    }

    /// Run one statement; in collecting mode record its error and carry on.
    fn guard_statement(&mut self, stmt: &Stmt) -> Result<(), SemaError> {
        let depth = self.scopes.len();
        match self.analyze_statement(stmt) {
            Ok(()) => Ok(()),
            Err(e) if self.collected.is_some() => {
                self.scopes.truncate(depth);
                if let Stmt::Let { name, .. } | Stmt::Mut { name, .. } = stmt {
                    self.poisoned.insert(name.clone());
                }
                let cascade = matches!(&e, SemaError::UndefinedIdentifier { name, .. } if self.poisoned.contains(name));
                if !cascade {
                    if let Some(list) = self.collected.as_mut() {
                        list.push(e);
                    }
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Run one function body; same recovery rule as `guard_statement`.
    fn guard_function(&mut self, func: &FunctionDecl) -> Result<(), SemaError> {
        let depth = self.scopes.len();
        match self.analyze_function(func) {
            Ok(()) => Ok(()),
            Err(e) if self.collected.is_some() => {
                self.scopes.truncate(depth);
                self.current_return_type = None;
                if let Some(list) = self.collected.as_mut() {
                    list.push(e);
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// A parameter may be `&T` / `&mut T`; the referenced type is validated like any other.
    fn validate_param_annotation(
        &self,
        anno: &TypeAnnotation,
        span: Span,
    ) -> Result<(), SemaError> {
        match anno {
            TypeAnnotation::Ref(inner, _) => self.validate_annotation(inner, span),
            other => self.validate_annotation(other, span),
        }
    }

    /// A type annotation may only name built-in types or declared structs.
    fn validate_annotation(&self, anno: &TypeAnnotation, span: Span) -> Result<(), SemaError> {
        match anno {
            TypeAnnotation::Array(inner) => self.validate_annotation(inner, span),
            TypeAnnotation::Ref(..) => Err(SemaError::Invalid {
                message: "references (&T, &mut T) are only allowed as function parameter types"
                    .to_string(),
                span,
            }),
            TypeAnnotation::Custom(name)
                if name != "NetworkBuffer"
                    && !self.structs.contains_key(name)
                    && !self.enums.contains_key(name) =>
            {
                Err(SemaError::Invalid {
                    message: format!("unknown type '{name}'"),
                    span,
                })
            }
            _ => Ok(()),
        }
    }

    fn register_structs(&mut self, program: &Program) -> Result<(), SemaError> {
        // Names first, so structs may refer to each other regardless of order.
        let mut seen = HashSet::new();
        for item in &program.items {
            if let TopLevelItem::Struct(d) = item {
                if !seen.insert(d.name.clone()) || self.enums.contains_key(&d.name) {
                    return Err(SemaError::DuplicateDeclaration {
                        name: d.name.clone(),
                        span: d.span,
                    });
                }
                self.structs.insert(d.name.clone(), Vec::new());
            }
        }
        for item in &program.items {
            if let TopLevelItem::Struct(d) = item {
                let mut fields: Vec<(String, Type)> = Vec::new();
                for f in &d.fields {
                    if fields.iter().any(|(n, _)| *n == f.name) {
                        return Err(SemaError::DuplicateDeclaration {
                            name: f.name.clone(),
                            span: f.span,
                        });
                    }
                    self.validate_annotation(&f.ty, f.span)?;
                    if matches!(&f.ty, TypeAnnotation::Custom(n) if *n == d.name) {
                        return Err(SemaError::Invalid {
                            message: format!(
                                "struct '{}' contains itself, so it would have infinite size",
                                d.name
                            ),
                            span: f.span,
                        });
                    }
                    fields.push((f.name.clone(), Type::from_annotation(&f.ty)));
                }
                self.structs.insert(d.name.clone(), fields);
            }
        }
        Ok(())
    }

    /// Enum names first (so structs can use them), then their variants once every type name is known.
    fn register_enum_names(&mut self, program: &Program) -> Result<(), SemaError> {
        for item in &program.items {
            if let TopLevelItem::Enum(d) = item {
                if self.enums.contains_key(&d.name) {
                    return Err(SemaError::DuplicateDeclaration {
                        name: d.name.clone(),
                        span: d.span,
                    });
                }
                self.enums.insert(d.name.clone(), Vec::new());
            }
        }
        Ok(())
    }

    fn register_enum_variants(&mut self, program: &Program) -> Result<(), SemaError> {
        for item in &program.items {
            if let TopLevelItem::Enum(d) = item {
                let mut variants: Vec<(String, Option<Type>)> = Vec::new();
                for v in &d.variants {
                    if variants.iter().any(|(n, _)| *n == v.name) {
                        return Err(SemaError::DuplicateDeclaration {
                            name: v.name.clone(),
                            span: v.span,
                        });
                    }
                    let payload = match &v.payload {
                        Some(a) => {
                            self.validate_annotation(a, v.span)?;
                            if matches!(a, TypeAnnotation::Custom(n) if *n == d.name) {
                                return Err(SemaError::Invalid {
                                    message: format!(
                                        "enum '{}' contains itself, so it would have infinite size",
                                        d.name
                                    ),
                                    span: v.span,
                                });
                            }
                            Some(Type::from_annotation(a))
                        }
                        None => None,
                    };
                    variants.push((v.name.clone(), payload));
                }
                if variants.is_empty() {
                    return Err(SemaError::Invalid {
                        message: format!("enum '{}' has no variants", d.name),
                        span: d.span,
                    });
                }
                self.enums.insert(d.name.clone(), variants);
            }
        }
        Ok(())
    }

    /// `Name` when `expr` is a bare identifier that names an enum and is not shadowed by a variable.
    fn enum_name_of(&self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Identifier(n)
                if self.enums.contains_key(n) && self.lookup_symbol(n, expr.span).is_err() =>
            {
                Some(n.clone())
            }
            _ => None,
        }
    }

    pub fn analyze_program(&mut self, program: &Program) -> Result<(), SemaError> {
        self.register_enum_names(program)?;
        self.register_structs(program)?;
        self.register_enum_variants(program)?;
        // First pass: collect function signatures
        for item in &program.items {
            if let TopLevelItem::Function(func) = item {
                for p in &func.params {
                    self.validate_param_annotation(&p.ty, p.span)?;
                    if matches!(p.ty, TypeAnnotation::Ref(..)) && p.is_mut {
                        return Err(SemaError::Invalid {
                            message: format!(
                                "reference parameter '{}' cannot also be declared `mut`; use `&mut` instead",
                                p.name
                            ),
                            span: p.span,
                        });
                    }
                }
                if let Some(r) = &func.return_type {
                    self.validate_annotation(r, func.span)?;
                }
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
                    self.validate_annotation(anno, c.span)?;
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
                self.guard_function(func)?;
            }
        }

        // Finally top-level statements, in their own scope (functions cannot see them).
        self.enter_scope();
        for item in &program.items {
            if let TopLevelItem::Statement(stmt) = item {
                self.guard_statement(stmt)?;
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
            // Inside the body a reference parameter reads as the value it refers to; it is writable only
            // through `&mut`.
            let (ty, is_mut) = match Type::from_annotation(&param.ty) {
                Type::Ref(inner, m) => (*inner, m),
                other => (other, param.is_mut),
            };
            self.declare_symbol(param.name.clone(), ty, is_mut, false, param.span)?;
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
            self.guard_statement(stmt)?;
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
                    self.validate_annotation(anno, *span)?;
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
                    self.validate_annotation(anno, *span)?;
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
            Stmt::AssignPlace {
                target,
                value,
                span,
            } => {
                let root = place_root(target).unwrap_or_default().to_string();
                let sym = self.lookup_symbol(&root, *span)?;
                if !sym.is_mut {
                    return Err(SemaError::CannotAssignImmutable {
                        name: root,
                        span: *span,
                    });
                }
                if place_depth(target) > 1 && place_has_call(target) {
                    return Err(SemaError::Invalid {
                        message: "an index in a nested assignment target must not contain a call"
                            .to_string(),
                        span: target.span,
                    });
                }
                let place_ty = self.analyze_expression(target)?;
                let val_ty = self.analyze_expression(value)?;
                if !val_ty.is_assignable_to(&place_ty) {
                    return Err(SemaError::TypeMismatch {
                        expected: place_ty.to_string(),
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
            Stmt::Match {
                scrutinee,
                arms,
                span,
            } => {
                let sty = self.analyze_expression(scrutinee)?;
                let mut seen: Vec<&Pattern> = Vec::new();
                for arm in arms {
                    let binds = self.check_arm_pattern(&seen, &arm.pattern, &sty, arm.span)?;
                    if arm.guard.is_none() {
                        seen.push(&arm.pattern);
                    }
                    self.enter_scope();
                    let declared = self.declare_pattern_bindings(binds, arm.span);
                    let result = declared
                        .and_then(|_| self.analyze_guard(arm.guard.as_ref()))
                        .and_then(|_| self.analyze_block(&arm.body));
                    self.leave_scope();
                    result?;
                }
                self.check_exhaustive(&seen, &sty, *span)?;
            }
            Stmt::For {
                var,
                iterable,
                body,
                span,
            } => {
                let it_ty = self.analyze_expression(iterable)?;
                let elem = match it_ty {
                    Type::Array(t) => *t,
                    Type::Unknown => Type::Unknown,
                    other => {
                        return Err(SemaError::TypeMismatch {
                            expected: "Array<_>".to_string(),
                            found: other.to_string(),
                            span: iterable.span,
                        })
                    }
                };
                self.enter_scope();
                let declared = self.declare_symbol(var.clone(), elem, false, false, *span);
                let result = declared.and_then(|_| self.analyze_block(body));
                self.leave_scope();
                result?;
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

    #[inline(never)]
    fn analyze_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchExprArm],
        span: Span,
    ) -> Result<Type, SemaError> {
        let sty = self.analyze_expression(scrutinee)?;
        if arms.is_empty() {
            return Err(SemaError::Invalid {
                message: "a match expression needs at least one arm".to_string(),
                span: span,
            });
        }
        let mut seen: Vec<&Pattern> = Vec::new();
        let mut result_ty: Option<Type> = None;
        for arm in arms {
            let binds = self.check_arm_pattern(&seen, &arm.pattern, &sty, arm.span)?;
            if arm.guard.is_none() {
                seen.push(&arm.pattern);
            }
            self.enter_scope();
            let declared = self.declare_pattern_bindings(binds, arm.span);
            let body = declared
                .and_then(|_| self.analyze_guard(arm.guard.as_ref()))
                .and_then(|_| self.analyze_expression(&arm.body));
            self.leave_scope();
            let body = body?;
            match &result_ty {
                None => result_ty = Some(body),
                Some(first) => {
                    if !body.is_assignable_to(first) {
                        return Err(SemaError::TypeMismatch {
                            expected: first.to_string(),
                            found: body.to_string(),
                            span: arm.body.span,
                        });
                    }
                    if *first == Type::Unknown {
                        result_ty = Some(body);
                    }
                }
            }
        }
        self.check_exhaustive(&seen, &sty, span)?;
        Ok(result_ty.unwrap_or(Type::Unknown))
    }

    /// A match guard must be a Bool; it is checked with the arm's bindings in scope.
    fn analyze_guard(&mut self, guard: Option<&Expr>) -> Result<(), SemaError> {
        if let Some(g) = guard {
            let t = self.analyze_expression(g)?;
            if t != Type::Bool && t != Type::Unknown {
                return Err(SemaError::TypeMismatch {
                    expected: "Bool".to_string(),
                    found: t.to_string(),
                    span: g.span,
                });
            }
        }
        Ok(())
    }

    pub fn analyze_expression(&mut self, expr: &Expr) -> Result<Type, SemaError> {
        let ty = match &expr.kind {
            ExprKind::Match { scrutinee, arms } => {
                self.analyze_match_expr(scrutinee, arms, expr.span)?
            }
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
                    let t = match &arg.kind {
                        ExprKind::Borrow { operand, mutable } => {
                            self.analyze_borrow(operand, *mutable)?
                        }
                        _ => self.analyze_expression(arg)?,
                    };
                    arg_types.push(t);
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
                match name.as_str() {
                    "len" => {
                        if !matches!(
                            arg_types[0],
                            Type::Array(_) | Type::Text | Type::NetworkBuffer | Type::Unknown
                        ) {
                            return Err(SemaError::TypeMismatch {
                                expected: "Array<_>, Text or NetworkBuffer".to_string(),
                                found: arg_types[0].to_string(),
                                span: args[0].span,
                            });
                        }
                        Type::Int
                    }
                    "push" => match &arg_types[0] {
                        Type::Array(elem) => {
                            if **elem == Type::Unknown {
                                Type::Array(Box::new(arg_types[1].clone()))
                            } else if arg_types[1].is_assignable_to(elem) {
                                Type::Array(elem.clone())
                            } else {
                                return Err(SemaError::TypeMismatch {
                                    expected: elem.to_string(),
                                    found: arg_types[1].to_string(),
                                    span: args[1].span,
                                });
                            }
                        }
                        Type::Unknown => Type::Array(Box::new(arg_types[1].clone())),
                        other => {
                            return Err(SemaError::TypeMismatch {
                                expected: "Array<_>".to_string(),
                                found: other.to_string(),
                                span: args[0].span,
                            })
                        }
                    },
                    _ => sig.return_type,
                }
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let Some(ename) = self.enum_name_of(receiver) else {
                    return Err(SemaError::Unsupported {
                        feature: "method calls".to_string(),
                        span: expr.span,
                    });
                };
                let variants = self.enums[&ename].clone();
                let Some((_, payload)) = variants.iter().find(|(n, _)| n == method) else {
                    return Err(SemaError::Invalid {
                        message: format!("enum '{ename}' has no variant '{method}'"),
                        span: expr.span,
                    });
                };
                match payload {
                    None => {
                        return Err(SemaError::Invalid {
                            message: format!(
                                "variant '{ename}.{method}' carries no value; write it without parentheses"
                            ),
                            span: expr.span,
                        })
                    }
                    Some(want) => {
                        if args.len() != 1 {
                            return Err(SemaError::ArgumentCountMismatch {
                                name: format!("{ename}.{method}"),
                                expected: 1,
                                found: args.len(),
                                span: expr.span,
                            });
                        }
                        let got = self.analyze_expression(&args[0])?;
                        if !got.is_assignable_to(want) {
                            return Err(SemaError::TypeMismatch {
                                expected: want.to_string(),
                                found: got.to_string(),
                                span: args[0].span,
                            });
                        }
                    }
                }
                Type::Custom(ename)
            }
            ExprKind::Borrow { .. } => {
                return Err(SemaError::Invalid {
                    message:
                        "a borrow (`&x`, `&mut x`) is only allowed as a function-call argument"
                            .to_string(),
                    span: expr.span,
                })
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
            ExprKind::FieldAccess { object, field } if self.enum_name_of(object).is_some() => {
                let ename = self.enum_name_of(object).unwrap_or_default();
                match self.enums[&ename].iter().find(|(n, _)| n == field) {
                    None => {
                        return Err(SemaError::Invalid {
                            message: format!("enum '{ename}' has no variant '{field}'"),
                            span: expr.span,
                        })
                    }
                    Some((_, Some(_))) => {
                        return Err(SemaError::Invalid {
                            message: format!(
                                "variant '{ename}.{field}' carries a value; write {ename}.{field}(value)"
                            ),
                            span: expr.span,
                        })
                    }
                    Some((_, None)) => Type::Custom(ename),
                }
            }
            ExprKind::FieldAccess { object, field } => {
                let obj_ty = self.analyze_expression(object)?;
                match &obj_ty {
                    Type::Custom(name) if self.structs.contains_key(name) => self.structs[name]
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, t)| t.clone())
                        .ok_or_else(|| SemaError::Invalid {
                            message: format!("struct '{name}' has no field '{field}'"),
                            span: expr.span,
                        })?,
                    Type::Unknown => Type::Unknown,
                    other => {
                        return Err(SemaError::Invalid {
                            message: format!("cannot read field '{field}' of type {other}"),
                            span: expr.span,
                        })
                    }
                }
            }
            ExprKind::StructLiteral { name, fields } => {
                let Some(decl) = self.structs.get(name).cloned() else {
                    return Err(SemaError::Invalid {
                        message: format!("unknown struct '{name}'"),
                        span: expr.span,
                    });
                };
                let mut given: HashSet<&str> = HashSet::new();
                for (fname, value) in fields {
                    let Some((_, want)) = decl.iter().find(|(n, _)| n == fname) else {
                        return Err(SemaError::Invalid {
                            message: format!("struct '{name}' has no field '{fname}'"),
                            span: value.span,
                        });
                    };
                    if !given.insert(fname.as_str()) {
                        return Err(SemaError::Invalid {
                            message: format!("field '{fname}' is given more than once"),
                            span: value.span,
                        });
                    }
                    let got = self.analyze_expression(value)?;
                    if !got.is_assignable_to(want) {
                        return Err(SemaError::TypeMismatch {
                            expected: want.to_string(),
                            found: got.to_string(),
                            span: value.span,
                        });
                    }
                }
                if let Some((missing, _)) = decl.iter().find(|(n, _)| !given.contains(n.as_str())) {
                    return Err(SemaError::Invalid {
                        message: format!("missing field '{missing}' in '{name}'"),
                        span: expr.span,
                    });
                }
                Type::Custom(name.clone())
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

    /// `&x` / `&mut x` as a call argument: the type is a reference to the variable's type.
    fn analyze_borrow(&mut self, operand: &Expr, mutable: bool) -> Result<Type, SemaError> {
        let ExprKind::Identifier(name) = &operand.kind else {
            return Err(SemaError::Invalid {
                message: "only a variable can be borrowed".to_string(),
                span: operand.span,
            });
        };
        let sym = self.lookup_symbol(name, operand.span)?;
        if mutable && !sym.is_mut {
            return Err(SemaError::Invalid {
                message: format!(
                    "cannot borrow '{name}' as mutable because it is not declared `mut`"
                ),
                span: operand.span,
            });
        }
        Ok(Type::Ref(Box::new(sym.ty), mutable))
    }

    /// Checks one arm's pattern against the matched type; returns the names it binds and their types.
    /// Rejects an arm that earlier arms already cover completely.
    fn check_arm_pattern(
        &self,
        prev: &[&Pattern],
        pat: &Pattern,
        ty: &Type,
        span: Span,
    ) -> Result<Vec<(String, Type)>, SemaError> {
        if prev.iter().any(|p| p.is_irrefutable()) {
            return Err(SemaError::Invalid {
                message: "this arm can never run: `_` above already matches everything".to_string(),
                span,
            });
        }
        let mut binds = Vec::new();
        self.check_pattern(pat, ty, span, &mut binds)?;
        if self.covers_pattern(prev, pat, ty) {
            return Err(SemaError::Invalid {
                message: "this arm can never run: earlier arms already cover every case it matches (a case is matched more than once)".to_string(),
                span,
            });
        }
        Ok(binds)
    }

    fn check_pattern(
        &self,
        pat: &Pattern,
        ty: &Type,
        span: Span,
        binds: &mut Vec<(String, Type)>,
    ) -> Result<(), SemaError> {
        let bad = |what: &str| SemaError::Invalid {
            message: format!("pattern {what} cannot match a value of type {ty}"),
            span,
        };
        match pat {
            Pattern::Wildcard => Ok(()),
            Pattern::Binding(n) => {
                binds.push((n.clone(), ty.clone()));
                Ok(())
            }
            Pattern::Int(n) => {
                if matches!(ty, Type::Int | Type::Unknown) {
                    Ok(())
                } else {
                    Err(bad(&n.to_string()))
                }
            }
            Pattern::Bool(b) => {
                if matches!(ty, Type::Bool | Type::Unknown) {
                    Ok(())
                } else {
                    Err(bad(&b.to_string()))
                }
            }
            Pattern::Variant {
                enum_name,
                variant,
                payload,
            } => {
                let ename = match ty {
                    Type::Custom(n) if self.enums.contains_key(n) => n.clone(),
                    Type::Unknown => enum_name.clone(),
                    other => {
                        return Err(SemaError::TypeMismatch {
                            expected: format!("a value matching {enum_name}.{variant}"),
                            found: other.to_string(),
                            span,
                        })
                    }
                };
                if *enum_name != ename {
                    return Err(SemaError::Invalid {
                        message: format!(
                            "pattern is for enum '{enum_name}' but the matched value is '{ename}'"
                        ),
                        span,
                    });
                }
                let Some(variants) = self.enums.get(&ename) else {
                    return Err(SemaError::Invalid {
                        message: format!("unknown enum '{ename}'"),
                        span,
                    });
                };
                let Some((_, carried)) = variants.iter().find(|(n, _)| n == variant) else {
                    return Err(SemaError::Invalid {
                        message: format!("enum '{ename}' has no variant '{variant}'"),
                        span,
                    });
                };
                match (carried, payload) {
                    (Some(t), Some(p)) => self.check_pattern(p, t, span, binds),
                    (Some(_), None) => Err(SemaError::Invalid {
                        message: format!("variant '{ename}.{variant}' carries a value; write {ename}.{variant}(name) or {ename}.{variant}(_)"),
                        span,
                    }),
                    (None, Some(_)) => Err(SemaError::Invalid {
                        message: format!("variant '{ename}.{variant}' carries no value; remove the parentheses"),
                        span,
                    }),
                    (None, None) => Ok(()),
                }
            }
        }
    }

    fn declare_pattern_bindings(
        &mut self,
        binds: Vec<(String, Type)>,
        span: Span,
    ) -> Result<(), SemaError> {
        for (name, ty) in binds {
            self.declare_symbol(name, ty, false, false, span)?;
        }
        Ok(())
    }

    /// Do the patterns together match every value of type `ty`?
    fn covers(&self, pats: &[&Pattern], ty: &Type) -> bool {
        if pats.iter().any(|p| p.is_irrefutable()) {
            return true;
        }
        match ty {
            Type::Custom(e) => {
                let Some(variants) = self.enums.get(e) else {
                    return false;
                };
                variants.iter().all(|(v, carried)| {
                    let rows: Vec<&Pattern> = pats
                        .iter()
                        .copied()
                        .filter(|p| matches!(p, Pattern::Variant { variant, .. } if variant == v))
                        .collect();
                    if rows.is_empty() {
                        return false;
                    }
                    match carried {
                        Some(t) => {
                            let subs: Vec<&Pattern> = rows
                                .iter()
                                .filter_map(|p| match p {
                                    Pattern::Variant {
                                        payload: Some(b), ..
                                    } => Some(&**b),
                                    _ => None,
                                })
                                .collect();
                            self.covers(&subs, t)
                        }
                        None => true,
                    }
                })
            }
            Type::Bool => {
                pats.iter().any(|p| matches!(p, Pattern::Bool(true)))
                    && pats.iter().any(|p| matches!(p, Pattern::Bool(false)))
            }
            _ => false,
        }
    }

    /// Do the earlier patterns already match every value that `pat` matches?
    fn covers_pattern(&self, prev: &[&Pattern], pat: &Pattern, ty: &Type) -> bool {
        match pat {
            Pattern::Wildcard | Pattern::Binding(_) => self.covers(prev, ty),
            Pattern::Int(n) => prev
                .iter()
                .any(|p| p.is_irrefutable() || matches!(p, Pattern::Int(m) if m == n)),
            Pattern::Bool(b) => prev
                .iter()
                .any(|p| p.is_irrefutable() || matches!(p, Pattern::Bool(c) if c == b)),
            Pattern::Variant {
                enum_name: _,
                variant,
                payload,
            } => {
                let wild = Pattern::Wildcard;
                let mut subs: Vec<&Pattern> = Vec::new();
                let mut any_row = false;
                for p in prev {
                    match p {
                        Pattern::Variant {
                            variant: v2,
                            payload: p2,
                            ..
                        } if v2 == variant => {
                            any_row = true;
                            subs.push(p2.as_deref().unwrap_or(&wild));
                        }
                        p if p.is_irrefutable() => {
                            any_row = true;
                            subs.push(&wild);
                        }
                        _ => {}
                    }
                }
                if !any_row {
                    return false;
                }
                match (payload, ty) {
                    (Some(sub), Type::Custom(e)) => {
                        let carried = self
                            .enums
                            .get(e)
                            .and_then(|vs| vs.iter().find(|(n, _)| n == variant))
                            .and_then(|(_, t)| t.clone());
                        match carried {
                            Some(t) => self.covers_pattern(&subs, sub, &t),
                            None => true,
                        }
                    }
                    _ => true,
                }
            }
        }
    }

    fn check_exhaustive(&self, pats: &[&Pattern], ty: &Type, span: Span) -> Result<(), SemaError> {
        if self.covers(pats, ty) {
            return Ok(());
        }
        let hint = match ty {
            Type::Custom(e) if self.enums.contains_key(e) => {
                let missing: Vec<String> = self.enums[e]
                    .iter()
                    .filter(|(v, _)| {
                        !pats
                            .iter()
                            .any(|p| matches!(p, Pattern::Variant { variant, .. } if variant == v))
                    })
                    .map(|(v, _)| format!("{e}.{v}"))
                    .collect();
                if missing.is_empty() {
                    "some nested cases are not covered".to_string()
                } else {
                    format!("missing {}", missing.join(", "))
                }
            }
            Type::Bool => "both `true` and `false` must be covered".to_string(),
            _ => "literal patterns never cover every value".to_string(),
        };
        Err(SemaError::Invalid {
            message: format!("match is not exhaustive: {hint} (add the arms or a `_` arm)"),
            span,
        })
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
        // Sema has already required the arms to be exhaustive.
        Stmt::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|a| block_always_returns(&a.body))
        }
        _ => false,
    })
}

/// The variable at the root of an index/field chain such as `a[i].f[j]`.
fn place_root(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Identifier(n) => Some(n),
        ExprKind::Index { collection, .. } => place_root(collection),
        ExprKind::FieldAccess { object, .. } => place_root(object),
        _ => None,
    }
}

/// Number of index/field steps in a place chain.
fn place_depth(e: &Expr) -> usize {
    match &e.kind {
        ExprKind::Index { collection, .. } => 1 + place_depth(collection),
        ExprKind::FieldAccess { object, .. } => 1 + place_depth(object),
        _ => 0,
    }
}

fn expr_has_call(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Call { .. } | ExprKind::MethodCall { .. } | ExprKind::Pipe { .. } => true,
        ExprKind::Binary { left, right, .. } => expr_has_call(left) || expr_has_call(right),
        ExprKind::Unary { operand, .. } => expr_has_call(operand),
        ExprKind::Array(items) => items.iter().any(expr_has_call),
        ExprKind::Index { collection, index } => expr_has_call(collection) || expr_has_call(index),
        ExprKind::FieldAccess { object, .. } => expr_has_call(object),
        ExprKind::StructLiteral { fields, .. } => fields.iter().any(|(_, v)| expr_has_call(v)),
        _ => false,
    }
}

/// Whether any index expression inside a place chain contains a call.
fn place_has_call(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Index { collection, index } => expr_has_call(index) || place_has_call(collection),
        ExprKind::FieldAccess { object, .. } => place_has_call(object),
        _ => false,
    }
}
