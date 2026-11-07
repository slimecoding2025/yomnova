//! Built-in standard library and `import std.<module>`.
//!
//! The standard library is written in YOMNOVA itself and expanded **before lexing**: each
//! `import std.math` line is blanked (so line numbers in diagnostics do not move), every
//! `math.abs(` in the program becomes the ordinary function `math_abs(`, and the source of
//! each *used* function is appended at the end. So there is no new runtime machinery: the
//! sema, ownership checker, compiler, optimiser and verifier see plain functions.
//!
//! Honest limits: only these built-in modules exist (no user modules, no files, no
//! `import foo`), names are always qualified (`math.abs`, never a bare `abs`), and the
//! array functions work on `Array<Int>` only because the language has no generics yet.

use yomnova_diagnostics::Diagnostic;

/// `(module, function, source)`; every function name is already `module_function`.
const FUNCTIONS: &[(&str, &str, &str)] = &[
    ("math", "abs", "fn math_abs(x: Int) -> Int {\n    if x < 0 {\n        return 0 - x\n    }\n    return x\n}"),
    ("math", "sign", "fn math_sign(x: Int) -> Int {\n    if x < 0 {\n        return 0 - 1\n    }\n    if x > 0 {\n        return 1\n    }\n    return 0\n}"),
    ("math", "min", "fn math_min(a: Int, b: Int) -> Int {\n    if a < b {\n        return a\n    }\n    return b\n}"),
    ("math", "max", "fn math_max(a: Int, b: Int) -> Int {\n    if a > b {\n        return a\n    }\n    return b\n}"),
    ("math", "clamp", "fn math_clamp(x: Int, lo: Int, hi: Int) -> Int {\n    if x < lo {\n        return lo\n    }\n    if x > hi {\n        return hi\n    }\n    return x\n}"),
    ("math", "pow", "fn math_pow(base: Int, exp: Int) -> Int {\n    mut r = 1\n    mut i = 0\n    while i < exp {\n        r = r * base\n        i = i + 1\n    }\n    return r\n}"),
    ("text", "upper", "fn text_upper(s: &Text) -> Text {\n    return str_upper(s)\n}"),
    ("text", "lower", "fn text_lower(s: &Text) -> Text {\n    return str_lower(s)\n}"),
    ("text", "trim", "fn text_trim(s: &Text) -> Text {\n    return str_trim(s)\n}"),
    ("text", "contains", "fn text_contains(s: &Text, part: Text) -> Bool {\n    return str_contains(s, part)\n}"),
    ("text", "starts_with", "fn text_starts_with(s: &Text, prefix: Text) -> Bool {\n    return str_starts_with(s, prefix)\n}"),
    ("text", "ends_with", "fn text_ends_with(s: &Text, suffix: Text) -> Bool {\n    return str_ends_with(s, suffix)\n}"),
    ("text", "find", "fn text_find(s: &Text, part: Text) -> Int {\n    return str_find(s, part)\n}"),
    ("text", "slice", "fn text_slice(s: &Text, from: Int, to: Int) -> Text {\n    return str_slice(s, from, to)\n}"),
    ("text", "repeat", "fn text_repeat(s: &Text, n: Int) -> Text {\n    return str_repeat(s, n)\n}"),
    ("text", "replace", "fn text_replace(s: &Text, from: Text, to: Text) -> Text {\n    return str_replace(s, from, to)\n}"),
    ("text", "split", "fn text_split(s: &Text, sep: Text) -> Array<Text> {\n    return str_split(s, sep)\n}"),
    ("text", "from_int", "fn text_from_int(n: Int) -> Text {\n    return str_from_int(n)\n}"),
    ("text", "parse_int", "fn text_parse_int(s: &Text) -> Int {\n    return str_parse_int(s)\n}"),
    ("arrays", "sum", "fn arrays_sum(xs: &Array<Int>) -> Int {\n    mut s = 0\n    for x in xs {\n        s = s + x\n    }\n    return s\n}"),
    ("arrays", "contains", "fn arrays_contains(xs: &Array<Int>, v: Int) -> Bool {\n    for x in xs {\n        if x == v {\n            return true\n        }\n    }\n    return false\n}"),
    ("arrays", "index_of", "fn arrays_index_of(xs: &Array<Int>, v: Int) -> Int {\n    mut i = 0\n    while i < len(xs) {\n        if xs[i] == v {\n            return i\n        }\n        i = i + 1\n    }\n    return 0 - 1\n}"),
    ("arrays", "max_of", "fn arrays_max_of(xs: &Array<Int>) -> Int {\n    mut m = xs[0]\n    for x in xs {\n        if x > m {\n            m = x\n        }\n    }\n    return m\n}"),
    ("arrays", "min_of", "fn arrays_min_of(xs: &Array<Int>) -> Int {\n    mut m = xs[0]\n    for x in xs {\n        if x < m {\n            m = x\n        }\n    }\n    return m\n}"),
];

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn err(line: usize, msg: String) -> Diagnostic {
    Diagnostic::error(
        msg,
        Some(yomnova_diagnostics::Span {
            start: 0,
            end: 0,
            line,
            col: 1,
        }),
    )
}

