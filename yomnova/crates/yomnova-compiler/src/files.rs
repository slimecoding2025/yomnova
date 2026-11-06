//! File imports: `import "path/to/lib.yom" as alias`.
//!
//! Resolved by the command-line tool *before* compilation (the browser playground has no file
//! system, so it only supports `import std.<module>`). The imported file is inlined at the end
//! of the program and every function it declares is renamed `alias_name`; callers write
//! `alias.name(...)`. Imports nest (paths are relative to the importing file), cycles and
//! missing files are errors, and a library may not define `main`.
//!
//! Honest limits: this is renaming, not a real module system. Only **functions** are
//! namespaced; structs and enums stay global, so two libraries declaring the same struct
//! collide. There is no visibility (`pub`), no re-export, and a function name that is also a
//! struct field name inside the same library would be renamed with it.

use std::path::{Path, PathBuf};
use yomnova_diagnostics::{Diagnostic, Span};

const MAX_DEPTH: usize = 16;
const MAX_FILE_BYTES: u64 = 1 << 20;

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn err(msg: String) -> Vec<Diagnostic> {
    vec![Diagnostic::error(msg, None::<Span>)]
}

type Rewrite<'a> = &'a mut dyn FnMut(&str, &[char]) -> Result<Option<(String, usize)>, String>;

