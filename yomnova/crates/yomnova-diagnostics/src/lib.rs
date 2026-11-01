//! Diagnostic facilities, source spans, and error reports for YOMNOVA compiler.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Self {
            start,
            end,
            line,
            col,
        }
    }

    pub const fn zero() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            col: if self.line <= other.line {
                self.col
            } else {
                other.col
            },
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn warning(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn render(&self, source: Option<&str>, filename: Option<&str>) -> String {
        let file = filename.unwrap_or("<input>");
        let mut out = String::new();
        let level_str = match self.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Note => "note",
            DiagnosticLevel::Help => "help",
        };

        if let Some(span) = self.span {
            out.push_str(&format!(
                "{file}:{}:{}: {level_str}: {}\n",
                span.line, span.col, self.message
            ));
            if let Some(src) = source {
                let lines: Vec<&str> = src.lines().collect();
                if span.line > 0 && span.line <= lines.len() {
                    let line_content = lines[span.line - 1];
                    out.push_str(&format!("{:5} | {}\n", span.line, line_content));
                    let indent = " ".repeat(span.col.saturating_sub(1));
                    let caret_len = (span.end.saturating_sub(span.start)).max(1);
                    let carets = "^".repeat(caret_len);
                    out.push_str(&format!("      | {indent}{carets}\n"));
                }
            }
        } else {
            out.push_str(&format!("{file}: {level_str}: {}\n", self.message));
        }

        if let Some(ref hint) = self.hint {
            out.push_str(&format!("  = hint: {}\n", hint));
        }

        out
    }
}