/// Expand imports. A program without `import` is returned unchanged.
pub fn expand(source: &str) -> Result<String, Vec<Diagnostic>> {
    if !source
        .lines()
        .any(|l| l.trim_start().starts_with("import "))
    {
        return Ok(source.to_string());
    }
    let mut modules: Vec<String> = Vec::new();
    let mut kept = String::new();
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("import ") {
            let name = rest.trim();
            let m = name.strip_prefix("std.").unwrap_or("");
            if m.is_empty() || !FUNCTIONS.iter().any(|f| f.0 == m) {
                return Err(vec![err(
                    i + 1,
                    format!(
                        "cannot import `{name}`: only the built-in modules `std.math`, `std.arrays` and `std.text` exist"
                    ),
                )]);
            }
            if !modules.iter().any(|x| x == m) {
                modules.push(m.to_string());
            }
            kept.push('\n');
        } else {
            kept.push_str(line);
            kept.push('\n');
        }
    }

    // Rewrite `module.func` -> `module_func` outside strings and `//` comments.
    let chars: Vec<char> = kept.chars().collect();
    let mut out = String::new();
    let mut used: Vec<(&str, &str, &str)> = Vec::new();
    let (mut i, mut line, mut in_str) = (0usize, 1usize, false);
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            in_str = false;
        }
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        let at_start = i == 0 || !(is_ident(chars[i - 1]) || chars[i - 1] == '.');
        if at_start && is_ident(c) {
            let mut j = i;
            while j < chars.len() && is_ident(chars[j]) {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();
            if modules.contains(&word) && chars.get(j) == Some(&'.') {
                let mut k = j + 1;
                while k < chars.len() && is_ident(chars[k]) {
                    k += 1;
                }
                let func: String = chars[j + 1..k].iter().collect();
                match FUNCTIONS.iter().find(|f| f.0 == word && f.1 == func) {
                    Some(f) => {
                        if !used.iter().any(|u| u.2 == f.2) {
                            used.push(*f);
                        }
                        out.push_str(&format!("{word}_{func}"));
                        i = k;
                        continue;
                    }
                    None => {
                        return Err(vec![err(
                            line,
                            format!("module `std.{word}` has no function `{func}`"),
                        )]);
                    }
                }
            }
            out.push_str(&word);
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    for (_, _, src) in used {
        out.push('\n');
        out.push_str(src);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn no_import_is_unchanged() {
        assert_eq!(
            expand("fn main() -> Int { return 1 }").unwrap(),
            "fn main() -> Int { return 1 }"
        );
    }

    #[test]
    fn strings_and_comments_are_not_rewritten() {
        let s = expand("import std.math\n// math.abs\nlet t = \"math.abs\"\nlet a = math.abs(1)\n")
            .unwrap();
        assert!(
            s.contains("// math.abs") && s.contains("\"math.abs\"") && s.contains("math_abs(1)")
        );
        assert!(s.contains("fn math_abs"));
        assert!(
            !s.contains("fn math_max"),
            "unused functions are not included"
        );
    }

    #[test]
    fn unknown_module_and_function_are_errors() {
        assert!(expand("import std.net\n").is_err());
        assert!(expand("import foo\n").is_err());
        assert!(expand("import std.math\nlet a = math.nope(1)\n").is_err());
    }
}
