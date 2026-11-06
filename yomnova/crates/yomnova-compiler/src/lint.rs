//! Warnings: problems that do not stop a program from running.
//!
//! The lint pass runs on a program that already type-checked and passed the ownership
//! check, so every name it sees is declared. Current warnings:
//!
//! * a `let`, `mut`, `flow`, `for` or `match` binding that is never read (names that start with
//!   `_` are exempt; an assignment is not a read);
//! * a statement that can never run because it follows `return` in the same block.

use yomnova_ast::*;
use yomnova_diagnostics::{Diagnostic, Span};

struct Var {
    name: String,
    span: Span,
    used: bool,
}

#[derive(Default)]
struct Lint {
    scopes: Vec<Vec<Var>>,
    out: Vec<Diagnostic>,
}

/// All warnings for `program`, ordered by position.
pub fn lint(program: &Program) -> Vec<Diagnostic> {
    let mut l = Lint::default();
    l.scopes.push(Vec::new());
    for item in &program.items {
        match item {
            TopLevelItem::Function(f) => {
                l.scopes.push(Vec::new());
                for p in &f.params {
                    // Parameters are part of a function's signature, so they are never reported.
                    l.declare_quiet(&p.name, p.span);
                }
                l.block_body(&f.body);
                l.pop();
            }
            TopLevelItem::Const(c) => l.expr(&c.value),
            TopLevelItem::Statement(s) => l.stmt(s),
            _ => {}
        }
    }
    l.pop();
    l.out
        .sort_by_key(|d| d.span.map_or((0, 0), |s| (s.line, s.col)));
    l.out
}

impl Lint {
    fn declare(&mut self, name: &str, span: Span) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Var {
                name: name.to_string(),
                span,
                used: name.starts_with('_'),
            });
        }
    }

    fn declare_quiet(&mut self, name: &str, span: Span) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Var {
                name: name.to_string(),
                span,
                used: true,
            });
        }
    }

    fn pop(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            for v in scope {
                if !v.used {
                    self.out.push(
                        Diagnostic::warning(
                            format!("variable '{}' is never read", v.name),
                            Some(v.span),
                        )
                        .with_hint(format!(
                            "use it, remove it, or rename it to '_{}' to silence this warning",
                            v.name
                        )),
                    );
                }
            }
        }
    }

    fn read(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(v) = scope.iter_mut().rev().find(|v| v.name == name) {
                v.used = true;
                return;
            }
        }
    }

    fn block(&mut self, block: &Block) {
        self.scopes.push(Vec::new());
        self.block_body(block);
        self.pop();
    }

    fn block_body(&mut self, block: &Block) {
        let mut returned = false;
        for stmt in &block.statements {
            if returned {
                self.out.push(
                    Diagnostic::warning(
                        "unreachable code: it follows a `return`",
                        Some(stmt.span()),
                    )
                    .with_hint("remove it, or move the `return` below it"),
                );
                // One warning per block is enough.
                returned = false;
            }
            self.stmt(stmt);
            if matches!(stmt, Stmt::Return { .. }) {
                returned = true;
            }
        }
    }

    /// The variable at the root of an assignment target is written, not read; index expressions are read.
    fn place(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Index { collection, index } => {
                self.expr(index);
                self.place(collection);
            }
            ExprKind::FieldAccess { object, .. } => self.place(object),
            _ => {}
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, init, span, ..
            }
            | Stmt::Mut {
                name, init, span, ..
            } => {
                self.expr(init);
                self.declare(name, *span);
            }
            Stmt::Assign { value, .. } => self.expr(value),
            Stmt::AssignPlace { target, value, .. } => {
                self.expr(value);
                self.place(target);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expr(condition);
                self.block(then_branch);
                if let Some(e) = else_branch {
                    self.block(e);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expr(condition);
                self.block(body);
            }
            Stmt::For {
                var,
                iterable,
                body,
                span,
            } => {
                self.expr(iterable);
                self.scopes.push(Vec::new());
                self.declare(var, *span);
                self.block(body);
                self.pop();
            }
            Stmt::Match {
                scrutinee, arms, ..
            } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.scopes.push(Vec::new());
                    if let Pattern::Variant {
                        binding: Some(name),
                        ..
                    } = &arm.pattern
                    {
                        self.declare(name, arm.span);
                    }
                    self.block(&arm.body);
                    self.pop();
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.expr(v);
                }
            }
            Stmt::Flow {
                name,
                source,
                stages,
                span,
            } => {
                self.expr(source);
                for st in stages {
                    if let Some(a) = &st.arg {
                        self.expr(a);
                    }
                }
                self.declare(name, *span);
            }
            Stmt::Expr(e) => self.expr(e),
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Identifier(n) => self.read(n),
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Unary { operand, .. } => self.expr(operand),
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                args.iter().for_each(|a| self.expr(a));
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.expr(receiver);
                args.iter().for_each(|a| self.expr(a));
            }
            ExprKind::Array(els) => els.iter().for_each(|e| self.expr(e)),
            ExprKind::Index { collection, index } => {
                self.expr(collection);
                self.expr(index);
            }
            ExprKind::FieldAccess { object, .. } => self.expr(object),
            ExprKind::StructLiteral { fields, .. } => fields.iter().for_each(|(_, e)| self.expr(e)),
            ExprKind::Pipe { expr, stages } => {
                self.expr(expr);
                for st in stages {
                    if let Some(a) = &st.arg {
                        self.expr(a);
                    }
                }
            }
            ExprKind::Lambda { body, .. } => self.expr(body),
            _ => {}
        }
    }
}
