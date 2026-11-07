//! File imports: `import "path/to/lib.yom" as alias`.
//!
//! Resolved by the command-line tool *before* compilation (the browser playground has no file
//! system, so it only supports `import std.<module>`). The imported file is inlined at the end
//! of the program and every function it declares is renamed `alias_name`; callers write
//! `alias.name(...)`. Imports nest (paths are relative to the importing file), cycles and
//! missing files are errors, and a library may not define `main`.
//!
//! Visibility: in a library only items marked `pub` (`pub fn`, `pub struct`, `pub enum`) can be
//! used from outside; everything else is private and using it is an error. Functions, structs
//! and enums are all namespaced (`alias.Point`, `alias.area(...)`), so two libraries can declare
//! the same name. An import is not re-exported: a library's own imports are private to it.
//!
//! Honest limits: this is source renaming, not a separate compilation unit. Renamed types
//! show up with their prefixed name (`geo_Point`) in output and error messages; an item whose
//! name is also used as a struct *field* name inside the same library would be renamed there
//! too; there is no `use`/glob import, no nested paths (`a.b.c`) and no `pub` on struct fields.

use std::path::{Path, PathBuf};
use yomnova_diagnostics::{Diagnostic, Span};

const MAX_DEPTH: usize = 16;
const MAX_FILE_BYTES: u64 = 1 << 20;

/// `geo` + `area` -> `geo_area`; `geo` + `Rect` -> `Geo_Rect` (type names must start upper-case).
fn mangle(alias: &str, name: &str) -> String {
    if name.starts_with(|c: char| c.is_uppercase()) {
        let mut a = alias.chars();
        let first: String = a
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        format!("{first}{}_{name}", a.as_str())
    } else {
        format!("{alias}_{name}")
    }
}

type Item = (String, bool);

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

/// `(name, is_pub)` for every `fn` / `struct` / `enum` declared in `src`.
fn declared_items(src: &str) -> Vec<(String, bool)> {
    let mut items = Vec::new();
    let (mut p2, mut p1) = (String::new(), String::new());
    let _ = rewrite(src, &mut |w, _| {
        if matches!(p1.as_str(), "fn" | "struct" | "enum") {
            items.push((w.to_string(), p2 == "pub"));
        }
        p2 = std::mem::take(&mut p1);
        p1 = w.to_string();
        Ok(None)
    });
    items
}

/// An imported library, for checking it on its own.
pub struct LibSource {
    pub path: PathBuf,
    /// The file as written.
    pub original: String,
    /// The file with its own imports inlined (what the importer sees), still with `pub`.
    pub own: String,
}

impl LibSource {
    /// Source that can be checked by itself: `pub` removed and a stub `main` appended
    /// (after the last line, so line numbers are those of the file).
    pub fn standalone(&self) -> String {
        let stripped = rewrite(&self.own, &mut |w, _| {
            Ok((w == "pub").then(|| (String::new(), 0)))
        })
        .unwrap_or_else(|_| self.own.clone());
        format!("{stripped}\nfn main() -> Int {{\n    return 0\n}}\n")
    }
}

/// Inline every `import "file" as alias` of `source`; paths are relative to `base`.
pub fn resolve(source: &str, base: &Path) -> Result<String, Vec<Diagnostic>> {
    resolve_with_libs(source, base).map(|r| r.0)
}

/// Like [`resolve`], also returning every imported library (nested ones too, once each) so that
/// each can be checked separately and its errors reported against its own file and lines.
pub fn resolve_with_libs(
    source: &str,
    base: &Path,
) -> Result<(String, Vec<LibSource>), Vec<Diagnostic>> {
    let mut stack = Vec::new();
    let mut libs = Vec::new();
    let out = resolve_in(source, base, &mut stack, &mut libs).map_err(err)?;
    Ok((out, libs))
}

