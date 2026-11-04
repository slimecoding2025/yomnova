use yomnova_ast::*;
use yomnova_diagnostics::{Diagnostic, Span};
use yomnova_lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParserError {
    #[error("Unexpected token '{token}' at {span}, expected {expected}")]
    UnexpectedToken {
        token: String,
        expected: String,
        span: Span,
    },
    #[error("Unexpected end of file, expected {expected}")]
    UnexpectedEof { expected: String, span: Span },
    #[error("Invalid assignment target at {span}")]
    InvalidAssignmentTarget { span: Span },
    #[error("Nesting too deep at {span} (limit is {limit})")]
    NestingTooDeep { limit: usize, span: Span },
}

impl ParserError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            ParserError::UnexpectedToken { span, .. } => {
                Diagnostic::error(self.to_string(), Some(*span))
            }
            ParserError::UnexpectedEof { span, .. } => {
                Diagnostic::error(self.to_string(), Some(*span))
            }
            ParserError::NestingTooDeep { span, .. } => {
                Diagnostic::error(self.to_string(), Some(*span))
                    .with_hint("Split the expression or block into smaller parts")
            }
            ParserError::InvalidAssignmentTarget { span } => {
                Diagnostic::error(self.to_string(), Some(*span))
                    .with_hint("Only identifiers or indexable paths can be assigned to")
            }
        }
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    depth: usize,
    /// When set, statement-level errors are recorded and parsing resumes at the next
    /// statement or declaration instead of stopping at the first error.
    recovering: bool,
    errors: Vec<ParserError>,
}

