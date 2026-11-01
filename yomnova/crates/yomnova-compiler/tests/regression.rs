//! End-to-end regression tests: each case compiles and runs a real program through the
//! full pipeline (lexer → parser → sema → ownership → bytecode → VM).

use yomnova_compiler::Compiler;

fn run(body: &str) -> Result<Vec<String>, String> {
    let src = format!("module t\n{body}");
    Compiler::compile_and_run(&src)
        .map(|o| o.output_lines)
        .map_err(|ds| {
            ds.into_iter()
                .map(|d| d.message)
                .collect::<Vec<_>>()
                .join("\n")
        })
}

fn ok(body: &str, expected: &[&str]) {
    match run(body) {
        Ok(lines) => assert_eq!(lines, expected, "program:\n{body}"),
        Err(e) => panic!("expected success, got error: {e}\nprogram:\n{body}"),
    }
}

fn err(body: &str, needle: &str) {
    match run(body) {
        Ok(lines) => panic!("expected error containing {needle:?}, program ran: {lines:?}\n{body}"),
        Err(e) => assert!(
            e.contains(needle),
            "error {e:?} lacks {needle:?}\nprogram:\n{body}"
        ),
    }
}

#[test]
fn condition_does_not_leak_onto_stack() {
    // Before the fix `JumpIfFalse` left the condition on the stack, so loops grew it.
    ok(
        "fn main() -> Int {\n mut i: Int = 0\n while i < 100000 { i = i + 1 }\n emit(i)\n return 0\n}",
        &["100000"],
    );
}

#[test]
fn block_scoping_and_shadowing() {
    ok(
        "fn main() -> Int {\n let a: Int = 1\n if true {\n let a: Int = 2\n emit(a)\n }\n emit(a)\n return 0\n}",
        &["2", "1"],
    );
}

#[test]
fn short_circuit_does_not_evaluate_right_side() {
    let f = "fn boom() -> Bool {\n emit(\"boom\")\n return true\n}\n";
    ok(
        &format!("{f}fn main() -> Int {{\n if false && boom() {{ emit(\"no\") }}\n return 0\n}}"),
        &[],
    );
    ok(
        &format!("{f}fn main() -> Int {{\n if true || boom() {{ emit(\"yes\") }}\n return 0\n}}"),
        &["yes"],
    );
    ok(
        &format!("{f}fn main() -> Int {{\n if true && boom() {{ emit(\"both\") }}\n return 0\n}}"),
        &["boom", "both"],
    );
}

#[test]
fn pipelines_apply_every_stage() {
    let fns = "fn inc(n: Int) -> Int {\n return n + 1\n}\nfn dbl(n: Int) -> Int {\n return n * 2\n}\nfn add(a: Int, b: Int) -> Int {\n return a + b\n}\n";
    ok(
        &format!(
            "{fns}fn main() -> Int {{\n let r: Int = 5 |> inc |> dbl\n emit(r)\n return 0\n}}"
        ),
        &["12"],
    );
    ok(
        &format!(
            "{fns}fn main() -> Int {{\n flow r = 5 |> inc |> add(10)\n emit(r)\n return 0\n}}"
        ),
        &["16"],
    );
    ok(
        &format!("{fns}fn main() -> Int {{\n 5 |> inc |> emit\n return 0\n}}"),
        &["6"],
    );
    err(
        &format!("{fns}fn main() -> Int {{\n let r: Int = 5 |> nothere\n return r\n}}"),
        "nothere",
    );
    err(
        &format!("{fns}fn main() -> Int {{\n let r: Int = \"x\" |> inc\n return r\n}}"),
        "Type mismatch",
    );
}

#[test]
fn constants_work_globally_and_locally() {
    ok("const K: Int = 5\nfn f() -> Int {\n return K * 2\n}\nfn main() -> Int {\n emit(f())\n return 0\n}", &["10"]);
    ok(
        "fn main() -> Int {\n const K: Int = 7\n emit(K)\n return 0\n}",
        &["7"],
    );
    err(
        "const K: Int = 5\nfn main() -> Int {\n K = 6\n return 0\n}",
        "immutable",
    );
}