fn resolve_in(
    source: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    libs_out: &mut Vec<LibSource>,
) -> Result<String, String> {
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
    let mut libs: Vec<(String, Vec<Item>, String)> = Vec::new(); // alias, items, source
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
        let original = text.clone();
        let text = resolve_in(&text, dir, stack, libs_out)?;
        stack.pop();
        if !libs_out.iter().any(|l| l.path == canon) {
            libs_out.push(LibSource {
                path: canon.clone(),
                original,
                own: text.clone(),
            });
        }
        let items = declared_items(&text);
        if items.iter().any(|f| f.0 == "main") {
            return Err(format!("'{path}' defines `main`; a library cannot"));
        }
        let renamed = rewrite(&text, &mut |w, _| {
            if w == "pub" {
                return Ok(Some((String::new(), 0)));
            }
            Ok(items
                .iter()
                .any(|i| i.0 == w)
                .then(|| (mangle(alias, w), 0)))
        })?;
        libs.push((alias.to_string(), items, renamed));
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
        match lib.1.iter().find(|i| i.0 == name) {
            Some((_, true)) => Ok(Some((mangle(w, &name), 1 + name.chars().count()))),
            Some((_, false)) => Err(format!(
                "`{w}.{name}` is private; mark it `pub` in the library"
            )),
            None => Err(format!("library `{w}` has no item `{name}`")),
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
        std::fs::write(
            d.join("a.yom"),
            "pub fn dbl(x: Int) -> Int { return x * 2 }\n",
        )
        .unwrap();
        std::fs::write(
            d.join("b.yom"),
            "import \"a.yom\" as a\npub fn quad(x: Int) -> Int { return a.dbl(a.dbl(x)) }\n",
        )
        .unwrap();
        let s = resolve(
            "import \"b.yom\" as b\nfn main() -> Int { return b.quad(3) }\n",
            &d,
        )
        .unwrap();
        assert!(s.contains("b_quad(3)") && s.contains("fn b_quad") && s.contains("fn b_a_dbl"));
        let t = resolve("import \"t.yom\" as t\nlet p = t.Pt { x: 1 }\n", &d);
        assert!(t.is_err(), "missing file");
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
        std::fs::write(
            d.join("ok.yom"),
            "pub fn f() -> Int { return 1 }\nfn hidden() -> Int { return 2 }\n",
        )
        .unwrap();
        for src in [
            "import \"nope.yom\" as n\n",
            "import \"m.yom\" as m\n",
            "import \"c1.yom\" as c\n",
            "import \"ok.yom\"\n",
            "import \"ok.yom\" as o\nlet x = o.zzz()\n",
            "import \"ok.yom\" as o\nlet x = o.hidden()\n",
        ] {
            assert!(resolve(src, &d).is_err(), "{src}");
        }
    }

    #[test]
    fn types_and_private_items_work_end_to_end() {
        let d = dir("e2e");
        std::fs::write(
            d.join("lib.yom"),
            "pub struct Pt {\n    x: Int,\n    y: Int,\n}\nfn secret() -> Int {\n    return 7\n}\npub fn sum(p: Pt) -> Int {\n    return p.x + p.y + secret()\n}\n",
        )
        .unwrap();
        // the importer may define its own `secret` and `Pt`: names do not collide
        let src = "import \"lib.yom\" as l\nstruct Pt {\n    z: Int,\n}\nfn secret() -> Int {\n    return 1\n}\nfn main() -> Int {\n    let p = l.Pt { x: 1, y: 2 }\n    let q = Pt { z: 5 }\n    emit(l.sum(p))\n    emit(secret() + q.z)\n    return 0\n}\n";
        let out = crate::Compiler::compile_and_run(&resolve(src, &d).unwrap()).expect("runs");
        assert_eq!(out.output_lines, vec!["10", "6"]);
    }

    #[test]
    fn libraries_are_checked_on_their_own_with_their_own_line_numbers() {
        let d = dir("libcheck");
        std::fs::write(
            d.join("bad.yom"),
            "pub fn ok() -> Int {\n    return 1\n}\n\npub fn broken() -> Int {\n    let x: Int = \"text\"\n    return x\n}\n",
        )
        .unwrap();
        std::fs::write(d.join("good.yom"), "pub fn f() -> Int {\n    return 1\n}\n").unwrap();
        let (_, libs) =
            resolve_with_libs("import \"good.yom\" as g\nimport \"bad.yom\" as b\n", &d).unwrap();
        assert_eq!(libs.len(), 2);
        let report = |name: &str| {
            let lib = libs.iter().find(|l| l.path.ends_with(name)).unwrap();
            crate::Compiler::check(&lib.standalone())
        };
        assert!(report("good.yom").is_ok());
        let diags = report("bad.yom").unwrap_err();
        assert_eq!(
            diags[0].span.as_ref().map(|s| s.line),
            Some(6),
            "line inside bad.yom"
        );
    }
}
