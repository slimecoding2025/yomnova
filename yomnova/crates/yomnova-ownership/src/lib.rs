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
    #[error("Cannot borrow '{name}' at {span}: it is already borrowed or used in the same call")]
    BorrowConflict { name: String, span: Span },
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
            )
            .with_hint("A reference parameter can be read, but only a copy type can be taken out of it"),
            OwnershipError::BorrowConflict { name, span } => {
                Diagnostic::error(self.to_string(), Some(*span)).with_hint(format!(
                    "While '{name}' is mutably borrowed, nothing else in the call may use it; shared borrows may not coexist with a move or a `&mut`"
                ))
            }
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
    /// A reference parameter: reads are fine, moving a non-copy value out is not.
    pub borrowed: bool,
}

pub struct OwnershipChecker {
    scopes: Vec<HashMap<String, Binding>>,
    fn_returns: HashMap<String, Type>,
    /// `Some` while collecting several errors per run (see `check_program_all`).
    collected: Option<Vec<OwnershipError>>,
    /// Enum name -> variant name -> carried value type (`Unit` when the variant carries none).
    enums: HashMap<String, HashMap<String, Type>>,
}

impl OwnershipChecker {
    pub fn new() -> Self {
        let mut fn_returns = HashMap::new();
        fn_returns.insert("emit".to_string(), Type::Unit);
        fn_returns.insert("len".to_string(), Type::Int);
        fn_returns.insert("IPv4".to_string(), Type::IPv4);
        fn_returns.insert("IPv6".to_string(), Type::IPv6);
        fn_returns.insert("Port".to_string(), Type::Port);
        fn_returns.insert("Endpoint".to_string(), Type::Endpoint);
        for (name, _, ret) in yomnova_types::net_builtins() {
            fn_returns.insert(name.to_string(), ret);
        }
        Self {
            scopes: vec![HashMap::new()],
            fn_returns,
            collected: None,
            enums: HashMap::new(),
        }
    }
}

impl Default for OwnershipChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnershipChecker {
    /// Maximum number of errors reported in one run.
    pub const MAX_ERRORS: usize = 20;

    /// Check the whole program and return every ownership error found (recovery is per
    /// statement and per function).
    pub fn check_program_all(&mut self, program: &Program) -> Vec<OwnershipError> {
        self.collected = Some(Vec::new());
        let fatal = self.check_program(program).err();
        let mut errors = self.collected.take().unwrap_or_default();
        if let Some(e) = fatal {
            errors.push(e);
        }
        errors.truncate(Self::MAX_ERRORS);
        errors
    }