impl Parser {
    /// Maximum syntactic nesting (blocks, expressions, types). Prevents stack
    /// exhaustion on adversarial input.
    pub const MAX_NESTING: usize = 150;

    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            depth: 0,
            recovering: false,
            errors: Vec::new(),
        }
    }

    /// Upper bound on diagnostics collected in one run (keeps cascades readable).
    pub const MAX_ERRORS: usize = 20;

    fn enter(&mut self) -> Result<(), ParserError> {
        if self.depth >= Self::MAX_NESTING {
            return Err(ParserError::NestingTooDeep {
                limit: Self::MAX_NESTING,
                span: self.current_span(),
            });
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Parse the whole program, collecting every syntax error that can be found.
    ///
    /// Recovery happens at statement and declaration boundaries. The returned program is
    /// only meaningful when the error list is empty.
    pub fn parse_program_recovering(&mut self) -> (Program, Vec<ParserError>) {
        self.recovering = true;
        self.errors.clear();
        let result = self.parse_program();
        self.recovering = false;
        let mut errors = std::mem::take(&mut self.errors);
        let program = match result {
            Ok(p) => p,
            Err(e) => {
                errors.push(e);
                Program {
                    module_name: None,
                    items: Vec::new(),
                    span: self.current_span(),
                }
            }
        };
        errors.truncate(Self::MAX_ERRORS);
        (program, errors)
    }

    /// Record `e` and skip to a safe resume point, or give it back when recovery is off or
    /// pointless (end of input, nesting limit, error cap reached).
    fn recover(&mut self, e: ParserError, in_block: bool, decl: bool) -> Result<(), ParserError> {
        let fatal = matches!(
            e,
            ParserError::NestingTooDeep { .. } | ParserError::UnexpectedEof { .. }
        );
        if !self.recovering || fatal || self.errors.len() >= Self::MAX_ERRORS {
            return Err(e);
        }
        self.errors.push(e);
        self.synchronize(in_block, decl);
        Ok(())
    }

    /// Always makes progress, then stops before the next token that can start a statement
    /// (or declaration, when the failed item was one) or at a block's closing brace.
    fn synchronize(&mut self, in_block: bool, decl: bool) {
        if !self.is_at_end() && !(in_block && self.check(&TokenKind::CloseBrace)) {
            self.advance();
        }
        while !self.is_at_end() {
            if self.cursor > 0 && self.tokens[self.cursor - 1].kind == TokenKind::Semicolon {
                return;
            }
            let k = &self.tokens[self.cursor].kind;
            let stop = if decl {
                matches!(
                    k,
                    TokenKind::Fn | TokenKind::Struct | TokenKind::Enum | TokenKind::Const
                )
            } else {
                matches!(
                    k,
                    TokenKind::Fn
                        | TokenKind::Struct
                        | TokenKind::Enum
                        | TokenKind::Const
                        | TokenKind::Let
                        | TokenKind::Mut
                        | TokenKind::Flow
                        | TokenKind::Return
                        | TokenKind::If
                        | TokenKind::While
                        | TokenKind::For
                        | TokenKind::Match
                        | TokenKind::Emit
                        | TokenKind::CloseBrace
                )
            };
            if stop {
                return;
            }
            self.advance();
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParserError> {
        let start_span = self.current_span();
        let mut module_name = None;

        if self.check(&TokenKind::Module) {
            self.advance();
            let name_token = self.consume_identifier("module name")?;
            module_name = Some(name_token);
        }

        let mut items = Vec::new();
        while !self.is_at_end() {
            let decl = matches!(
                self.peek_kind(),
                Some(TokenKind::Fn | TokenKind::Struct | TokenKind::Enum | TokenKind::Const)
            );
            let depth = self.depth;
            match self.parse_top_level_item() {
                Ok(item) => items.push(item),
                Err(e) => {
                    self.depth = depth;
                    self.recover(e, false, decl)?;
                }
            }
        }

        let end_span = self.previous_span();
        let span = start_span.merge(end_span);

        Ok(Program {
            module_name,
            items,
            span,
        })
    }

    fn parse_top_level_item(&mut self) -> Result<TopLevelItem, ParserError> {
        if self.check(&TokenKind::Fn) {
            let func = self.parse_function()?;
            Ok(TopLevelItem::Function(func))
        } else if self.check(&TokenKind::Struct) {
            let s = self.parse_struct()?;
            Ok(TopLevelItem::Struct(s))
        } else if self.check(&TokenKind::Enum) {
            let e = self.parse_enum()?;
            Ok(TopLevelItem::Enum(e))
        } else if self.check(&TokenKind::Const) {
            let c = self.parse_const()?;
            Ok(TopLevelItem::Const(c))
        } else {
            let stmt = self.parse_statement()?;
            Ok(TopLevelItem::Statement(stmt))
        }
    }

    fn parse_function(&mut self) -> Result<FunctionDecl, ParserError> {
        let start_span = self.current_span();
        self.consume(&TokenKind::Fn, "'fn'")?;
        let name = self.consume_identifier("function name")?;

        self.consume(&TokenKind::OpenParen, "'('")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::CloseParen) {
            loop {
                let p_start = self.current_span();
                let is_mut = if self.check(&TokenKind::Mut) {
                    self.advance();
                    true
                } else {
                    false
                };
                let p_name = self.consume_identifier("parameter name")?;
                self.consume(&TokenKind::Colon, "':'")?;
                let ty = self.parse_type()?;
                let p_span = p_start.merge(self.previous_span());
                params.push(Param {
                    is_mut,
                    name: p_name,
                    ty,
                    span: p_span,
                });

                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.consume(&TokenKind::CloseParen, "')'")?;

        let mut return_type = None;
        if self.check(&TokenKind::Arrow) {
            self.advance();
            return_type = Some(self.parse_type()?);
        }

        let mut effects = Vec::new();
        if self.check(&TokenKind::BangBracket) {
            self.advance();
            while !self.check(&TokenKind::CloseBracket) && !self.is_at_end() {
                let mut eff = self.consume_identifier("effect name")?;
                while self.check(&TokenKind::Dot) {
                    self.advance();
                    eff.push('.');
                    eff.push_str(&self.consume_identifier("effect segment")?);
                }
                effects.push(eff);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.consume(&TokenKind::CloseBracket, "']'")?;
        }

        let body = self.parse_block()?;
        let span = start_span.merge(body.span);

        Ok(FunctionDecl {
            name,
            params,
            return_type,
            effects,
            body,
            span,
        })
    }

    fn parse_struct(&mut self) -> Result<StructDecl, ParserError> {
        let start_span = self.current_span();
        self.consume(&TokenKind::Struct, "'struct'")?;
        let name = self.consume_identifier("struct name")?;
        self.consume(&TokenKind::OpenBrace, "'{'")?;

        let mut fields = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            let f_start = self.current_span();
            let f_name = self.consume_identifier("field name")?;
            self.consume(&TokenKind::Colon, "':'")?;
            let ty = self.parse_type()?;
            let f_span = f_start.merge(self.previous_span());
            fields.push(StructField {
                name: f_name,
                ty,
                span: f_span,
            });

            if self.check(&TokenKind::Comma) {
                self.advance();
            }
        }
        self.consume(&TokenKind::CloseBrace, "'}'")?;
        let span = start_span.merge(self.previous_span());

        Ok(StructDecl { name, fields, span })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, ParserError> {
        let start_span = self.current_span();
        self.consume(&TokenKind::Enum, "'enum'")?;
        let name = self.consume_identifier("enum name")?;
        self.consume(&TokenKind::OpenBrace, "'{'")?;

        let mut variants = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            let v_start = self.current_span();
            let v_name = self.consume_identifier("variant name")?;
            let mut payload = None;
            if self.check(&TokenKind::OpenParen) {
                self.advance();
                payload = Some(self.parse_type()?);
                self.consume(&TokenKind::CloseParen, "')'")?;
            }
            let v_span = v_start.merge(self.previous_span());
            variants.push(EnumVariant {
                name: v_name,
                payload,
                span: v_span,
            });

            if self.check(&TokenKind::Comma) {
                self.advance();
            }
        }
        self.consume(&TokenKind::CloseBrace, "'}'")?;
        let span = start_span.merge(self.previous_span());

        Ok(EnumDecl {
            name,
            variants,
            span,
        })
    }

    fn parse_const(&mut self) -> Result<ConstDecl, ParserError> {
        let start_span = self.current_span();
        self.consume(&TokenKind::Const, "'const'")?;
        let name = self.consume_identifier("const name")?;

        let mut ty = None;
        if self.check(&TokenKind::Colon) {
            self.advance();
            ty = Some(self.parse_type()?);
        }

        self.consume(&TokenKind::Equal, "'='")?;
        let value = self.parse_expr()?;
        let span = start_span.merge(value.span);

        Ok(ConstDecl {
            name,
            ty,
            value,
            span,
        })
    }

    fn parse_block(&mut self) -> Result<Block, ParserError> {
        self.enter()?;
        let result = self.parse_block_inner();
        self.leave();
        result
    }

    fn parse_block_inner(&mut self) -> Result<Block, ParserError> {
        let start_span = self.current_span();
        self.consume(&TokenKind::OpenBrace, "'{'")?;

        let mut statements = Vec::new();
        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            let depth = self.depth;
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.depth = depth;
                    self.recover(e, true, false)?;
                }
            }
        }

        self.consume(&TokenKind::CloseBrace, "'}'")?;
        let span = start_span.merge(self.previous_span());

        Ok(Block { statements, span })
    }

    /// `_`, `Enum.Variant`, `Enum.Variant(name)` or `Enum.Variant(_)`.
    fn parse_pattern(&mut self) -> Result<Pattern, ParserError> {
        let first = self.consume_identifier("pattern")?;
        if first == "_" {
            return Ok(Pattern::Wildcard);
        }
        self.consume(&TokenKind::Dot, "'.' (patterns look like Enum.Variant)")?;
        let variant = self.consume_identifier("variant name")?;
        let mut parens = false;
        let mut binding = None;
        if self.check(&TokenKind::OpenParen) {
            self.advance();
            parens = true;
            let name = self.consume_identifier("binding name or '_'")?;
            if name != "_" {
                binding = Some(name);
            }
            self.consume(&TokenKind::CloseParen, "')'")?;
        }
        Ok(Pattern::Variant {
            enum_name: first,
            variant,
            parens,
            binding,
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParserError> {
        self.enter()?;
        let result = self.parse_statement_inner();
        self.leave();
        result
    }

    fn parse_statement_inner(&mut self) -> Result<Stmt, ParserError> {
        let start_span = self.current_span();

        if self.check(&TokenKind::Const) {
            // A local `const` is an immutable binding.
            let decl = self.parse_const()?;
            Ok(Stmt::Let {
                name: decl.name,
                ty: decl.ty,
                init: decl.value,
                span: decl.span,
            })
        } else if self.check(&TokenKind::Let) {
            self.advance();
            let name = self.consume_identifier("variable name")?;
            let mut ty = None;
            if self.check(&TokenKind::Colon) {
                self.advance();
                ty = Some(self.parse_type()?);
            }
            self.consume(&TokenKind::Equal, "'='")?;
            let init = self.parse_expr()?;
            let span = start_span.merge(init.span);
            Ok(Stmt::Let {
                name,
                ty,
                init,
                span,
            })
        } else if self.check(&TokenKind::Mut) {
            self.advance();
            let name = self.consume_identifier("mutable variable name")?;
            let mut ty = None;
            if self.check(&TokenKind::Colon) {
                self.advance();
                ty = Some(self.parse_type()?);
            }
            self.consume(&TokenKind::Equal, "'='")?;
            let init = self.parse_expr()?;
            let span = start_span.merge(init.span);
            Ok(Stmt::Mut {
                name,
                ty,
                init,
                span,
            })
        } else if self.check(&TokenKind::If) {
            self.advance();
            let condition = self.parse_expr()?;
            let then_branch = self.parse_block()?;
            let mut else_branch = None;

            if self.check(&TokenKind::Else) {
                self.advance();
                if self.check(&TokenKind::If) {
                    let if_stmt = self.parse_statement()?;
                    let s = if_stmt.span();
                    else_branch = Some(Block {
                        statements: vec![if_stmt],
                        span: s,
                    });
                } else {
                    else_branch = Some(self.parse_block()?);
                }
            }

            let end_span = else_branch
                .as_ref()
                .map(|b| b.span)
                .unwrap_or(then_branch.span);
            let span = start_span.merge(end_span);

            Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            })
        } else if self.check(&TokenKind::While) {
            self.advance();
            let condition = self.parse_expr()?;
            let body = self.parse_block()?;
            let span = start_span.merge(body.span);
            Ok(Stmt::While {
                condition,
                body,
                span,
            })
        } else if self.check(&TokenKind::Match) {
            self.advance();
            let scrutinee = self.parse_expr()?;
            self.consume(&TokenKind::OpenBrace, "'{'")?;
            let mut arms = Vec::new();
            while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
                let a_start = self.current_span();
                let pattern = self.parse_pattern()?;
                self.consume(&TokenKind::FatArrow, "'=>'")?;
                let body = self.parse_block()?;
                let span = a_start.merge(body.span);
                arms.push(MatchArm {
                    pattern,
                    body,
                    span,
                });
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            }
            self.consume(&TokenKind::CloseBrace, "'}'")?;
            let span = start_span.merge(self.previous_span());
            Ok(Stmt::Match {
                scrutinee,
                arms,
                span,
            })
        } else if self.check(&TokenKind::For) {
            self.advance();
            let var = self.consume_identifier("loop variable name")?;
            self.consume(&TokenKind::In, "'in'")?;
            let iterable = self.parse_expr()?;
            let body = self.parse_block()?;
            let span = start_span.merge(body.span);
            Ok(Stmt::For {
                var,
                iterable,
                body,
                span,
            })
        } else if self.check(&TokenKind::Return) {
            self.advance();
            let mut value = None;
            if !self.check(&TokenKind::CloseBrace)
                && !self.check(&TokenKind::Semicolon)
                && !self.is_at_end()
            {
                value = Some(self.parse_expr()?);
            }
            let span = if let Some(ref v) = value {
                start_span.merge(v.span)
            } else {
                start_span
            };
            if self.check(&TokenKind::Semicolon) {
                self.advance();
            }
            Ok(Stmt::Return { value, span })
        } else if self.check(&TokenKind::Flow) {
            self.advance();
            let name = self.consume_identifier("flow target name")?;
            self.consume(&TokenKind::Equal, "'='")?;
            let source = self.parse_expr_without_pipe()?;
            let mut stages = Vec::new();
            while self.check(&TokenKind::PipeGreater) {
                let p_start = self.current_span();
                self.advance();
                let op_name = self.consume_identifier("pipeline operator name")?;
                let mut arg = None;
                if self.check(&TokenKind::OpenParen) {
                    self.advance();
                    if !self.check(&TokenKind::CloseParen) {
                        arg = Some(self.parse_expr()?);
                    }
                    self.consume(&TokenKind::CloseParen, "')'")?;
                }
                let p_span = p_start.merge(self.previous_span());
                stages.push(PipeStage {
                    op_name,
                    arg,
                    span: p_span,
                });
            }
            let end_span = stages.last().map(|s| s.span).unwrap_or(source.span);
            let span = start_span.merge(end_span);
            Ok(Stmt::Flow {
                name,
                source,
                stages,
                span,
            })
        } else {
            // Check for assignment: id = expr
            if let Some(TokenKind::Identifier(ref name)) = self.peek_kind() {
                if let Some(TokenKind::Equal) = self.peek_second_kind() {
                    let var_name = name.clone();
                    self.advance(); // identifier
                    self.advance(); // '='
                    let value = self.parse_expr()?;
                    let span = start_span.merge(value.span);
                    return Ok(Stmt::Assign {
                        target: var_name,
                        value,
                        span,
                    });
                }
            }

            let expr = self.parse_expr()?;
            if self.check(&TokenKind::Semicolon) {
                self.advance();
            }
            Ok(Stmt::Expr(expr))
        }
    }

    pub fn parse_expr(&mut self) -> Result<Expr, ParserError> {
        self.enter()?;
        let result = self.parse_expr_inner();
        self.leave();
        result
    }

    fn parse_expr_inner(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.parse_binary_expr(0)?;

        if self.check(&TokenKind::PipeGreater) {
            let mut stages = Vec::new();
            while self.check(&TokenKind::PipeGreater) {
                let p_start = self.current_span();
                self.advance();
                let op_name = if self.check(&TokenKind::Emit) {
                    self.advance();
                    "emit".to_string()
                } else {
                    self.consume_identifier("pipe stage")?
                };
                let mut arg = None;
                if self.check(&TokenKind::OpenParen) {
                    self.advance();
                    if !self.check(&TokenKind::CloseParen) {
                        arg = Some(self.parse_expr()?);
                    }
                    self.consume(&TokenKind::CloseParen, "')'")?;
                }
                let p_span = p_start.merge(self.previous_span());
                stages.push(PipeStage {
                    op_name,
                    arg,
                    span: p_span,
                });
            }
            let end_span = stages.last().map(|s| s.span).unwrap_or(expr.span);
            let span = expr.span.merge(end_span);
            expr = Expr {
                kind: ExprKind::Pipe {
                    expr: Box::new(expr),
                    stages,
                },
                span,
            };
        }

        Ok(expr)
    }

    fn parse_expr_without_pipe(&mut self) -> Result<Expr, ParserError> {
        self.parse_binary_expr(0)
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Result<Expr, ParserError> {
        let mut left = self.parse_unary()?;

        while let Some((op, prec)) = self.current_binary_op() {
            if prec < min_prec {
                break;
            }
            self.advance();
            let right = self.parse_binary_expr(prec + 1)?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn current_binary_op(&self) -> Option<(BinaryOp, u8)> {
        match self.peek_kind()? {
            TokenKind::OrOr => Some((BinaryOp::Or, 1)),
            TokenKind::AndAnd => Some((BinaryOp::And, 2)),
            TokenKind::EqualEqual => Some((BinaryOp::Eq, 3)),
            TokenKind::BangEqual => Some((BinaryOp::Neq, 3)),
            TokenKind::Less => Some((BinaryOp::Lt, 4)),
            TokenKind::LessEqual => Some((BinaryOp::Lte, 4)),
            TokenKind::Greater => Some((BinaryOp::Gt, 4)),
            TokenKind::GreaterEqual => Some((BinaryOp::Gte, 4)),
            TokenKind::Plus => Some((BinaryOp::Add, 5)),
            TokenKind::Minus => Some((BinaryOp::Sub, 5)),
            TokenKind::Star => Some((BinaryOp::Mul, 6)),
            TokenKind::Slash => Some((BinaryOp::Div, 6)),
            TokenKind::Percent => Some((BinaryOp::Mod, 6)),
            _ => None,
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, ParserError> {
        self.enter()?;
        let result = self.parse_unary_inner();
        self.leave();
        result
    }

    fn parse_unary_inner(&mut self) -> Result<Expr, ParserError> {
        let start_span = self.current_span();
        if self.check(&TokenKind::Minus) {
            self.advance();
            let operand = self.parse_unary()?;
            let span = start_span.merge(operand.span);
            Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                },
                span,
            })
        } else if self.check(&TokenKind::Bang) {
            self.advance();
            let operand = self.parse_unary()?;
            let span = start_span.merge(operand.span);
            Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                },
                span,
            })
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParserError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(&TokenKind::OpenParen) {
                // Function call
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::CloseParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(&TokenKind::CloseParen, "')'")?;
                let span = expr.span.merge(self.previous_span());
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                };
            } else if self.check(&TokenKind::OpenBracket) {
                // Index access: expr[idx]
                self.advance();
                let index = self.parse_expr()?;
                self.consume(&TokenKind::CloseBracket, "']'")?;
                let span = expr.span.merge(self.previous_span());
                expr = Expr {
                    kind: ExprKind::Index {
                        collection: Box::new(expr),
                        index: Box::new(index),
                    },
                    span,
                };
            } else if self.check(&TokenKind::Dot) {
                // Method call or Field access: expr.field or expr.method(args)
                self.advance();
                let field_or_method = self.consume_identifier("field or method name")?;
                if self.check(&TokenKind::OpenParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::CloseParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(&TokenKind::CloseParen, "')'")?;
                    let span = expr.span.merge(self.previous_span());
                    expr = Expr {
                        kind: ExprKind::MethodCall {
                            receiver: Box::new(expr),
                            method: field_or_method,
                            args,
                        },
                        span,
                    };
                } else {
                    let span = expr.span.merge(self.previous_span());
                    expr = Expr {
                        kind: ExprKind::FieldAccess {
                            object: Box::new(expr),
                            field: field_or_method,
                        },
                        span,
                    };
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParserError> {
        let span = self.current_span();

        if let Some(kind) = self.peek_kind() {
            match kind {
                TokenKind::Integer(n) => {
                    let val = *n;
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Integer(val),
                        span,
                    })
                }
                TokenKind::Float(f) => {
                    let val = *f;
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Float(val),
                        span,
                    })
                }
                TokenKind::Text(s) => {
                    let val = s.clone();
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Text(val),
                        span,
                    })
                }
                TokenKind::Bool(b) => {
                    let val = *b;
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Bool(val),
                        span,
                    })
                }
                TokenKind::Emit => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("emit".to_string()),
                        span,
                    })
                }
                TokenKind::IPv4 => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("IPv4".to_string()),
                        span,
                    })
                }
                TokenKind::IPv6 => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("IPv6".to_string()),
                        span,
                    })
                }
                TokenKind::Port => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("Port".to_string()),
                        span,
                    })
                }
                TokenKind::Endpoint => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("Endpoint".to_string()),
                        span,
                    })
                }
                TokenKind::TCP => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("TCP".to_string()),
                        span,
                    })
                }
                TokenKind::UDP => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("UDP".to_string()),
                        span,
                    })
                }
                TokenKind::Packet => {
                    self.advance();
                    Ok(Expr {
                        kind: ExprKind::Identifier("Packet".to_string()),
                        span,
                    })
                }
                TokenKind::Identifier(id) => {
                    let name = id.clone();
                    self.advance();

                    // Check for lambda: x => expr
                    if self.check(&TokenKind::FatArrow) {
                        self.advance();
                        let body = self.parse_expr()?;
                        let full_span = span.merge(body.span);
                        return Ok(Expr {
                            kind: ExprKind::Lambda {
                                params: vec![name],
                                body: Box::new(body),
                            },
                            span: full_span,
                        });
                    }

                    // Struct literal: `Name { field: expr, … }`. Only an uppercase name followed by
                    // `{ ident :` is a literal, so `if x == Limit { … }` and `for a in Items { … }`
                    // keep meaning "expression, then block".
                    if name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                        && self.check(&TokenKind::OpenBrace)
                        && matches!(self.peek_at(1), Some(TokenKind::Identifier(_)))
                        && matches!(self.peek_at(2), Some(TokenKind::Colon))
                    {
                        self.advance(); // {
                        let mut fields = Vec::new();
                        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
                            let f = self.consume_identifier("field name")?;
                            self.consume(&TokenKind::Colon, "':'")?;
                            let v = self.parse_expr()?;
                            fields.push((f, v));
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        self.consume(&TokenKind::CloseBrace, "'}'")?;
                        return Ok(Expr {
                            kind: ExprKind::StructLiteral { name, fields },
                            span: span.merge(self.previous_span()),
                        });
                    }
                    Ok(Expr {
                        kind: ExprKind::Identifier(name),
                        span,
                    })
                }
                TokenKind::OpenBracket => {
                    // Array literal [1, 2, 3]
                    self.advance();
                    let mut elements = Vec::new();
                    if !self.check(&TokenKind::CloseBracket) {
                        loop {
                            elements.push(self.parse_expr()?);
                            if self.check(&TokenKind::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.consume(&TokenKind::CloseBracket, "']'")?;
                    let full_span = span.merge(self.previous_span());
                    Ok(Expr {
                        kind: ExprKind::Array(elements),
                        span: full_span,
                    })
                }
                TokenKind::OpenParen => {
                    // Could be unit (), parenthesized expression, or multi-arg lambda: (a, b) => expr
                    self.advance();
                    if self.check(&TokenKind::CloseParen) {
                        self.advance();
                        return Ok(Expr {
                            kind: ExprKind::Unit,
                            span: span.merge(self.previous_span()),
                        });
                    }

                    // Check if it's a lambda parameter list: (x, y) => ...
                    let mut is_lambda = false;
                    let mut lambda_params = Vec::new();

                    if let Some(TokenKind::Identifier(first_id)) = self.peek_kind() {
                        lambda_params.push(first_id.clone());
                        let mut temp_cur = self.cursor + 1;
                        while temp_cur < self.tokens.len() {
                            if self.tokens[temp_cur].kind == TokenKind::Comma {
                                temp_cur += 1;
                                if temp_cur < self.tokens.len() {
                                    if let TokenKind::Identifier(ref next_id) =
                                        self.tokens[temp_cur].kind
                                    {
                                        lambda_params.push(next_id.clone());
                                        temp_cur += 1;
                                    } else {
                                        break;
                                    }
                                }
                            } else if self.tokens[temp_cur].kind == TokenKind::CloseParen {
                                temp_cur += 1;
                                if temp_cur < self.tokens.len()
                                    && self.tokens[temp_cur].kind == TokenKind::FatArrow
                                {
                                    is_lambda = true;
                                    self.cursor = temp_cur + 1; // skip past '=>'
                                }
                                break;
                            } else {
                                break;
                            }
                        }
                    }

                    if is_lambda {
                        let body = self.parse_expr()?;
                        let full_span = span.merge(body.span);
                        return Ok(Expr {
                            kind: ExprKind::Lambda {
                                params: lambda_params,
                                body: Box::new(body),
                            },
                            span: full_span,
                        });
                    }

                    let expr = self.parse_expr()?;
                    self.consume(&TokenKind::CloseParen, "')'")?;
                    Ok(expr)
                }
                _ => Err(ParserError::UnexpectedToken {
                    token: format!("{}", kind),
                    expected: "expression".to_string(),
                    span,
                }),
            }
        } else {
            Err(ParserError::UnexpectedEof {
                expected: "expression".to_string(),
                span,
            })
        }
    }

    fn parse_type(&mut self) -> Result<TypeAnnotation, ParserError> {
        self.enter()?;
        let result = self.parse_type_inner();
        self.leave();
        result
    }

    fn parse_type_inner(&mut self) -> Result<TypeAnnotation, ParserError> {
        let span = self.current_span();
        if let Some(kind) = self.peek_kind() {
            let ty = match kind {
                TokenKind::Identifier(id) => match id.as_str() {
                    "Int" => {
                        self.advance();
                        TypeAnnotation::Int
                    }
                    "Float" => {
                        self.advance();
                        TypeAnnotation::Float
                    }
                    "Bool" => {
                        self.advance();
                        TypeAnnotation::Bool
                    }
                    "Text" => {
                        self.advance();
                        TypeAnnotation::Text
                    }
                    "Unit" => {
                        self.advance();
                        TypeAnnotation::Unit
                    }
                    "Array" => {
                        self.advance();
                        self.consume(&TokenKind::Less, "'<'")?;
                        let inner = self.parse_type()?;
                        self.consume(&TokenKind::Greater, "'>'")?;
                        TypeAnnotation::Array(Box::new(inner))
                    }
                    other => {
                        let custom = other.to_string();
                        self.advance();
                        TypeAnnotation::Custom(custom)
                    }
                },
                TokenKind::IPv4 => {
                    self.advance();
                    TypeAnnotation::IPv4
                }
                TokenKind::IPv6 => {
                    self.advance();
                    TypeAnnotation::IPv6
                }
                TokenKind::Port => {
                    self.advance();
                    TypeAnnotation::Port
                }
                TokenKind::Endpoint => {
                    self.advance();
                    TypeAnnotation::Endpoint
                }
                TokenKind::TCP => {
                    self.advance();
                    TypeAnnotation::TCP
                }
                TokenKind::UDP => {
                    self.advance();
                    TypeAnnotation::UDP
                }
                TokenKind::Packet => {
                    self.advance();
                    TypeAnnotation::Packet
                }
                _ => {
                    return Err(ParserError::UnexpectedToken {
                        token: format!("{}", kind),
                        expected: "type annotation".to_string(),
                        span,
                    });
                }
            };
            Ok(ty)
        } else {
            Err(ParserError::UnexpectedEof {
                expected: "type annotation".to_string(),
                span,
            })
        }
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len() || self.tokens[self.cursor].kind == TokenKind::Eof
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            &self.tokens[self.cursor].kind == kind
        }
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.cursor += 1;
        }
        &self.tokens[self.cursor - 1]
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        if self.is_at_end() {
            None
        } else {
            Some(&self.tokens[self.cursor].kind)
        }
    }

    /// Token kind `n` positions after the current one (`peek_at(0)` is the current token).
    fn peek_at(&self, n: usize) -> Option<&TokenKind> {
        self.tokens.get(self.cursor + n).map(|t| &t.kind)
    }

    fn peek_second_kind(&self) -> Option<&TokenKind> {
        if self.cursor + 1 < self.tokens.len() {
            Some(&self.tokens[self.cursor + 1].kind)
        } else {
            None
        }
    }

    fn consume(&mut self, kind: &TokenKind, expected: &str) -> Result<&Token, ParserError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let span = self.current_span();
            let actual = self
                .peek_kind()
                .map(|k| format!("{}", k))
                .unwrap_or_else(|| "<EOF>".to_string());
            Err(ParserError::UnexpectedToken {
                token: actual,
                expected: expected.to_string(),
                span,
            })
        }
    }

    fn consume_identifier(&mut self, expected: &str) -> Result<String, ParserError> {
        if let Some(TokenKind::Identifier(id)) = self.peek_kind() {
            let name = id.clone();
            self.advance();
            Ok(name)
        } else {
            let span = self.current_span();
            let actual = self
                .peek_kind()
                .map(|k| format!("{}", k))
                .unwrap_or_else(|| "<EOF>".to_string());
            Err(ParserError::UnexpectedToken {
                token: actual,
                expected: expected.to_string(),
                span,
            })
        }
    }

    fn current_span(&self) -> Span {
        if self.cursor < self.tokens.len() {
            self.tokens[self.cursor].span
        } else if let Some(last) = self.tokens.last() {
            last.span
        } else {
            Span::zero()
        }
    }

    fn previous_span(&self) -> Span {
        if self.cursor > 0 && self.cursor <= self.tokens.len() {
            self.tokens[self.cursor - 1].span
        } else {
            Span::zero()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yomnova_lexer::Lexer;

    #[test]
    fn test_parse_hello_world() {
        let src = r#"module genesis
fn main() -> Int {
    emit("Hello YOMNOVA")
    return 0
}"#;
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().expect("Parsing failed");

        assert_eq!(program.module_name, Some("genesis".to_string()));
        assert_eq!(program.items.len(), 1);
        if let TopLevelItem::Function(func) = &program.items[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.return_type, Some(TypeAnnotation::Int));
            assert_eq!(func.body.statements.len(), 2);
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_dataflow_and_variables() {
        let src = r#"
let x: Int = 10
mut y: Text = "initial"
flow result = [1, 2, 3] |> filter(x => x > 1) |> map(x => x * 2)
"#;
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().expect("Parsing failed");
        assert_eq!(program.items.len(), 3);
    }
}
