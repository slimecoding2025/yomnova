//! Source formatter for `yomnova fmt`.
//!
//! This is a deliberately small, line-based re-indenter, not a full pretty-printer:
//! it normalises line endings, trailing whitespace, blank-line runs and brace
//! indentation (4 spaces). It never touches the inside of string literals or block
//! comments, and it is idempotent (`format_source(format_source(x)) == format_source(x)`).

#[derive(Clone, Copy, PartialEq)]
enum State {
    Code,
    Str,
    BlockComment,
}

/// Format YOMNOVA source text.
pub fn format_source(src: &str) -> String {
    let normalized = src.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(normalized.len());
    let mut depth: usize = 0;
    let mut state = State::Code;
    let mut blank_run = 0usize;

    for line in normalized.split('\n') {
        // Lines that begin inside a multi-line string or block comment are verbatim.
        if state != State::Code {
            out.push_str(line);
            out.push('\n');
            blank_run = 0;
            state = scan(line, state, &mut 0, &mut 0).0;
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run == 1 && !out.is_empty() {
                out.push('\n');
            }
            continue;
        }
        blank_run = 0;

        let leading_closes = trimmed.chars().take_while(|c| *c == '}').count();
        let mut opens = 0usize;
        let mut closes = 0usize;
        let (next_state, _) = scan(trimmed, State::Code, &mut opens, &mut closes);

        out.push_str(&"    ".repeat(depth.saturating_sub(leading_closes)));
        out.push_str(trimmed);
        out.push('\n');

        depth = (depth + opens).saturating_sub(closes);
        state = next_state;
    }

    // Exactly one trailing newline, no trailing blank lines.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if out == "\n" {
        out.clear();
    }
    out
}

/// Scan one line, counting braces that occur in code. Returns the state at end of line.
fn scan(line: &str, mut state: State, opens: &mut usize, closes: &mut usize) -> (State, ()) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match state {
            State::Code => match c {
                '"' => state = State::Str,
                '/' if chars.get(i + 1) == Some(&'/') => return (State::Code, ()),
                '/' if chars.get(i + 1) == Some(&'*') => {
                    state = State::BlockComment;
                    i += 1;
                }
                '{' => *opens += 1,
                '}' => *closes += 1,
                _ => {}
            },
            State::Str => match c {
                '\\' => i += 1,
                '"' => state = State::Code,
                _ => {}
            },
            State::BlockComment => {
                if c == '*' && chars.get(i + 1) == Some(&'/') {
                    state = State::Code;
                    i += 1;
                }
            }
        }
        i += 1;
    }
    (state, ())
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn indents_blocks() {
        let src = "module m\nfn main() -> Int {\nif true {\nemit(1)\n}\nreturn 0\n}\n";
        let expected = "module m\nfn main() -> Int {\n    if true {\n        emit(1)\n    }\n    return 0\n}\n";
        assert_eq!(format_source(src), expected);
    }

    #[test]
    fn else_chain_dedents() {
        let src = "fn f() -> Int {\nif a {\nreturn 1\n} else {\nreturn 2\n}\n}\n";
        let expected = "fn f() -> Int {\n    if a {\n        return 1\n    } else {\n        return 2\n    }\n}\n";
        assert_eq!(format_source(src), expected);
    }

    #[test]
    fn braces_in_strings_and_comments_are_ignored() {
        let src = "fn main() -> Int {\nemit(\"{{{\")\n// }}}\nreturn 0\n}\n";
        let expected = "fn main() -> Int {\n    emit(\"{{{\")\n    // }}}\n    return 0\n}\n";
        assert_eq!(format_source(src), expected);
    }

    #[test]
    fn multiline_string_and_block_comment_are_verbatim() {
        let src = "fn main() -> Int {\nemit(\"a\n   b {\n c\")\n/* x\n   {\n*/\nreturn 0\n}\n";
        let out = format_source(src);
        assert!(out.contains("\n   b {\n c\")"));
        assert!(out.contains("/* x\n   {\n*/"));
        assert!(out.ends_with("    return 0\n}\n"));
    }

    #[test]
    fn normalises_whitespace_and_is_idempotent() {
        let src = "\n\nfn a() {  \r\n\r\n\r\n\r\nreturn\r\n}\n\n\n";
        let once = format_source(src);
        assert_eq!(once, "fn a() {\n\n    return\n}\n");
        assert_eq!(format_source(&once), once);
    }

    #[test]
    fn empty_input() {
        assert_eq!(format_source(""), "");
        assert_eq!(format_source("\n\n"), "");
    }
}