    fn guard_statement(&mut self, stmt: &Stmt) -> Result<(), OwnershipError> {
        let depth = self.scopes.len();
        match self.check_statement(stmt) {
            Ok(()) => Ok(()),
            Err(e) if self.collected.is_some() => {
                self.scopes.truncate(depth);
                if let Some(list) = self.collected.as_mut() {
                    list.push(e);
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// `Some(enum name)` when `expr` is a bare name of an enum that no variable shadows.
    fn enum_ref(&self, expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Identifier(n) if self.enums.contains_key(n) && self.lookup(n).is_none() => {
                Some(n.clone())
            }
            _ => None,
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), OwnershipError> {
        for item in &program.items {
            if let TopLevelItem::Enum(d) = item {
                let variants = d
                    .variants
                    .iter()
                    .map(|v| {
                        (
                            v.name.clone(),
                            v.payload.as_ref().map_or(Type::Unit, Type::from_annotation),
                        )
                    })
                    .collect();
                self.enums.insert(d.name.clone(), variants);
            }
        }
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
                TopLevelItem::Statement(stmt) => self.guard_statement(stmt)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn check_function(&mut self, func: &FunctionDecl) -> Result<(), OwnershipError> {
        self.enter_scope();
        for param in &func.params {
            let full = Type::from_annotation(&param.ty);
            let borrowed = matches!(full, Type::Ref(..));
            let ty = full.referent().clone();
            let is_copy = Self::is_copy_type(&ty);
            self.declare(param.name.clone(), ty, is_copy);
            if borrowed {
                if let Some(b) = self.lookup_mut(&param.name) {
                    b.borrowed = true;
                }
            }
        }
        self.check_block(&func.body)?;
        self.leave_scope();
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<(), OwnershipError> {
        self.enter_scope();
        for stmt in &block.statements {
            self.guard_statement(stmt)?;
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
            ExprKind::Call { callee, args } => match &callee.kind {
                // `push(xs, v)` returns the array it was given, extended.
                ExprKind::Identifier(n) if n == "push" && !args.is_empty() => self.infer(&args[0]),
                ExprKind::Identifier(n) => self.fn_returns.get(n).cloned().unwrap_or(Type::Unknown),
                _ => Type::Unknown,
            },
            // Bindings of the arms are not in scope here, so use the first arm whose type is evident.
            ExprKind::Match { arms, .. } => arms
                .iter()
                .map(|a| self.infer(&a.body))
                .find(|t| *t != Type::Unknown)
                .unwrap_or(Type::Unknown),
            ExprKind::StructLiteral { name, .. } => Type::Custom(name.clone()),
            ExprKind::FieldAccess { object, .. }
            | ExprKind::MethodCall {
                receiver: object, ..
            } if self.enum_ref(object).is_some() => {
                Type::Custom(self.enum_ref(object).unwrap_or_default())
            }
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
            Stmt::AssignPlace { target, value, .. } => {
                self.check_expr_move(value)?;
                // The variable must still be usable: writing into a moved value is an error.
                self.check_expr_use(target)?;
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
                self.check_moved_in_loop(&before)?;
            }
            Stmt::Match {
                scrutinee, arms, ..
            } => {
                // Matching borrows the value; a bound payload is a fresh binding for that arm.
                self.check_expr_use(scrutinee)?;
                let sty = self.infer(scrutinee);
                let before = self.scopes.clone();
                let mut after_arms = Vec::new();
                for arm in arms {
                    self.scopes = before.clone();
                    self.enter_scope();
                    self.declare_pattern(&arm.pattern, &sty);
                    let result = self.check_block(&arm.body);
                    self.leave_scope();
                    result?;
                    after_arms.push(self.scopes.clone());
                }
                // A value moved in any arm is "maybe moved" after the match.
                self.scopes = before;
                for after in &after_arms {
                    for (scope, arm_scope) in self.scopes.iter_mut().zip(after.iter()) {
                        for (name, binding) in scope.iter_mut() {
                            if let Some(Binding {
                                state: VariableState::Moved { at },
                                ..
                            }) = arm_scope.get(name)
                            {
                                if binding.state == VariableState::Owned {
                                    binding.state = VariableState::Moved { at: *at };
                                }
                            }
                        }
                    }
                }
            }
            Stmt::For {
                var,
                iterable,
                body,
                ..
            } => {
                // Iterating borrows the array; each element is bound to `var` for one iteration.
                self.check_expr_use(iterable)?;
                let elem = match self.infer(iterable) {
                    Type::Array(t) => *t,
                    _ => Type::Unknown,
                };
                let before = self.scopes.clone();
                self.enter_scope();
                let is_copy = Self::is_copy_type(&elem);
                self.declare(var.clone(), elem, is_copy);
                let result = self.check_block(body);
                self.leave_scope();
                result?;
                self.check_moved_in_loop(&before)?;
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
    /// Anything declared outside a loop that was owned before the body and is moved after it
    /// would be used again on the next iteration.
    fn check_moved_in_loop(
        &self,
        before: &[HashMap<String, Binding>],
    ) -> Result<(), OwnershipError> {
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
                if binding.borrowed && !binding.is_copy && binding.ty != Type::Unknown {
                    return Err(OwnershipError::CannotMoveOut {
                        name: name.clone(),
                        span: expr.span,
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
                // `emit` and `len` only borrow their argument; every other call consumes identifier arguments.
                let borrows = matches!(&callee.kind, ExprKind::Identifier(n)
                    if n == "emit" || n == "len" || yomnova_types::NET_READ_ONLY.contains(&n.as_str()));
                self.check_borrow_conflicts(args)?;
                for arg in args {
                    if borrows || matches!(arg.kind, ExprKind::Borrow { .. }) {
                        self.check_expr_use(arg)?;
                    } else {
                        self.check_expr_move(arg)?;
                    }
                }
            }
            ExprKind::Borrow { operand, .. } => self.check_expr_use(operand)?,
            ExprKind::Match { scrutinee, arms } => {
                self.check_expr_use(scrutinee)?;
                let sty = self.infer(scrutinee);
                let before = self.scopes.clone();
                let mut after_arms = Vec::new();
                for arm in arms {
                    self.scopes = before.clone();
                    self.enter_scope();
                    self.declare_pattern(&arm.pattern, &sty);
                    // The arm's value flows out of the match, so a plain variable is moved.
                    let result = self.check_expr_move(&arm.body);
                    self.leave_scope();
                    result?;
                    after_arms.push(self.scopes.clone());
                }
                self.scopes = before;
                self.merge_arm_moves(&after_arms);
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.check_expr_use(receiver)?;
                // `Enum.Variant(value)` takes ownership of its value.
                let constructs = self.enum_ref(receiver).is_some();
                for arg in args {
                    if constructs {
                        self.check_expr_move(arg)?;
                    } else {
                        self.check_expr_use(arg)?;
                    }
                }
            }
            ExprKind::Array(elements) => {
                for el in elements {
                    self.check_expr_move(el)?;
                }
            }
            // Building a struct moves the values put into it.
            ExprKind::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.check_expr_move(value)?;
                }
            }
            // Reading a field is a use of the struct; it does not move the struct.
            ExprKind::FieldAccess { object, .. } => self.check_expr_use(object)?,
            ExprKind::Index { collection, index } => {
                self.check_expr_use(collection)?;
                self.check_expr_use(index)?;
            }
            ExprKind::Pipe { expr: src, stages } => self.check_pipeline(src, stages)?,
            _ => {}
        }
        Ok(())
    }

    /// Borrow rules for one call (borrows live only as call arguments, so this is the whole borrow checker):
    /// a `&mut x` excludes every other use of `x` in the call; a `&x` excludes a by-value move of `x` and `&mut x`.
    fn check_borrow_conflicts(&self, args: &[Expr]) -> Result<(), OwnershipError> {
        for (i, arg) in args.iter().enumerate() {
            let ExprKind::Borrow { operand, mutable } = &arg.kind else {
                continue;
            };
            let ExprKind::Identifier(name) = &operand.kind else {
                continue;
            };
            for (j, other) in args.iter().enumerate() {
                if i == j {
                    continue;
                }
                let clash = if *mutable {
                    mentions(other, name)
                } else {
                    match &other.kind {
                        ExprKind::Identifier(n) => {
                            n == name && self.lookup(n).is_some_and(|b| !b.is_copy)
                        }
                        ExprKind::Borrow {
                            operand: o,
                            mutable: true,
                        } => matches!(&o.kind, ExprKind::Identifier(n) if n == name),
                        _ => false,
                    }
                };
                if clash {
                    return Err(OwnershipError::BorrowConflict {
                        name: name.clone(),
                        span: other.span,
                    });
                }
            }
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

    /// A value moved in any arm is "maybe moved" after the match.
    fn merge_arm_moves(&mut self, after_arms: &[Vec<HashMap<String, Binding>>]) {
        for after in after_arms {
            for (scope, arm_scope) in self.scopes.iter_mut().zip(after.iter()) {
                for (name, binding) in scope.iter_mut() {
                    if let Some(Binding {
                        state: VariableState::Moved { at },
                        ..
                    }) = arm_scope.get(name)
                    {
                        if binding.state == VariableState::Owned {
                            binding.state = VariableState::Moved { at: *at };
                        }
                    }
                }
            }
        }
    }

    /// Declares the names a pattern binds, with the type of the part of the value each one matches.
    fn declare_pattern(&mut self, pat: &Pattern, ty: &Type) {
        match pat {
            Pattern::Binding(name) => {
                let is_copy = Self::is_copy_type(ty);
                self.declare(name.clone(), ty.clone(), is_copy);
            }
            Pattern::Variant {
                enum_name,
                variant,
                payload: Some(p),
            } => {
                let inner = self
                    .enums
                    .get(enum_name)
                    .and_then(|vs| vs.get(variant))
                    .cloned()
                    .unwrap_or(Type::Unknown);
                self.declare_pattern(p, &inner);
            }
            _ => {}
        }
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
                    borrowed: false,
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

/// Whether `name` is used anywhere inside `e`.
fn mentions(e: &Expr, name: &str) -> bool {
    match &e.kind {
        ExprKind::Identifier(n) => n == name,
        ExprKind::Binary { left, right, .. } => mentions(left, name) || mentions(right, name),
        ExprKind::Unary { operand, .. } | ExprKind::Borrow { operand, .. } => {
            mentions(operand, name)
        }
        ExprKind::Match { scrutinee, arms } => {
            mentions(scrutinee, name) || arms.iter().any(|a| mentions(&a.body, name))
        }
        ExprKind::Call { callee, args } => {
            mentions(callee, name) || args.iter().any(|a| mentions(a, name))
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            mentions(receiver, name) || args.iter().any(|a| mentions(a, name))
        }
        ExprKind::Array(items) => items.iter().any(|a| mentions(a, name)),
        ExprKind::Index { collection, index } => {
            mentions(collection, name) || mentions(index, name)
        }
        ExprKind::FieldAccess { object, .. } => mentions(object, name),
        ExprKind::StructLiteral { fields, .. } => fields.iter().any(|(_, v)| mentions(v, name)),
        ExprKind::Pipe { expr, stages } => {
            mentions(expr, name)
                || stages
                    .iter()
                    .any(|s| s.arg.as_ref().is_some_and(|a| mentions(a, name)))
        }
        _ => false,
    }
}