#[test]
fn integer_overflow_is_an_error_not_a_panic() {
    err(
        "fn main() -> Int {\n let a: Int = 9223372036854775807\n emit(a + 1)\n return 0\n}",
        "overflow",
    );
    err(
        "fn main() -> Int {\n let a: Int = 9223372036854775807\n emit(a * 2)\n return 0\n}",
        "overflow",
    );
    err(
        "fn main() -> Int {\n let a: Int = -9223372036854775807 - 1\n emit(-a)\n return 0\n}",
        "overflow",
    );
    err(
        "fn main() -> Int {\n let a: Int = -9223372036854775807 - 1\n emit(a / -1)\n return 0\n}",
        "overflow",
    );
    err(
        "fn main() -> Int {\n emit(1 / 0)\n return 0\n}",
        "Division by zero",
    );
}

#[test]
fn negative_index_is_out_of_bounds() {
    err(
        "fn main() -> Int {\n let xs: Array<Int> = [1, 2, 3]\n emit(xs[-1])\n return 0\n}",
        "out of bounds",
    );
    ok(
        "fn main() -> Int {\n let xs: Array<Int> = [1, 2, 3]\n emit(xs[2])\n return 0\n}",
        &["3"],
    );
}

#[test]
fn endpoint_port_is_range_checked() {
    err(
        "fn main() -> Int {\n emit(Endpoint(IPv4(\"1.2.3.4\"), 70000))\n return 0\n}",
        "out of range",
    );
    ok(
        "fn main() -> Int {\n emit(Endpoint(IPv4(\"1.2.3.4\"), 8080))\n return 0\n}",
        &["1.2.3.4:8080"],
    );
}

#[test]
fn recursion_depth_is_bounded() {
    err(
        "fn f(n: Int) -> Int {\n return f(n + 1)\n}\nfn main() -> Int {\n return f(0)\n}",
        "call depth",
    );
}

#[test]
fn deep_nesting_is_rejected_cleanly() {
    let deep = format!(
        "fn main() -> Int {{\n emit({}1{})\n return 0\n}}",
        "(".repeat(5000),
        ")".repeat(5000)
    );
    err(&deep, "Nesting too deep");
}

#[test]
fn static_checks() {
    err(
        "fn f() -> Int {\n emit(1)\n}\nfn main() -> Int {\n return f()\n}",
        "without returning",
    );
    err(
        "fn f(n: Int) -> Int {\n return n\n}\nfn main() -> Int {\n return f(\"x\")\n}",
        "Type mismatch",
    );
    err(
        "fn main(x: Int) -> Int {\n return x\n}",
        "must not take parameters",
    );
    err(
        "struct P { x: Int }\nfn main() -> Int {\n return 0\n}",
        "not supported",
    );
    err(
        "fn main() -> Int {\n emit(1, 2)\n return 0\n}",
        "requires 1 arguments",
    );
    err(
        "fn main() -> Int {\n let a: Int = 1\n return a[0]\n}",
        "Array",
    );
}

#[test]
fn ownership_rules() {
    let eat = "fn eat(t: Text) -> Unit {\n emit(t)\n}\n";
    // inferred copy types may be reused
    ok("fn show(n: Int) -> Unit {\n emit(n)\n}\nfn main() -> Int {\n let k = 9\n show(k)\n show(k)\n return 0\n}", &["9", "9"]);
    // inferred non-copy types move
    err(
        &format!("{eat}fn main() -> Int {{\n let k = \"x\"\n eat(k)\n eat(k)\n return 0\n}}"),
        "moved",
    );
    // aliasing moves
    err(
        "fn main() -> Int {\n let a = \"x\"\n let b = a\n emit(a)\n return 0\n}",
        "moved",
    );
    // emit only borrows
    ok(
        "fn main() -> Int {\n let a = \"x\"\n emit(a)\n emit(a)\n return 0\n}",
        &["x", "x"],
    );
    // moving inside a loop
    err(&format!("{eat}fn main() -> Int {{\n let s = \"x\"\n mut i: Int = 0\n while i < 2 {{\n eat(s)\n i = i + 1\n }}\n return 0\n}}"), "loop");
    // each branch may move independently
    ok(&format!("{eat}fn main() -> Int {{\n let s = \"x\"\n if true {{ eat(s) }} else {{ eat(s) }}\n return 0\n}}"), &["x"]);
    // maybe-moved after an if
    err(&format!("{eat}fn main() -> Int {{\n let s = \"x\"\n if true {{ eat(s) }}\n eat(s)\n return 0\n}}"), "moved");
}

#[test]
fn unsupported_syntax_is_reported_not_miscompiled() {
    err(
        "fn main() -> Int {\n let y = 1\n let x = y.z\n return 0\n}",
        "not supported",
    );
}