/// Calls `f(word, rest_after_word)` for each identifier outside strings and `//` comments that
/// is not preceded by `.`; `f` may return a replacement and how many chars after the word to skip.
fn rewrite(src: &str, f: Rewrite) -> Result<String, String> {
    let ch: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let (mut i, mut in_str) = (0usize, false);
    while i < ch.len() {
        let c = ch[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < ch.len() {
                out.push(ch[i + 1]);
                i += 1;
            } else if c == '"' || c == '\n' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
        } else if c == '/' && ch.get(i + 1) == Some(&'/') {
            while i < ch.len() && ch[i] != '\n' {
                out.push(ch[i]);
                i += 1;
            }
            continue;
        } else if is_ident(c) && (i == 0 || !(is_ident(ch[i - 1]) || ch[i - 1] == '.')) {
            let mut j = i;
            while j < ch.len() && is_ident(ch[j]) {
                j += 1;
            }
            let word: String = ch[i..j].iter().collect();
            match f(&word, &ch[j..])? {
                Some((rep, skip)) => {
                    out.push_str(&rep);
                    i = j + skip;
                }
                None => {
                    out.push_str(&word);
                    i = j;
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    Ok(out)
}

/// Names of the functions declared by `fn name` in `src`.
fn declared_fns(src: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut prev_fn = false;
    let _ = rewrite(src, &mut |w, _| {
        if prev_fn {
            names.push(w.to_string());
        }
        prev_fn = w == "fn";
        Ok(None)
    });
    names
}

/// Inline every `import "file" as alias` of `source`; paths are relative to `base`.
pub fn resolve(source: &str, base: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut stack = Vec::new();
    resolve_in(source, base, &mut stack).map_err(err)
}

fn resolve_in(source: &str, base: &Path, stack: &mut Vec<PathBuf>) -> Result<String, String> {
    if !source
        .lines()
        .any(|l| l.trim_start().starts_with("import \""))
    {
        return Ok(source.to_string());
    }
    if stack.len() >= MAX_DEPTH {
        return Err(format!(
            "imports are nested more than {MAX_DEPTH} levels deep"
        ));
    }
    let mut body = String::new();
    let mut libs: Vec<(String, Vec<String>, String)> = Vec::new(); // alias, fns, source
    for (n, line) in source.lines().enumerate() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("import \"") else {
            body.push_str(line);
            body.push('\n');
            continue;
        };
        let bad = || format!("line {}: expected `import \"file.yom\" as name`", n + 1);
        let (path, after) = rest.split_once('"').ok_or_else(bad)?;
        let alias = after.trim().strip_prefix("as ").ok_or_else(bad)?.trim();
        if alias.is_empty()
            || !alias.chars().all(is_ident)
            || alias.starts_with(|c: char| c.is_ascii_digit())
        {
            return Err(bad());
        }
        if libs.iter().any(|l| l.0 == alias) {
            return Err(format!(
                "line {}: the name `{alias}` is imported twice",
                n + 1
            ));
        }
        let file = base.join(path);
        let canon = file
            .canonicalize()
            .map_err(|e| format!("line {}: cannot import '{path}': {e}", n + 1))?;
        if stack.contains(&canon) {
            return Err(format!("line {}: circular import of '{path}'", n + 1));
        }
        let meta = std::fs::metadata(&canon).map_err(|e| format!("cannot read '{path}': {e}"))?;
        if meta.len() > MAX_FILE_BYTES {
            return Err(format!("'{path}' is larger than {MAX_FILE_BYTES} bytes"));
        }
        let text = std::fs::read_to_string(&canon)
            .map_err(|e| format!("line {}: cannot import '{path}': {e}", n + 1))?;
        stack.push(canon.clone());
        let dir = canon.parent().unwrap_or(Path::new("."));
        let text = resolve_in(&text, dir, stack)?;
        stack.pop();
        let fns = declared_fns(&text);
        if fns.iter().any(|f| f == "main") {
            return Err(format!("'{path}' defines `main`; a library cannot"));
        }
        let renamed = rewrite(&text, &mut |w, _| {
            Ok(fns
                .contains(&w.to_string())
                .then(|| (format!("{alias}_{w}"), 0)))
        })?;
        libs.push((alias.to_string(), fns, renamed));
        body.push('\n'); // keep line numbers of the importing file
    }
    let mut out = rewrite(&body, &mut |w, rest| {
        let Some(lib) = libs.iter().find(|l| l.0 == w) else {
            return Ok(None);
        };
        if rest.first() != Some(&'.') {
            return Ok(None);
        }
        let name: String = rest[1..].iter().take_while(|c| is_ident(**c)).collect();
        if lib.1.contains(&name) {
            Ok(Some((format!("{w}_{name}"), 1 + name.chars().count())))
        } else {
            Err(format!("library `{w}` has no function `{name}`"))
        }
    })?;
    for (_, _, text) in libs {
        out.push('\n');
        out.push_str(&text);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("yomnova-files-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn imports_rename_and_nest() {
        let d = dir("nest");
        std::fs::write(d.join("a.yom"), "fn dbl(x: Int) -> Int { return x * 2 }\n").unwrap();
        std::fs::write(
            d.join("b.yom"),
            "import \"a.yom\" as a\nfn quad(x: Int) -> Int { return a.dbl(a.dbl(x)) }\n",
        )
        .unwrap();
        let s = resolve(
            "import \"b.yom\" as b\nfn main() -> Int { return b.quad(3) }\n",
            &d,
        )
        .unwrap();
        assert!(s.contains("b_quad(3)") && s.contains("fn b_quad") && s.contains("fn b_a_dbl"));
        assert!(
            s.starts_with('\n'),
            "import line is blanked to keep line numbers"
        );
    }

    #[test]
    fn bad_imports_are_errors() {
        let d = dir("bad");
        std::fs::write(d.join("m.yom"), "fn main() -> Int { return 0 }\n").unwrap();
        std::fs::write(d.join("c1.yom"), "import \"c2.yom\" as x\n").unwrap();
        std::fs::write(d.join("c2.yom"), "import \"c1.yom\" as y\n").unwrap();
        std::fs::write(d.join("ok.yom"), "fn f() -> Int { return 1 }\n").unwrap();
        for src in [
            "import \"nope.yom\" as n\n",
            "import \"m.yom\" as m\n",
            "import \"c1.yom\" as c\n",
            "import \"ok.yom\"\n",
            "import \"ok.yom\" as o\nlet x = o.zzz()\n",
        ] {
            assert!(resolve(src, &d).is_err(), "{src}");
        }
    }
}
