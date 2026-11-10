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
        "fn main() -> Int {\n let v = [1]\n let n = v.first()\n return 0\n}",
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
        "fn main() -> Int {\n let y = 1\n let x = y.z()\n return 0\n}",
        "not supported",
    );
}

#[test]
fn float_modulo_is_a_static_error() {
    err("fn main() -> Int {\n emit(5.0 % 2.0)\n return 0\n}", "Mod");
    ok(
        "fn main() -> Int {\n emit(5.5 + 2.0)\n return 0\n}",
        &["7.5"],
    );
}

#[test]
fn bare_return_and_else_if_chain() {
    ok(
        "fn pick(n: Int) -> Unit {\n if n < 0 {\n emit(\"neg\")\n return\n } else if n == 0 {\n emit(\"zero\")\n } else {\n emit(\"pos\")\n }\n}\nfn main() -> Int {\n pick(-1)\n pick(0)\n pick(1)\n return 0\n}",
        &["neg", "zero", "pos"],
    );
}

// ---- multiple diagnostics per run (error recovery) ----

fn diags(body: &str) -> Vec<(usize, String)> {
    let src = format!("module t\n{body}");
    match Compiler::check(&src) {
        Ok(_) => Vec::new(),
        Err(ds) => ds
            .into_iter()
            .map(|d| (d.span.map_or(0, |s| s.line), d.message))
            .collect(),
    }
}

#[test]
fn reports_several_syntax_errors_in_one_run() {
    let d = diags("fn main() -> Int {\n let = 5\n let a: Int = 1\n emit(a +)\n return 0\n}");
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!(d[0].0, 3);
    assert_eq!(d[1].0, 5);
}

#[test]
fn reports_several_type_errors_in_one_run() {
    let d = diags("fn main() -> Int {\n let a: Int = \"x\"\n emit(1 + true)\n return 0\n}");
    assert_eq!(d.len(), 2, "{d:?}");
}

#[test]
fn failed_declaration_does_not_cascade() {
    // `a` fails to type-check; the later uses of `a` must not add "undefined" errors.
    let d = diags(
        "fn main() -> Int {\n let a: Int = \"x\"\n emit(a)\n let b = a\n emit(b)\n return 0\n}",
    );
    assert_eq!(d.len(), 1, "{d:?}");
}

#[test]
fn errors_in_separate_functions_are_all_reported() {
    let d = diags(
        "fn f() -> Int {\n return \"x\"\n}\nfn g() -> Int {\n return true\n}\nfn main() -> Int {\n return 0\n}",
    );
    assert_eq!(d.len(), 2, "{d:?}");
}

#[test]
fn reports_several_ownership_errors_in_one_run() {
    let d = diags(
        "fn main() -> Int {\n let a = \"t\"\n let b = a\n emit(a)\n let c = \"u\"\n let d = c\n emit(c)\n return 0\n}",
    );
    assert_eq!(d.len(), 2, "{d:?}");
}

#[test]
fn later_stage_waits_for_earlier_stage() {
    // A type error and an ownership error: only the type error is reported until it is fixed.
    let d = diags(
        "fn main() -> Int {\n let n: Int = \"x\"\n let a = \"t\"\n let b = a\n emit(a)\n return 0\n}",
    );
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].1.contains("mismatch"), "{d:?}");
}

#[test]
fn recovery_always_terminates_on_garbage() {
    for src in [
        "}}}}",
        "fn fn fn",
        "let let let",
        "((((",
        "fn main( {",
        "if if if }",
    ] {
        let _ = diags(src); // must return, not hang or panic
    }
}

#[test]
fn single_error_programs_still_report_exactly_one() {
    assert_eq!(
        diags("fn main() -> Int {\n emit(zzz)\n return 0\n}").len(),
        1
    );
}

// ---- for … in, len, push ----

#[test]
fn for_loop_iterates_an_array() {
    ok(
        "fn main() -> Int {\n mut total: Int = 0\n for n in [1, 2, 3, 4] { total = total + n }\n emit(total)\n return 0\n}",
        &["10"],
    );
}

#[test]
fn for_loop_over_empty_array_runs_zero_times() {
    ok(
        "fn main() -> Int {\n let e = []\n for z in e { emit(z) }\n emit(\"done\")\n return 0\n}",
        &["done"],
    );
}

#[test]
fn len_of_array_and_text() {
    ok(
        "fn main() -> Int {\n let a = [5, 6, 7]\n emit(len(a))\n emit(len(\"héllo\"))\n emit(len(a))\n return 0\n}",
        &["3", "5", "3"],
    );
}

#[test]
fn push_returns_an_extended_array() {
    ok(
        "fn main() -> Int {\n mut xs = [10]\n xs = push(xs, 20)\n xs = push(xs, 30)\n for x in xs { emit(x) }\n emit(len(xs))\n return 0\n}",
        &["10", "20", "30", "3"],
    );
}

#[test]
fn push_on_empty_literal_infers_the_element_type() {
    ok(
        "fn main() -> Int {\n let a = push([], \"a\")\n for s in a { emit(s) }\n return 0\n}",
        &["a"],
    );
}

#[test]
fn loop_variable_is_scoped_to_the_loop() {
    err(
        "fn main() -> Int {\n for x in [1] { emit(x) }\n emit(x)\n return 0\n}",
        "Undefined identifier 'x'",
    );
}

#[test]
fn for_loop_type_errors() {
    err(
        "fn main() -> Int {\n for x in 5 { emit(x) }\n return 0\n}",
        "expected Array",
    );
    err(
        "fn main() -> Int {\n for s in [1, 2] { emit(s + \"x\") }\n return 0\n}",
        "Cannot apply binary operator",
    );
}

#[test]
fn len_and_push_type_errors() {
    err(
        "fn main() -> Int {\n emit(len(true))\n return 0\n}",
        "expected Array<_>, Text or NetworkBuffer",
    );
    err(
        "fn main() -> Int {\n let a = push(7, 1)\n return 0\n}",
        "expected Array<_>",
    );
    err(
        "fn main() -> Int {\n let a = push([1], \"t\")\n return 0\n}",
        "expected Int, found Text",
    );
}

#[test]
fn for_loop_borrows_but_push_moves() {
    // Iterating borrows the array, so it can be used afterwards...
    ok(
        "fn main() -> Int {\n let n = [\"a\", \"b\"]\n for x in n { emit(x) }\n emit(len(n))\n return 0\n}",
        &["a", "b", "2"],
    );
    // ...while push consumes it.
    err(
        "fn main() -> Int {\n let n = [\"a\"]\n let m = push(n, \"b\")\n emit(len(n))\n return 0\n}",
        "moved value",
    );
}

#[test]
fn moving_an_outer_value_inside_a_for_loop_is_rejected() {
    err(
        "fn take(s: Text) -> Int {\n return 0\n}\nfn main() -> Int {\n let t = \"x\"\n for i in [1, 2] { take(t) }\n return 0\n}",
        "moved inside a loop",
    );
}

#[test]
fn nested_for_loops() {
    ok(
        "fn main() -> Int {\n for a in [1, 2] { for b in [10, 20] { emit(a * b) } }\n return 0\n}",
        &["10", "20", "20", "40"],
    );
}

// ---- structs ----

const POINT: &str = "struct Point { x: Int, y: Int }\n";

#[test]
fn struct_literal_field_read_and_display() {
    ok(
        &format!("{POINT}fn main() -> Int {{\n let p = Point {{ y: 4, x: 3 }}\n emit(p)\n emit(p.x)\n return 0\n}}"),
        &["Point { x: 3, y: 4 }", "3"],
    );
}

#[test]
fn struct_as_parameter_and_nested_struct() {
    ok(
        &format!(
            "{POINT}struct Line {{ from: Point, to: Point }}\nfn d(p: Point) -> Int {{\n return p.x * p.x + p.y * p.y\n}}\nfn main() -> Int {{\n let l = Line {{ from: Point {{ x: 0, y: 0 }}, to: Point {{ x: 3, y: 4 }} }}\n emit(d(l.to))\n emit(l.to.y)\n return 0\n}}"
        ),
        &["25", "4"],
    );
}

#[test]
fn structs_compare_by_value_regardless_of_field_order_in_the_literal() {
    ok(
        &format!("{POINT}fn main() -> Int {{\n let a = Point {{ x: 1, y: 2 }}\n let b = Point {{ y: 2, x: 1 }}\n emit(a == b)\n return 0\n}}"),
        &["true"],
    );
}

#[test]
fn array_of_structs_in_a_for_loop() {
    ok(
        &format!("{POINT}fn main() -> Int {{\n mut s: Int = 0\n for q in [Point {{ x: 1, y: 0 }}, Point {{ x: 2, y: 0 }}] {{ s = s + q.x }}\n emit(s)\n return 0\n}}"),
        &["3"],
    );
}

#[test]
fn struct_literal_errors() {
    let body = |lit: &str| format!("{POINT}fn main() -> Int {{\n let p = {lit}\n return 0\n}}");
    err(&body("Point { x: 1 }"), "missing field 'y'");
    err(
        &body("Point { x: 1, y: \"s\" }"),
        "expected Int, found Text",
    );
    err(&body("Point { x: 1, y: 2, z: 3 }"), "has no field 'z'");
    err(&body("Point { x: 1, x: 2, y: 3 }"), "more than once");
    err(&body("Nope { q: 1 }"), "unknown struct 'Nope'");
}

#[test]
fn field_access_errors() {
    err(
        &format!("{POINT}fn main() -> Int {{\n let p = Point {{ x: 1, y: 2 }}\n emit(p.z)\n return 0\n}}"),
        "has no field 'z'",
    );
    err(
        "fn main() -> Int {\n emit(5.x)\n return 0\n}",
        "cannot read field",
    );
}

#[test]
fn struct_declaration_errors() {
    err(
        "struct A { a: A }\nfn main() -> Int { return 0 }",
        "infinite size",
    );
    err(
        "struct A { a: Int, a: Int }\nfn main() -> Int { return 0 }",
        "already declared",
    );
    err(
        "struct A { a: Int }\nstruct A { b: Int }\nfn main() -> Int { return 0 }",
        "already declared",
    );
    err(
        "struct A { a: Missing }\nfn main() -> Int { return 0 }",
        "unknown type 'Missing'",
    );
    err(
        "fn main() -> Int {\n let x: Missing = 1\n return 0\n}",
        "unknown type 'Missing'",
    );
}

#[test]
fn structs_are_moved_but_field_reads_do_not_move() {
    ok(
        &format!("{POINT}fn main() -> Int {{\n let p = Point {{ x: 1, y: 2 }}\n emit(p.x)\n emit(p.y)\n emit(p)\n return 0\n}}"),
        &["1", "2", "Point { x: 1, y: 2 }"],
    );
    err(
        &format!("{POINT}fn take(p: Point) -> Int {{\n return p.x\n}}\nfn main() -> Int {{\n let p = Point {{ x: 1, y: 2 }}\n take(p)\n emit(p.x)\n return 0\n}}"),
        "moved value",
    );
}

#[test]
fn braces_after_an_uppercase_name_still_open_a_block() {
    // `Limit { emit(...) }` is not a struct literal: it is not followed by `ident :`.
    ok(
        "const Limit = 3\nfn main() -> Int {\n if 3 == Limit { emit(\"eq\") }\n for a in [1] { emit(a) }\n return 0\n}",
        &["eq", "1"],
    );
}

// ---- enums and match ----

const SHAPE: &str = "enum Shape { Circle(Int), Rect(Int), Empty }\n";

fn with_shape(body: &str) -> String {
    format!("{SHAPE}{body}")
}

#[test]
fn enum_values_display_and_compare_by_value() {
    ok(
        &with_shape("fn main() -> Int {\n emit(Shape.Circle(2))\n emit(Shape.Empty)\n emit(Shape.Circle(2) == Shape.Circle(2))\n emit(Shape.Circle(2) == Shape.Rect(2))\n return 0\n}"),
        &["Shape.Circle(2)", "Shape.Empty", "true", "false"],
    );
}

#[test]
fn match_binds_the_carried_value_and_returns_from_every_arm() {
    // `area` has no statement after the match: sema must see that every arm returns.
    ok(
        &with_shape("fn area(s: Shape) -> Int {\n match s {\n Shape.Circle(r) => { return r * r * 3 }\n Shape.Rect(w) => { return w * w }\n Shape.Empty => { return 0 }\n }\n}\nfn main() -> Int {\n emit(area(Shape.Circle(2)))\n emit(area(Shape.Rect(4)))\n emit(area(Shape.Empty))\n return 0\n}"),
        &["12", "16", "0"],
    );
}

#[test]
fn match_wildcard_ignore_binding_and_only_the_first_matching_arm_runs() {
    ok(
        &with_shape("fn main() -> Int {\n for s in [Shape.Circle(1), Shape.Empty, Shape.Rect(2)] {\n match s {\n Shape.Circle(_) => { emit(\"c\") }\n _ => { emit(\"other\") }\n }\n }\n return 0\n}"),
        &["c", "other", "other"],
    );
}

#[test]
fn enums_work_inside_structs_and_with_text_payloads() {
    ok(
        "enum Msg { Text(Text), Quit }\nstruct Box { m: Msg }\nfn main() -> Int {\n let b = Box { m: Msg.Text(\"hi\") }\n match b.m {\n Msg.Text(t) => { emit(t) }\n Msg.Quit => { emit(\"quit\") }\n }\n return 0\n}",
        &["hi"],
    );
}

#[test]
fn match_must_be_exhaustive() {
    err(
        &with_shape("fn main() -> Int {\n match Shape.Empty {\n Shape.Circle(r) => { emit(r) }\n }\n return 0\n}"),
        "not exhaustive: missing Shape.Rect, Shape.Empty",
    );
}

#[test]
fn match_arm_errors() {
    let m = |arms: &str| {
        with_shape(&format!(
            "fn main() -> Int {{\n let s = Shape.Empty\n match s {{\n{arms}\n }}\n return 0\n}}"
        ))
    };
    err(
        &m("Shape.Circle(a) => { emit(a) }\nShape.Circle(b) => { emit(b) }\n_ => { emit(0) }"),
        "matched more than once",
    );
    err(
        &m("_ => { emit(0) }\nShape.Empty => { emit(1) }"),
        "can never run",
    );
    err(
        &m("Shape.Circle => { emit(0) }\n_ => { emit(1) }"),
        "carries a value",
    );
    err(
        &m("Shape.Empty(x) => { emit(0) }\n_ => { emit(1) }"),
        "carries no value",
    );
    err(
        &m("Shape.Nope => { emit(0) }\n_ => { emit(1) }"),
        "has no variant 'Nope'",
    );
    err(
        &format!(
            "enum Other {{ A }}\n{}",
            m("Other.A => { emit(0) }\n_ => { emit(1) }")
        ),
        "pattern is for enum 'Other'",
    );
    // a non-enum scrutinee is fine with literal patterns or `_`, but an enum pattern cannot match it
    err(
        &with_shape("fn main() -> Int {\n match 5 { Shape.Empty => { emit(1) }\n _ => { emit(2) } }\n return 0\n}"),
        "expected a value matching Shape.Empty",
    );
}

#[test]
fn enum_construction_errors() {
    let b = |e: &str| with_shape(&format!("fn main() -> Int {{\n let v = {e}\n return 0\n}}"));
    err(&b("Shape.Nope"), "has no variant 'Nope'");
    err(&b("Shape.Circle"), "carries a value");
    err(&b("Shape.Empty(1)"), "carries no value");
    err(&b("Shape.Circle(\"x\")"), "expected Int, found Text");
}

#[test]
fn enum_declaration_errors() {
    err(
        "enum E { A, A }\nfn main() -> Int { return 0 }",
        "already declared",
    );
    err("enum E { }\nfn main() -> Int { return 0 }", "no variants");
    err(
        "enum E { A(E) }\nfn main() -> Int { return 0 }",
        "infinite size",
    );
    err(
        "enum E { A(Missing) }\nfn main() -> Int { return 0 }",
        "unknown type 'Missing'",
    );
    err(
        "enum E { A }\nenum E { B }\nfn main() -> Int { return 0 }",
        "already declared",
    );
    err(
        "enum E { A }\nstruct E { x: Int }\nfn main() -> Int { return 0 }",
        "already declared",
    );
}

#[test]
fn matching_borrows_but_constructing_and_passing_move() {
    // The match itself does not consume the value...
    ok(
        &with_shape("fn main() -> Int {\n let s = Shape.Circle(1)\n match s {\n Shape.Circle(r) => { emit(r) }\n _ => { emit(0) }\n }\n emit(s)\n return 0\n}"),
        &["1", "Shape.Circle(1)"],
    );
    // ...passing it to a function does, and so does putting a moved Text inside a variant.
    err(
        &with_shape("fn take(s: Shape) -> Int {\n return 0\n}\nfn main() -> Int {\n let s = Shape.Empty\n take(s)\n emit(s)\n return 0\n}"),
        "moved value",
    );
    err(
        "enum Msg { Text(Text) }\nfn main() -> Int {\n let t = \"x\"\n let m = Msg.Text(t)\n emit(t)\n return 0\n}",
        "moved value",
    );
}

#[test]
fn a_move_inside_a_match_arm_makes_the_value_maybe_moved_afterwards() {
    err(
        &with_shape("fn take(t: Text) -> Int {\n return 0\n}\nfn main() -> Int {\n let t = \"x\"\n match Shape.Empty {\n Shape.Empty => { take(t) }\n _ => { emit(1) }\n }\n emit(t)\n return 0\n}"),
        "moved value",
    );
}

#[test]
fn match_binding_is_scoped_to_its_arm() {
    err(
        &with_shape("fn main() -> Int {\n match Shape.Circle(1) {\n Shape.Circle(r) => { emit(r) }\n _ => { emit(0) }\n }\n emit(r)\n return 0\n}"),
        "Undefined identifier 'r'",
    );
}

// ---- warnings ----

fn warnings(body: &str) -> Vec<(usize, String)> {
    let src = format!("module t\n{body}");
    match Compiler::check_with_warnings(&src) {
        Ok((_, ws)) => ws
            .into_iter()
            .map(|d| (d.span.map_or(0, |s| s.line), d.message))
            .collect(),
        Err(ds) => panic!("expected a valid program, got errors: {ds:?}\n{body}"),
    }
}

#[test]
fn unused_variable_is_a_warning_not_an_error() {
    let src = "fn main() -> Int {\n let unused = 1\n let used = 2\n emit(used)\n return 0\n}";
    let w = warnings(src);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, 3);
    assert!(w[0].1.contains("'unused' is never read"), "{w:?}");
    // The program still runs.
    ok(src, &["2"]);
}

#[test]
fn underscore_names_parameters_and_used_values_are_not_reported() {
    assert!(warnings("fn f(p: Int) -> Int {\n let _ignored = 1\n return 0\n}\nfn main() -> Int {\n emit(f(1))\n return 0\n}").is_empty());
}

#[test]
fn assignment_alone_does_not_count_as_a_read() {
    let w = warnings("fn main() -> Int {\n mut x = 1\n x = 2\n return 0\n}");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].1.contains("'x'"), "{w:?}");
}

#[test]
fn reads_inside_nested_blocks_loops_and_pipelines_count() {
    let w = warnings("fn inc(n: Int) -> Int {\n return n + 1\n}\nfn main() -> Int {\n let a = 1\n let b = [1, 2]\n let c = 3\n mut t = 0\n for i in b {\n if i > a { t = t + c }\n }\n flow r = 5 |> inc\n return t\n}");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].1.contains("'r'"), "{w:?}");
}

#[test]
fn unused_loop_variable_and_match_binding_are_reported() {
    let w = warnings("enum E { A(Int), B }\nfn main() -> Int {\n for x in [1] { emit(0) }\n match E.A(1) {\n E.A(v) => { emit(0) }\n E.B => { emit(1) }\n }\n return 0\n}");
    assert_eq!(w.len(), 2, "{w:?}");
}

#[test]
fn code_after_return_is_reported_once_per_block() {
    let w = warnings("fn main() -> Int {\n return 0\n emit(1)\n emit(2)\n}");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].0, 4);
    assert!(w[0].1.contains("unreachable"), "{w:?}");
}

#[test]
fn clean_program_has_no_warnings_and_errors_come_first() {
    assert!(warnings("fn main() -> Int {\n emit(1)\n return 0\n}").is_empty());
    assert!(
        Compiler::check_with_warnings("fn main() -> Int {\n let a: Int = \"x\"\n return 0\n}")
            .is_err()
    );
}

// ---- .yomc files ----

use yomnova_bytecode::module::{decode, encode};

fn module_for(src: &str) -> yomnova_bytecode::module::Module {
    match Compiler::compile_module(&format!("module t\n{src}")) {
        Ok(m) => m,
        Err(d) => panic!("compile failed: {d:?}"),
    }
}

const SAMPLE: &str = "struct P { x: Int, y: Int }\nenum E { A(Int), B }\nfn f(n: Int) -> Int {\n return n + 1\n}\nfn main() -> Int {\n let p = P { x: 1, y: 2 }\n emit(p)\n emit(f(41))\n for v in [E.A(5), E.B] {\n match v {\n E.A(n) => { emit(n) }\n E.B => { emit(\"b\") }\n }\n }\n return 0\n}";

#[test]
fn a_saved_program_behaves_exactly_like_the_source() {
    let direct = Compiler::compile_and_run(&format!("module t\n{SAMPLE}")).unwrap();
    let bytes = encode(&module_for(SAMPLE)).unwrap();
    let loaded = decode(&bytes).unwrap();
    let (result, lines) = Compiler::run_module(&loaded).unwrap();
    assert_eq!(lines, direct.output_lines);
    assert_eq!(Some(result), direct.execution_result);
    assert_eq!(lines, ["P { x: 1, y: 2 }", "42", "5", "b"]);
}

#[test]
fn encoding_is_deterministic() {
    let m = module_for(SAMPLE);
    assert_eq!(encode(&m).unwrap(), encode(&m).unwrap());
}

/// Corrupt valid files in many ways: loading must never panic, and anything that still loads must
/// run to a result or a clean error (the step limit stops loops).
#[test]
fn corrupted_files_never_panic_the_loader_or_the_vm() {
    let good = encode(&module_for(SAMPLE)).unwrap();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut loaded = 0usize;
    for _ in 0..4000 {
        let mut bytes = good.clone();
        for _ in 0..(1 + next() % 4) {
            let i = (next() as usize) % bytes.len();
            bytes[i] = (next() & 0xFF) as u8;
        }
        if let Ok(m) = decode(&bytes) {
            loaded += 1;
            let _ = Compiler::run_module(&m);
        }
    }
    assert!(loaded > 100, "mutations should often still load ({loaded})");
}

#[test]
fn hand_made_bad_programs_fail_cleanly() {
    use yomnova_bytecode::{Chunk, OpCode, Value};
    let run_main = |main: Chunk| {
        let m = yomnova_bytecode::module::Module {
            main,
            functions: vec![],
        };
        Compiler::run_module(&m)
    };
    // Constant index out of range.
    let mut c = Chunk::new();
    c.write_op(OpCode::Constant, 1);
    c.write_u16(9, 1);
    assert!(run_main(c).is_err());
    // Global name that is not text.
    let mut c = Chunk::new();
    c.constants.push(Value::Int(1));
    c.write_op(OpCode::GetGlobal, 1);
    c.write_u16(0, 1);
    assert!(run_main(c).is_err());
    // Stack underflow, call of a non-function, unknown opcode byte.
    let mut c = Chunk::new();
    c.write_op(OpCode::Add, 1);
    assert!(run_main(c).is_err());
    let mut c = Chunk::new();
    c.write_op(OpCode::Nil, 1);
    c.write_op(OpCode::Call, 1);
    c.write_byte(0, 1);
    assert!(run_main(c).is_err());
    let mut c = Chunk::new();
    c.write_byte(250, 1);
    assert!(run_main(c).is_err());
    // A jump to itself runs into the step limit instead of hanging.
    let mut c = Chunk::new();
    c.write_op(OpCode::Jump, 1);
    c.write_u16(0, 1);
    assert!(run_main(c).is_err());
    // A huge local slot is budgeted, not allocated blindly (65535 slots is within the budget, so it runs).
    let mut c = Chunk::new();
    c.write_op(OpCode::Nil, 1);
    c.write_op(OpCode::SetLocal, 1);
    c.write_u16(65_535, 1);
    c.write_op(OpCode::Halt, 1);
    assert!(run_main(c).is_ok());
}

// ---- IPv6 ----

#[test]
fn ipv6_values_parse_display_and_compare() {
    ok(
        "fn main() -> Int {\n let a: IPv6 = IPv6(\"::1\")\n emit(a)\n emit(IPv6(\"2001:db8:0:0:0:0:0:1\"))\n emit(IPv6(\"::1\") == a)\n emit(IPv6(\"::2\") == a)\n return 0\n}",
        &["::1", "2001:db8::1", "true", "false"],
    );
}

#[test]
fn ipv6_errors_are_clean() {
    err(
        "fn main() -> Int {\n emit(IPv6(\"not an address\"))\n return 0\n}",
        "Invalid IPv6 address",
    );
    err(
        "fn main() -> Int {\n emit(IPv6(5))\n return 0\n}",
        "expected Text, found Int",
    );
    err(
        "fn main() -> Int {\n let a: IPv4 = IPv6(\"::1\")\n return 0\n}",
        "expected IPv4, found IPv6",
    );
}

// ---- element and field assignment ----

const PQ: &str = "struct P { x: Int, y: Int }\nstruct Box { p: P, vals: Array<Int> }\n";

fn with_types(main: &str) -> String {
    format!("{PQ}fn main() -> Int {{\n{main}\n return 0\n}}")
}

#[test]
fn element_and_field_assignment_work_at_any_depth() {
    ok(
        &with_types(
            " mut xs = [1, 2, 3]\n xs[1] = 20\n emit(xs)\n mut p = P { x: 1, y: 2 }\n p.x = 10\n emit(p.x + p.y)\n mut b = Box { p: P { x: 5, y: 6 }, vals: [7, 8, 9] }\n b.vals[2] = 90\n b.p.y = 60\n emit(b.vals[2])\n emit(b.p.y)\n mut g = [[1, 2], [3, 4]]\n g[1][0] = 33\n emit(g)",
        ),
        &["[1, 20, 3]", "12", "90", "60", "[[1, 2], [33, 4]]"],
    );
}

#[test]
fn assignment_has_value_semantics_and_works_in_loops_and_functions() {
    ok(
        "fn bump(xs: Array<Int>) -> Array<Int> {\n mut ys = xs\n ys[0] = 99\n return ys\n}\nfn main() -> Int {\n mut xs = [1, 2, 3]\n mut i = 0\n while i < 3 {\n xs[i] = xs[i] * 2\n i = i + 1\n }\n emit(xs)\n emit(bump(xs))\n return 0\n}",
        &["[2, 4, 6]", "[99, 4, 6]"],
    );
}

#[test]
fn assignment_target_errors() {
    err(&with_types(" let xs = [1]\n xs[0] = 2"), "immutable");
    err(
        &with_types(" let p = P { x: 1, y: 2 }\n p.x = 2"),
        "immutable",
    );
    err(
        &with_types(" mut xs = [1]\n xs[0] = \"no\""),
        "expected Int, found Text",
    );
    err(
        &with_types(" mut xs = [1]\n xs[\"a\"] = 1"),
        "expected Int, found Text",
    );
    err(
        &with_types(" mut p = P { x: 1, y: 2 }\n p.z = 1"),
        "no field 'z'",
    );
    err(
        &with_types(" mut p = P { x: 1, y: 2 }\n p.x = true"),
        "expected Int, found Bool",
    );
    err(&with_types(" mut n = 5\n n[0] = 1"), "Array");
    err(&with_types(" mut n = 5\n n.f = 1"), "cannot read field");
    err(&with_types(" nope[0] = 1"), "Undefined identifier");
    err(&with_types(" f() = 1"), "Invalid assignment target");
    err(&with_types(" 5 = 1"), "Invalid assignment target");
}

#[test]
fn nested_assignment_rejects_calls_in_indices_but_single_level_allows_them() {
    err(
        "fn k() -> Int {\n return 0\n}\nfn main() -> Int {\n mut g = [[1, 2], [3, 4]]\n g[k()][0] = 1\n return 0\n}",
        "must not contain a call",
    );
    ok(
        "fn k() -> Int {\n emit(\"k\")\n return 1\n}\nfn main() -> Int {\n mut xs = [1, 2, 3]\n xs[k()] = 7\n emit(xs)\n return 0\n}",
        &["k", "[1, 7, 3]"],
    );
}

#[test]
fn assignment_out_of_bounds_is_a_runtime_error() {
    let e = run("fn main() -> Int {\n mut xs = [1, 2]\n xs[2] = 5\n return 0\n}").unwrap_err();
    assert!(e.contains("out of bounds"), "{e}");
    let e = run("fn main() -> Int {\n mut xs = [1, 2]\n xs[0 - 1] = 5\n return 0\n}").unwrap_err();
    assert!(e.contains("out of bounds"), "{e}");
}

#[test]
fn assigning_into_a_moved_value_is_an_ownership_error() {
    err(
        "fn take(xs: Array<Int>) -> Unit {\n emit(len(xs))\n}\nfn main() -> Int {\n mut xs = [1, 2]\n take(xs)\n xs[0] = 1\n return 0\n}",
        "moved",
    );
}

#[test]
fn a_variable_that_is_only_written_is_reported_as_unread() {
    let (_, warnings) = Compiler::check_with_warnings(
        "fn main() -> Int {\n mut xs = [1, 2]\n xs[0] = 5\n return 0\n}",
    )
    .unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.message.contains("'xs' is never read")),
        "{warnings:?}"
    );
}

// ---- NetworkBuffer and Packet ----

#[test]
fn buffers_read_and_write_big_endian_with_value_semantics() {
    ok(
        "fn main() -> Int {\n let a: NetworkBuffer = NetworkBuffer(8)\n let b: NetworkBuffer = buf_set32(a, 0, 3735928559)\n let c: NetworkBuffer = buf_set16(b, 4, 513)\n emit(buf_get(c, 0))\n emit(buf_get16(c, 0))\n emit(buf_get32(c, 0))\n emit(buf_get(c, 4))\n emit(buf_get(c, 5))\n emit(len(c))\n return 0\n}",
        &["222", "57005", "3735928559", "2", "1", "8"],
    );
    // The original is unchanged because writes return a new buffer (a is consumed, so rebuild it to check).
    ok(
        "fn main() -> Int {\n mut a: NetworkBuffer = NetworkBuffer(2)\n let b: NetworkBuffer = buf_set(a, 0, 9)\n emit(buf_get(b, 0))\n a = NetworkBuffer(2)\n emit(buf_get(a, 0))\n return 0\n}",
        &["9", "0"],
    );
}

#[test]
fn buffer_access_is_bounds_checked() {
    for (call, want) in [
        ("buf_get(b, 4)", "out of bounds"),
        ("buf_get(b, 0 - 1)", "out of bounds"),
        ("buf_get16(b, 3)", "out of bounds"),
        ("buf_get32(b, 1)", "out of bounds"),
        ("len(buf_set(b, 4, 1))", "out of bounds"),
        ("len(buf_set(b, 0, 256))", "does not fit"),
        ("len(buf_set16(b, 0, 65536))", "does not fit"),
        ("len(buf_set(b, 0, 0 - 1))", "does not fit"),
        ("len(buf_slice(b, 2, 3))", "out of bounds"),
        ("len(buf_slice(b, 0, 0 - 1))", "negative"),
        ("len(NetworkBuffer(0 - 1))", "negative"),
        ("len(NetworkBuffer(2000000))", "exceeds the limit"),
    ] {
        let e = run(&format!("fn main() -> Int {{\n let b: NetworkBuffer = NetworkBuffer(4)\n emit({call})\n return 0\n}}")).unwrap_err();
        assert!(e.contains(want), "{call}: {e}");
    }
}

#[test]
fn buffer_type_errors_and_ownership() {
    err(
        "fn main() -> Int {\n emit(buf_get(5, 0))\n return 0\n}",
        "expected NetworkBuffer, found Int",
    );
    err(
        "fn main() -> Int {\n emit(buf_get(NetworkBuffer(1), \"a\"))\n return 0\n}",
        "expected Int, found Text",
    );
    err(
        "fn main() -> Int {\n let p: Packet = Packet(NetworkBuffer(1), 5)\n return 0\n}",
        "expected NetworkBuffer, found Int",
    );
    err(
        "fn main() -> Int {\n let b: IPv4 = NetworkBuffer(1)\n return 0\n}",
        "expected IPv4, found NetworkBuffer",
    );
    err(
        "fn main() -> Int {\n let x: Blob = NetworkBuffer(1)\n return 0\n}",
        "unknown type 'Blob'",
    );
    // Reads borrow; writes and constructors consume.
    ok(
        "fn main() -> Int {\n let b: NetworkBuffer = NetworkBuffer(2)\n emit(buf_get(b, 0))\n emit(buf_get(b, 1))\n emit(len(b))\n return 0\n}",
        &["0", "0", "2"],
    );
    err(
        "fn main() -> Int {\n let b: NetworkBuffer = NetworkBuffer(2)\n let c: NetworkBuffer = buf_set(b, 0, 1)\n emit(buf_get(b, 0))\n return 0\n}",
        "moved",
    );
}

#[test]
fn packets_hold_a_header_and_a_payload() {
    ok(
        "fn main() -> Int {\n let p: Packet = Packet(bytes_of(\"ab\"), bytes_of(\"xyz\"))\n emit(len(packet_header(p)))\n emit(buf_get(packet_payload(p), 2))\n emit(p)\n return 0\n}",
        &["2", "122", "Packet(header=2 bytes, payload=3 bytes)"],
    );
    err(
        "fn main() -> Int {\n emit(packet_header(NetworkBuffer(1)))\n return 0\n}",
        "expected Packet, found NetworkBuffer",
    );
}

// ---- IR optimiser: must never change what a program does ----

fn outcome(source: &str, optimize: bool) -> Result<(Vec<String>, String), String> {
    match Compiler::compile_and_run_opt(source, optimize) {
        Ok(out) => Ok((out.output_lines, format!("{:?}", out.execution_result))),
        Err(d) => Err(d
            .iter()
            .map(|x| x.message.clone())
            .collect::<Vec<_>>()
            .join("\n")),
    }
}

#[test]
fn optimiser_never_changes_behaviour() {
    let mut programs: Vec<String> = Vec::new();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for entry in std::fs::read_dir(&dir).expect("examples dir") {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|e| e == "yom") {
            programs.push(std::fs::read_to_string(&p).unwrap());
        }
    }
    assert!(programs.len() >= 5, "expected the example programs");
    let inline = [
        // folding, including signs, division and modulo
        "fn main() -> Int {\n emit(2 + 3 * 4)\n emit(7 / 2)\n emit(0 - 7 / 2)\n emit((0 - 7) % 3)\n emit(7 % (0 - 3))\n emit(-5)\n emit(1 < 2)\n emit(2 <= 1)\n emit(\"a\" + \"b\")\n emit(\"a\" == \"a\")\n emit(true == false)\n emit(!true)\n return 0\n}",
        // constant conditions in if/while/else-if chains
        "fn main() -> Int {\n if true {\n emit(\"t\")\n } else {\n emit(\"f\")\n }\n if false {\n emit(\"no\")\n } else if 1 < 2 {\n emit(\"yes\")\n }\n while false {\n emit(\"never\")\n }\n mut i = 0\n while i < 3 {\n i = i + 1\n }\n emit(i)\n return i\n}",
        // errors must survive optimisation unchanged
        "fn main() -> Int {\n emit(1)\n emit(1 / 0)\n return 0\n}",
        "fn main() -> Int {\n emit(9223372036854775807 + 1)\n return 0\n}",
        "fn main() -> Int {\n emit(1 % 0)\n return 0\n}",
        // returns in branches and nested loops
        "fn f(n: Int) -> Int {\n if n < 0 {\n return 0 - 1\n }\n mut s = 0\n mut i = 0\n while i < n {\n mut j = 0\n while j < 2 {\n s = s + i * j\n j = j + 1\n }\n i = i + 1\n }\n return s\n}\nfn main() -> Int {\n emit(f(0 - 3))\n emit(f(5))\n return 0\n}",
        // recursion, short-circuit, match, for
        "fn fib(n: Int) -> Int {\n if n < 2 {\n return n\n }\n return fib(n - 1) + fib(n - 2)\n}\nfn main() -> Int {\n emit(fib(15))\n emit(true && false || true)\n for x in [1, 2, 3] {\n emit(x * 2)\n }\n return 0\n}",
    ];
    programs.extend(inline.iter().map(|s| s.to_string()));
    let mut compared = 0;
    for src in &programs {
        assert_eq!(
            outcome(src, true),
            outcome(src, false),
            "optimiser changed behaviour of:\n{src}"
        );
        compared += 1;
    }
    assert!(compared >= 10);
}

#[test]
fn optimiser_shrinks_constant_code_and_keeps_the_verifier_happy() {
    let src = "fn main() -> Int {\n emit(2 + 3 * 4)\n if true {\n emit(\"a\" + \"b\")\n } else {\n emit(\"dead\")\n }\n return 1 + 1\n}";
    let raw = Compiler::compile_unit_opt(src, false).unwrap();
    let opt = Compiler::compile_unit_opt(src, true).unwrap();
    assert!(
        opt.main.code.len() < raw.main.code.len(),
        "{} !< {}",
        opt.main.code.len(),
        raw.main.code.len()
    );
    assert!(
        !opt.disassemble_all().contains("dead"),
        "unreachable else branch was removed"
    );
    // Optimised modules still pass the loader's structural verifier after a round trip through `.yomc`.
    let module = Compiler::compile_module_opt(src, true).unwrap();
    let bytes = yomnova_bytecode::module::encode(&module).unwrap();
    assert!(yomnova_bytecode::module::decode(&bytes).is_ok());
    assert_eq!(outcome(src, true).unwrap().0, vec!["14", "ab"]);
}

// ---- borrowing: reference parameters ----

const SUM: &str = "fn total(xs: &Array<Int>) -> Int {\n mut s = 0\n for x in xs {\n s = s + x\n }\n return s\n}\n";
const BUMP: &str =
    "fn bump(xs: &mut Array<Int>) -> Int {\n xs[0] = xs[0] + 1\n return len(xs)\n}\n";

#[test]
fn shared_borrows_read_without_moving() {
    ok(
        &format!("{SUM}fn main() -> Int {{\n let d = [1, 2, 3]\n emit(total(&d))\n emit(total(&d))\n emit(d)\n return 0\n}}"),
        &["6", "6", "[1, 2, 3]"],
    );
    // Two shared borrows of the same variable in one call are fine.
    ok(
        "fn both(a: &Array<Int>, b: &Array<Int>) -> Int {\n return len(a) + len(b)\n}\nfn main() -> Int {\n let d = [1, 2]\n emit(both(&d, &d))\n return 0\n}",
        &["4"],
    );
}

#[test]
fn mutable_borrows_write_back_to_the_caller() {
    ok(
        &format!("{BUMP}fn main() -> Int {{\n mut d = [1, 2, 3]\n emit(bump(&mut d))\n emit(bump(&mut d))\n emit(d)\n return 0\n}}"),
        &["3", "3", "[3, 2, 3]"],
    );
    ok(
        "struct P { x: Int }\nfn inc(p: &mut P, n: &mut Int) -> Unit {\n p.x = p.x + 1\n n = n + 10\n}\nfn main() -> Int {\n mut p = P { x: 1 }\n mut n = 5\n inc(&mut p, &mut n)\n inc(&mut p, &mut n)\n emit(p.x)\n emit(n)\n return 0\n}",
        &["3", "25"],
    );
    // Early returns, recursion and reborrowing all write back.
    ok(
        "fn count(n: &mut Int, left: Int) -> Int {\n if left == 0 {\n return n\n }\n n = n + 1\n count(&mut n, left - 1)\n return n\n}\nfn main() -> Int {\n mut c = 0\n emit(count(&mut c, 4))\n emit(c)\n return 0\n}",
        &["4", "4"],
    );
    // A write-back target may be a global-less local inside a loop body.
    ok(
        &format!("{BUMP}fn main() -> Int {{\n mut d = [0]\n mut i = 0\n while i < 3 {{\n bump(&mut d)\n i = i + 1\n }}\n emit(d)\n return 0\n}}"),
        &["[3]"],
    );
}

#[test]
fn borrow_rule_violations_are_rejected() {
    // same call, conflicting borrows
    err(
        "fn f(a: &mut Array<Int>, b: &Array<Int>) -> Unit {\n a[0] = len(b)\n}\nfn main() -> Int {\n mut d = [1]\n f(&mut d, &d)\n return 0\n}",
        "already borrowed or used",
    );
    err(
        "fn f(a: &mut Array<Int>, b: &mut Array<Int>) -> Unit {\n a[0] = 1\n}\nfn main() -> Int {\n mut d = [1]\n f(&mut d, &mut d)\n return 0\n}",
        "already borrowed or used",
    );
    err(
        "fn f(a: &mut Array<Int>, n: Int) -> Unit {\n a[0] = n\n}\nfn main() -> Int {\n mut d = [1]\n f(&mut d, len(d))\n return 0\n}",
        "already borrowed or used",
    );
    err(
        "fn f(a: &Array<Int>, b: Array<Int>) -> Unit {\n emit(len(a))\n}\nfn main() -> Int {\n let d = [1]\n f(&d, d)\n return 0\n}",
        "already borrowed or used",
    );
    // &mut needs a mutable variable
    err(
        &format!("{BUMP}fn main() -> Int {{\n let d = [1]\n bump(&mut d)\n return 0\n}}"),
        "not declared `mut`",
    );
    // wrong kind of argument for the parameter
    err(
        &format!("{BUMP}fn main() -> Int {{\n mut d = [1]\n bump(&d)\n return 0\n}}"),
        "expected &mut Array<Int>, found &Array<Int>",
    );
    err(
        &format!("{BUMP}fn main() -> Int {{\n mut d = [1]\n bump(d)\n return 0\n}}"),
        "expected &mut Array<Int>, found Array<Int>",
    );
    err(
        &format!("{SUM}fn main() -> Int {{\n let d = [1]\n emit(total(&mut d))\n return 0\n}}"),
        "not declared `mut`",
    );
    err("fn f(n: Int) -> Int {\n return n\n}\nfn main() -> Int {\n let d = 1\n emit(f(&d))\n return 0\n}", "expected Int, found &Int");
    // borrowing a moved value
    err(
        &format!("{SUM}fn eat(xs: Array<Int>) -> Unit {{\n emit(len(xs))\n}}\nfn main() -> Int {{\n let d = [1]\n eat(d)\n emit(total(&d))\n return 0\n}}"),
        "moved",
    );
}

#[test]
fn references_are_parameter_only_and_cannot_be_moved_out() {
    err(
        "fn main() -> Int {\n let d = [1]\n let r = &d\n return 0\n}",
        "only allowed as a function-call argument",
    );
    err(
        "fn main() -> Int {\n let r: &Int = 1\n return 0\n}",
        "only allowed as function parameter types",
    );
    err(
        "fn f() -> &Int {\n return 1\n}\nfn main() -> Int {\n return 0\n}",
        "only allowed as function parameter types",
    );
    err(
        "struct S { r: &Int }\nfn main() -> Int {\n return 0\n}",
        "only allowed as function parameter types",
    );
    err(
        "fn f(xs: &Array<&Int>) -> Unit {\n}\nfn main() -> Int {\n return 0\n}",
        "only allowed as function parameter types",
    );
    err(
        "fn f(mut xs: &Array<Int>) -> Unit {\n}\nfn main() -> Int {\n return 0\n}",
        "cannot also be declared `mut`",
    );
    err(
        "fn main() -> Int {\n let d = [1]\n emit(len(&d))\n return 0\n}",
        "found &Array<Int>",
    );
    // only variables can be borrowed
    err("fn f(xs: &Array<Int>) -> Unit {\n}\nfn main() -> Int {\n let d = [[1]]\n f(&d[0])\n return 0\n}", "Only a variable can be borrowed");
    // reading is fine; moving a non-copy value out of a reference is not
    err(
        "fn f(xs: &Array<Int>) -> Array<Int> {\n return xs\n}\nfn main() -> Int {\n return 0\n}",
        "Cannot move out of borrowed",
    );
    err("fn eat(xs: Array<Int>) -> Unit {\n}\nfn f(xs: &Array<Int>) -> Unit {\n eat(xs)\n}\nfn main() -> Int {\n return 0\n}", "Cannot move out of borrowed");
    // a shared reference cannot be written through
    err(
        "fn f(xs: &Array<Int>) -> Unit {\n xs[0] = 1\n}\nfn main() -> Int {\n return 0\n}",
        "immutable",
    );
    ok("fn f(n: &Int) -> Int {\n return n + 1\n}\nfn main() -> Int {\n let a = 1\n emit(f(&a))\n return 0\n}", &["2"]);
}

#[test]
fn std_modules_can_be_imported_and_used() {
    let out = Compiler::compile_and_run(
        "import std.math\nimport std.arrays\nfn main() -> Int {\n    emit(math.abs(0 - 7))\n    emit(math.pow(2, 10))\n    emit(math.clamp(15, 0, 10))\n    let xs = [4, 8, 15]\n    emit(arrays.sum(&xs))\n    emit(arrays.contains(&xs, 8))\n    emit(arrays.index_of(&xs, 99))\n    emit(arrays.max_of(&xs))\n    emit(len(xs))\n    return 0\n}\n",
    )
    .expect("runs");
    assert_eq!(
        out.output_lines,
        vec!["7", "1024", "10", "27", "true", "-1", "15", "3"]
    );
}

#[test]
fn bad_imports_are_reported() {
    for src in [
        "import std.nope\nfn main() -> Int { return 0 }\n",
        "import mylib\nfn main() -> Int { return 0 }\n",
        "import std.math\nfn main() -> Int { return math.nope(1) }\n",
    ] {
        assert!(Compiler::compile_and_run(src).is_err(), "{src}");
    }
}

#[test]
fn imports_do_not_shift_error_lines() {
    let Err(e) = Compiler::compile_and_run(
        "import std.math\nfn main() -> Int {\n    let x: Int = \"a\"\n    return 0\n}\n",
    ) else {
        panic!("should fail")
    };
    assert_eq!(e[0].span.as_ref().map(|s| s.line), Some(3));
}

#[test]
fn every_compiled_example_passes_the_stack_verifier() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("examples dir") {
        let p = entry.unwrap().path();
        if !p.extension().is_some_and(|e| e == "yom") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        for optimize in [true, false] {
            let Ok(unit) = Compiler::compile_unit_opt(&src, optimize) else {
                continue;
            };
            yomnova_bytecode::module::verify_chunk_arity(&unit.main, 0)
                .unwrap_or_else(|e| panic!("{}: main: {e}", p.display()));
            for (name, chunk, arity) in &unit.functions {
                yomnova_bytecode::module::verify_chunk_arity(chunk, *arity)
                    .unwrap_or_else(|e| panic!("{}: {name}: {e}", p.display()));
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 20,
        "expected to verify the example programs, got {checked}"
    );
}

#[test]
fn text_library_works_through_std_text_and_natives() {
    let out = Compiler::compile_and_run(
        "import std.text\nfn main() -> Int {\n    let s = \"  Hello, YOMNOVA  \"\n    let t = text.trim(&s)\n    emit(text.upper(&t))\n    emit(text.find(&t, \"YOM\"))\n    emit(text.slice(&t, 0, 5))\n    emit(text.replace(&t, \"Hello\", \"Bye\"))\n    let parts = text.split(&t, \", \")\n    emit(len(parts))\n    emit(parts[1])\n    let n = \"41\"\n    emit(text.parse_int(&n) + 1)\n    emit(str_from_int(7) + \"!\")\n    emit(str_repeat(\"ab\", 3))\n    emit(str_starts_with(t, \"Hel\"))\n    emit(len(t))\n    emit(str_find(\"héllo wörld\", \"w\"))\n    return 0\n}\n",
    )
    .expect("runs");
    assert_eq!(
        out.output_lines,
        vec![
            "HELLO, YOMNOVA",
            "7",
            "Hello",
            "Bye, YOMNOVA",
            "2",
            "YOMNOVA",
            "42",
            "7!",
            "ababab",
            "true",
            "14",
            "6"
        ]
    );
}

#[test]
fn text_errors_are_reported_not_panics() {
    for body in [
        "emit(str_slice(\"abc\", 2, 9))",
        "emit(str_slice(\"abc\", 0 - 1, 2))",
        "emit(str_replace(\"abc\", \"\", \"x\"))",
        "emit(str_split(\"abc\", \"\"))",
        "emit(str_parse_int(\"12x\"))",
        "emit(str_repeat(\"ab\", 0 - 1))",
        "emit(str_repeat(\"abcdefgh\", 9223372036854775807))",
    ] {
        let src = format!("fn main() -> Int {{\n    {body}\n    return 0\n}}\n");
        assert!(Compiler::compile_and_run(&src).is_err(), "{body}");
    }
    // wrong argument types are caught by the type checker
    assert!(Compiler::compile_and_run(
        "fn main() -> Int {\n    emit(str_upper(5))\n    return 0\n}\n"
    )
    .is_err());
}

#[test]
fn file_access_is_off_by_default_and_confined_to_the_allowed_directory() {
    let dir = std::env::temp_dir().join(format!("yomnova-fs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("../yomnova-outside.txt"), "secret").unwrap();
    let run = |body: &str, root: Option<&std::path::Path>| {
        let src = format!("fn main() -> Int {{\n    {body}\n    return 0\n}}\n");
        Compiler::compile_and_run_with(&src, true, root.map(|p| p.to_path_buf()))
    };
    // disabled without a directory
    assert!(run("emit(file_exists(\"a.txt\"))", None).is_err());
    // read, write, append, exists inside the directory
    let ok = run(
        "file_write(\"a.txt\", \"x\")\n    file_append(\"a.txt\", \"y\")\n    file_write(\"sub/b.txt\", \"z\")\n    emit(file_read(\"a.txt\"))\n    emit(file_exists(\"sub/b.txt\"))\n    emit(file_exists(\"nope.txt\"))",
        Some(&dir),
    )
    .expect("runs");
    assert_eq!(ok.output_lines, vec!["xy", "true", "false"]);
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "xy");
    // escapes are refused
    for body in [
        "emit(file_read(\"../yomnova-outside.txt\"))",
        "emit(file_read(\"/etc/hostname\"))",
        "file_write(\"../x.txt\", \"no\")",
        "emit(file_read(\"\"))",
        "emit(file_read(\"sub/../a.txt\"))",
        "emit(file_read(\"missing.txt\"))",
    ] {
        assert!(run(body, Some(&dir)).is_err(), "{body}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.join("../yomnova-outside.txt"), dir.join("link.txt"))
            .unwrap();
        assert!(
            run("emit(file_read(\"link.txt\"))", Some(&dir)).is_err(),
            "symlink escape"
        );
        assert!(
            run("file_write(\"link.txt\", \"no\")", Some(&dir)).is_err(),
            "symlink write escape"
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("../yomnova-outside.txt")).unwrap(),
        "secret"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(std::env::temp_dir().join("yomnova-outside.txt"));
}

const MATCH_PRELUDE: &str = "enum Inner {\n Num(Int),\n None,\n}\nenum Shape {\n Circle(Int),\n Boxed(Inner),\n Empty,\n}\n";

fn run_main(body: &str) -> Vec<String> {
    let src = format!("{MATCH_PRELUDE}fn main() -> Int {{\n{body}\n return 0\n}}\n");
    Compiler::compile_and_run(&src).expect(&src).output_lines
}

fn fails_with(body: &str, want: &str) {
    let src = format!("{MATCH_PRELUDE}fn main() -> Int {{\n{body}\n return 0\n}}\n");
    let Err(e) = Compiler::compile_and_run(&src) else {
        panic!("should fail: {body}")
    };
    let all: String = e
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(all.contains(want), "{all} should contain {want}");
}

#[test]
fn match_works_as_an_expression_with_nested_and_literal_patterns() {
    let out = run_main(
        " let s = Shape.Boxed(Inner.Num(7))\n let a = match s {\n Shape.Circle(r) => r * 3,\n Shape.Boxed(Inner.Num(n)) => n + 1,\n Shape.Boxed(Inner.None) => 0 - 1,\n Shape.Empty => 0\n }\n emit(a)\n emit(10 + match 2 { 1 => 100, 2 => 200, _ => 300 })\n emit(match true { true => \"t\", false => \"f\" })\n emit(match 5 { -5 => \"neg\", n => \"other\" })\n emit(match Shape.Boxed(Inner.None) { Shape.Boxed(x) => match x { Inner.Num(_) => 1, Inner.None => 2 }, _ => 3 })\n emit(match Shape.Circle(4) { Shape.Circle(0) => \"zero\", Shape.Circle(_) => \"some\", _ => \"other\" })",
    );
    assert_eq!(out, vec!["8", "210", "t", "other", "2", "some"]);
}

#[test]
fn nested_pattern_checks() {
    fails_with(
        " emit(match Shape.Empty { Shape.Circle(r) => r })",
        "not exhaustive: missing Shape.Boxed, Shape.Empty",
    );
    fails_with(" emit(match Shape.Empty { Shape.Boxed(Inner.Num(n)) => n, Shape.Circle(_) => 1, Shape.Empty => 2 })", "not exhaustive: some nested cases are not covered");
    fails_with(" emit(match 3 { 1 => 1, 2 => 2 })", "not exhaustive");
    fails_with(" emit(match true { true => 1 })", "both `true` and `false`");
    fails_with(" emit(match 1 { 1 => 1, 1 => 2, _ => 3 })", "can never run");
    fails_with(" emit(match Shape.Empty { Shape.Boxed(Inner.Num(n)) => n, Shape.Boxed(Inner.Num(m)) => m, _ => 0 })", "can never run");
    fails_with(" emit(match 1 { _ => 1, 2 => 2 })", "can never run");
    fails_with(
        " emit(match 1 { true => 1, _ => 2 })",
        "cannot match a value of type Int",
    );
    fails_with(
        " emit(match Shape.Empty { Shape.Circle(Inner.None) => 1, _ => 2 })",
        "expected a value matching Inner.None",
    );
    fails_with(" emit(match 1 { 1 => 1, _ => \"x\" })", "Type mismatch");
    fails_with(
        " emit(match Shape.Empty { Inner.None => 1, _ => 2 })",
        "pattern is for enum",
    );
}

#[test]
fn match_expression_bindings_and_moves() {
    // bindings are scoped to their arm; a moved non-copy value cannot be reused
    let src = format!("{MATCH_PRELUDE}fn main() -> Int {{\n let xs = [1, 2]\n let n = match 1 {{ 1 => len(xs), _ => 0 }}\n emit(n)\n emit(len(xs))\n return 0\n}}\n");
    assert_eq!(
        Compiler::compile_and_run(&src).unwrap().output_lines,
        vec!["2", "2"]
    );
    let bad = format!("{MATCH_PRELUDE}fn main() -> Int {{\n let r = match 1 {{ 1 => 5, _ => 0 }}\n emit(r)\n emit(x)\n return 0\n}}\n");
    assert!(Compiler::compile_and_run(&bad).is_err());
    let bad2 = format!("{MATCH_PRELUDE}fn main() -> Int {{\n let r = match Shape.Circle(2) {{ Shape.Circle(v) => v, _ => 0 }}\n emit(v)\n return 0\n}}\n");
    assert!(
        Compiler::compile_and_run(&bad2).is_err(),
        "arm binding must not leak"
    );
}

#[test]
fn std_net_parses_a_known_ipv4_header_and_its_checksum() {
    // The header from the Wikipedia IPv4 checksum example: checksum field 0xb861.
    let out = Compiler::compile_and_run(
        "import std.net\nfn main() -> Int {\n    let xs = [69, 0, 0, 115, 0, 0, 64, 0, 64, 17, 184, 97, 192, 168, 0, 1, 192, 168, 0, 199]\n    let b = net.from_bytes(&xs)\n    emit(net.ipv4_version(&b))\n    emit(net.ipv4_header_len(&b))\n    emit(net.ipv4_total_length(&b))\n    emit(net.ipv4_ttl(&b))\n    emit(net.ipv4_protocol(&b))\n    emit(net.ipv4_src(&b))\n    emit(net.ipv4_dst(&b))\n    emit(net.ipv4_compute_checksum(&b))\n    emit(net.ipv4_checksum_ok(&b))\n    let bad = [69, 0, 0, 115, 0, 0, 64, 0, 64, 17, 184, 98, 192, 168, 0, 1, 192, 168, 0, 199]\n    let c = net.from_bytes(&bad)\n    emit(net.ipv4_checksum_ok(&c))\n    emit(net.to_bytes(&b)[3])\n    emit(net.ipv4_valid(&b))\n    emit(net.ipv4_valid(&c))\n    let z = NetworkBuffer(20)\n    emit(net.ipv4_valid(&z))\n    return 0\n}\n",
    )
    .expect("runs");
    assert_eq!(
        out.output_lines,
        vec![
            "4",
            "20",
            "115",
            "64",
            "17",
            "192.168.0.1",
            "192.168.0.199",
            "47201",
            "true",
            "false",
            "115",
            "false",
            "false",
            "false"
        ]
    );
}

#[test]
fn std_net_builds_and_parses_udp_in_ipv4_and_reads_tcp_flags() {
    let out = Compiler::compile_and_run(
        "import std.net\nfn main() -> Int {\n    let udp = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let ip = net.make_ipv4(\"192.168.0.1\", \"10.0.0.7\", 17, 64, udp)\n    emit(net.ipv4_checksum_ok(&ip))\n    emit(net.ipv4_total_length(&ip))\n    emit(len(ip))\n    let inner = net.ipv4_payload(&ip)\n    emit(net.udp_src_port(&inner))\n    emit(net.udp_dst_port(&inner))\n    emit(net.udp_length(&inner))\n    emit(net.udp_payload(&inner))\n    let t = [0, 80, 48, 57, 0, 0, 0, 1, 0, 0, 0, 2, 80, 18, 0, 0, 0, 0, 0, 0, 104, 105]\n    let tcp = net.from_bytes(&t)\n    emit(net.tcp_src_port(&tcp))\n    emit(net.tcp_dst_port(&tcp))\n    emit(net.tcp_seq(&tcp))\n    emit(net.tcp_ack(&tcp))\n    emit(net.tcp_header_len(&tcp))\n    emit(net.tcp_syn(&tcp))\n    emit(net.tcp_ack_flag(&tcp))\n    emit(net.tcp_fin(&tcp))\n    emit(net.tcp_rst(&tcp))\n    emit(net.tcp_payload(&tcp))\n    emit(net.ipv4_valid(&ip))\n    return 0\n}\n",
    )
    .expect("runs");
    let l = &out.output_lines;
    assert_eq!(&l[0..6], ["true", "33", "33", "5000", "53", "13"]);
    assert_eq!(
        &l[7..16],
        ["80", "12345", "1", "2", "20", "true", "true", "false", "false"]
    );
    assert_eq!(l[6], "NetworkBuffer[5 bytes: 68 65 6c 6c 6f]");
    assert_eq!(l[16], "NetworkBuffer[2 bytes: 68 69]");
    assert_eq!(l[17], "true", "a packet built by make_ipv4 is valid");
}

#[test]
fn std_net_short_or_inconsistent_buffers_are_errors_not_garbage() {
    for body in [
        "let b = NetworkBuffer(10)\n    emit(net.ipv4_ttl(&b))\n    emit(net.ipv4_src(&b))",
        "let b = NetworkBuffer(3)\n    emit(net.udp_src_port(&b) + net.udp_length(&b))",
        "let b = NetworkBuffer(3)\n    emit(len(net.ipv4_payload(&b)))",
        "let xs = [1, 2]\n    let b = net.from_bytes(&xs)\n    emit(net.tcp_seq(&b))",
        "emit(len(net.make_ipv4(\"1.2.3\", \"1.2.3.4\", 17, 64, NetworkBuffer(0))))",
        "emit(len(net.make_ipv4(\"1.2.3.999\", \"1.2.3.4\", 17, 64, NetworkBuffer(0))))",
    ] {
        let src = format!("import std.net\nfn main() -> Int {{\n    {body}\n    return 0\n}}\n");
        assert!(Compiler::compile_and_run(&src).is_err(), "{body}");
    }
}

#[test]
fn match_guards_choose_arms_and_fall_through() {
    let out = run_main(
        " let t = Shape.Circle(7)\n let a = match t {\n Shape.Circle(r) if r > 10 => \"big\",\n Shape.Circle(r) if r > 5 => \"medium\",\n Shape.Circle(_) => \"small\",\n _ => \"other\"\n }\n emit(a)\n emit(match 4 { n if n % 2 == 0 => \"even\", _ => \"odd\" })\n emit(match 5 { n if n % 2 == 0 => \"even\", _ => \"odd\" })\n match Shape.Circle(1) {\n Shape.Circle(r) if r == 2 => { emit(\"two\") }\n Shape.Circle(r) if r == 1 => { emit(\"one\") }\n _ => { emit(\"none\") }\n }\n emit(match 1 { x if x > 0 && x < 3 => x * 10, _ => 0 })",
    );
    assert_eq!(out, vec!["medium", "even", "odd", "one", "10"]);
}

#[test]
fn match_guard_rules_are_checked() {
    // guarded arms do not make a match exhaustive
    fails_with(" emit(match 1 { x if x > 0 => 1 })", "not exhaustive");
    fails_with(
        " emit(match Shape.Empty { Shape.Circle(r) if r > 0 => 1, Shape.Boxed(_) => 2, Shape.Empty => 3 })",
        "not exhaustive",
    );
    // the guard must be a Bool
    fails_with(" emit(match 1 { x if x + 1 => 1, _ => 2 })", "Bool");
    // an arm after a guarded `_` is still reachable
    assert_eq!(
        run_main(" emit(match 3 { _ if false => 1, 3 => 2, _ => 3 })"),
        vec!["2"]
    );
    // a guard may read bindings but not undefined names
    fails_with(" emit(match 1 { x if y > 0 => 1, _ => 2 })", "y");
    // a guard cannot move a value
    let src = format!("{MATCH_PRELUDE}fn eat(xs: Array<Int>) -> Bool {{\n return true\n}}\nfn main() -> Int {{\n let xs = [1]\n emit(match 1 {{ n if eat(xs) => n, _ => 0 }})\n return 0\n}}\n");
    let Err(e) = Compiler::compile_and_run(&src) else {
        panic!("a guard that moves a value must be rejected")
    };
    assert!(
        e.iter().any(|d| d.message.contains("guard cannot move")),
        "{e:?}"
    );
}

// Expected values below come from an independent Python implementation of RFC 1071 with the RFC 768 / 793 /
// 8200 pseudo-headers (not from this library).
#[test]
fn std_net_l4_checksums_match_an_independent_implementation() {
    let out = Compiler::compile_and_run(
        "import std.net\nfn main() -> Int {\n    let u = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let p4 = net.fill_ipv4_udp_checksum(net.make_ipv4(\"192.168.0.1\", \"10.0.0.7\", 17, 64, u))\n    emit(net.ipv4_udp_checksum(&p4))\n    emit(net.ipv4_udp_checksum_ok(&p4))\n    let u6 = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let p6 = net.fill_ipv6_udp_checksum(net.make_ipv6(\"2001:db8::1\", \"fe80::2\", 17, 64, u6))\n    emit(net.ipv6_udp_checksum(&p6))\n    emit(net.ipv6_udp_checksum_ok(&p6))\n    let t = [0, 80, 48, 57, 0, 0, 0, 1, 0, 0, 0, 2, 80, 18, 0, 0, 0, 0, 0, 0, 104, 105]\n    let seg = net.from_bytes(&t)\n    let t4 = net.fill_ipv4_tcp_checksum(net.make_ipv4(\"192.168.0.1\", \"10.0.0.7\", 6, 64, seg))\n    emit(net.ipv4_tcp_checksum(&t4))\n    emit(net.ipv4_tcp_checksum_ok(&t4))\n    let seg6 = net.from_bytes(&t)\n    let t6 = net.fill_ipv6_tcp_checksum(net.make_ipv6(\"2001:db8::1\", \"fe80::2\", 6, 64, seg6))\n    emit(net.ipv6_tcp_checksum(&t6))\n    emit(net.ipv6_tcp_checksum_ok(&t6))\n    let odd = net.make_udp(1, 2, bytes_of(\"abc\"))\n    emit(net.ipv4_udp_checksum(&net.make_ipv4(\"10.1.1.1\", \"10.2.2.2\", 17, 64, odd)))\n    return 0\n}\n",
    );
    // `&net.make_ipv4(..)` is not a variable, so that last line must be rejected by the borrow rule:
    assert!(out.is_err());
    let out = Compiler::compile_and_run(
        "import std.net\nfn main() -> Int {\n    let u = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let p4 = net.fill_ipv4_udp_checksum(net.make_ipv4(\"192.168.0.1\", \"10.0.0.7\", 17, 64, u))\n    emit(net.ipv4_udp_checksum(&p4))\n    emit(net.ipv4_udp_checksum_ok(&p4))\n    let u6 = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let p6 = net.fill_ipv6_udp_checksum(net.make_ipv6(\"2001:db8::1\", \"fe80::2\", 17, 64, u6))\n    emit(net.ipv6_udp_checksum(&p6))\n    emit(net.ipv6_udp_checksum_ok(&p6))\n    let t = [0, 80, 48, 57, 0, 0, 0, 1, 0, 0, 0, 2, 80, 18, 0, 0, 0, 0, 0, 0, 104, 105]\n    let seg = net.from_bytes(&t)\n    let t4 = net.fill_ipv4_tcp_checksum(net.make_ipv4(\"192.168.0.1\", \"10.0.0.7\", 6, 64, seg))\n    emit(net.ipv4_tcp_checksum(&t4))\n    emit(net.ipv4_tcp_checksum_ok(&t4))\n    let seg6 = net.from_bytes(&t)\n    let t6 = net.fill_ipv6_tcp_checksum(net.make_ipv6(\"2001:db8::1\", \"fe80::2\", 6, 64, seg6))\n    emit(net.ipv6_tcp_checksum(&t6))\n    emit(net.ipv6_tcp_checksum_ok(&t6))\n    let odd = net.make_udp(1, 2, bytes_of(\"abc\"))\n    let o4 = net.make_ipv4(\"10.1.1.1\", \"10.2.2.2\", 17, 64, odd)\n    emit(net.ipv4_udp_checksum(&o4))\n    return 0\n}\n",
    )
    .expect("runs");
    assert_eq!(
        out.output_lines,
        vec!["56724", "true", "31752", "true", "19499", "true", "60062", "true", "9325"]
    );
}

#[test]
fn std_net_detects_corrupted_packets_and_zero_udp_checksum_rules() {
    let out = Compiler::compile_and_run(
        "import std.net\nfn main() -> Int {\n    let u = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let p = net.fill_ipv4_udp_checksum(net.make_ipv4(\"1.2.3.4\", \"5.6.7.8\", 17, 64, u))\n    emit(net.ipv4_udp_checksum_ok(&p))\n    let bad = buf_set(p, 32, 99)\n    emit(net.ipv4_udp_checksum_ok(&bad))\n    let u2 = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let nocsum = net.make_ipv4(\"1.2.3.4\", \"5.6.7.8\", 17, 64, u2)\n    emit(net.ipv4_udp_checksum_ok(&nocsum))\n    let u3 = net.make_udp(5000, 53, bytes_of(\"hello\"))\n    let v6 = net.make_ipv6(\"::1\", \"::2\", 17, 64, u3)\n    emit(net.ipv6_udp_checksum_ok(&v6))\n    return 0\n}\n",
    )
    .expect("runs");
    // fine packet; corrupted payload byte; IPv4 UDP checksum 0 = \"not computed\" is accepted; IPv6 requires one
    assert_eq!(out.output_lines, vec!["true", "false", "true", "false"]);
}

#[test]
fn std_net_ethernet_and_ipv6_headers_round_trip() {
    let out = Compiler::compile_and_run(
        "import std.net\nfn main() -> Int {\n    let u = net.make_udp(1, 2, bytes_of(\"x\"))\n    let p = net.make_ipv6(\"2001:db8:0:0:0:0:0:1\", \"::1\", 17, 33, u)\n    emit(net.ipv6_version(&p))\n    emit(net.ipv6_next_header(&p))\n    emit(net.ipv6_hop_limit(&p))\n    emit(net.ipv6_payload_length(&p))\n    emit(net.ipv6_src(&p))\n    emit(net.ipv6_dst(&p))\n    emit(net.ipv6_valid(&p))\n    emit(len(net.ipv6_payload(&p)))\n    let f = net.make_ethernet(\"AA:bb:cc:dd:ee:0f\", \"00:11:22:33:44:55\", 34525, p)\n    emit(net.eth_dst(&f))\n    emit(net.eth_src(&f))\n    emit(net.eth_type(&f))\n    let inner = net.eth_payload(&f)\n    emit(net.ipv6_hop_limit(&inner))\n    emit(net.ipv6_traffic_class(&inner))\n    emit(net.ipv6_flow_label(&inner))\n    return 0\n}\n",
    )
    .expect("runs");
    assert_eq!(
        out.output_lines,
        vec![
            "6",
            "17",
            "33",
            "9",
            "2001:db8::1",
            "::1",
            "true",
            "9",
            "aa:bb:cc:dd:ee:0f",
            "00:11:22:33:44:55",
            "34525",
            "33",
            "0",
            "0"
        ]
    );
}

#[test]
fn std_net_rejects_malformed_addresses_with_a_clear_failure() {
    for (body, want) in [
        (
            "net.make_ipv6(\"1:2:3\", \"::1\", 17, 64, NetworkBuffer(0))",
            "expected 8 groups",
        ),
        (
            "net.make_ipv6(\"1:2:3:4:5:6:7:8:9\", \"::1\", 17, 64, NetworkBuffer(0))",
            "expected 8 groups",
        ),
        (
            "net.make_ipv6(\"1:2:3:4:5:6:7::8\", \"::1\", 17, 64, NetworkBuffer(0))",
            "at least one group",
        ),
        (
            "net.make_ipv6(\"12345::1\", \"::1\", 17, 64, NetworkBuffer(0))",
            "longer than 4",
        ),
        (
            "net.make_ipv6(\"g::1\", \"::1\", 17, 64, NetworkBuffer(0))",
            "hexadecimal",
        ),
        (
            "net.make_ethernet(\"aa:bb:cc\", \"00:11:22:33:44:55\", 2048, NetworkBuffer(0))",
            "six bytes",
        ),
    ] {
        let src = format!("import std.net\nfn main() -> Int {{\n    let p = {body}\n    emit(len(p))\n    return 0\n}}\n");
        let Err(e) = Compiler::compile_and_run(&src) else {
            panic!("should fail: {body}")
        };
        let all: String = e
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            all.contains("Program failed") && all.contains(want),
            "{all} should mention {want}"
        );
    }
}

#[test]
fn or_patterns_share_one_arm_body() {
    let out = run_main(
        " let s = Shape.Boxed(Inner.Num(2))\n emit(match s { Shape.Circle(_) | Shape.Empty => \"a\", Shape.Boxed(_) => \"b\" })\n emit(match 3 { 1 | 2 | 3 => \"small\", _ => \"big\" })\n emit(match 9 { 1 | 2 | 3 => \"small\", _ => \"big\" })\n emit(match 5 { 4 | 5 if false => \"never\", 5 => \"five\", _ => \"other\" })\n emit(match Shape.Circle(7) { Shape.Circle(r) | Shape.Boxed(Inner.Num(r)) => r + 1, _ => 0 })\n emit(match true { true | false => 1 })\n match 2 {\n 1 | 2 => { emit(\"one-or-two\") }\n _ => { emit(\"other\") }\n }",
    );
    assert_eq!(
        out,
        vec!["b", "small", "big", "five", "8", "1", "one-or-two"]
    );
}

#[test]
fn or_pattern_rules_are_checked() {
    // alternatives must cover for exhaustiveness
    fails_with(
        " emit(match Shape.Empty { Shape.Circle(_) | Shape.Empty => 1 })",
        "not exhaustive",
    );
    // a repeated alternative can never run
    fails_with(" emit(match 1 { 1 | 1 => 1, _ => 2 })", "can never run");
    // an alternative covered by an earlier arm
    fails_with(
        " emit(match 1 { 1 => 1, 2 | 1 => 2, _ => 3 })",
        "can never run",
    );
    // alternatives with different bindings: the body sees an undefined name in one of them
    fails_with(
        " emit(match Shape.Empty { Shape.Circle(r) | Shape.Empty => r, _ => 0 })",
        "must bind the same names",
    );
    // alternatives must bind the same names, even when the body does not use them
    fails_with(
        " emit(match Shape.Empty { Shape.Circle(a) | Shape.Rect(b) => 1, _ => 0 })",
        "must bind the same names",
    );
    // a name bound with different types is an error where the body uses it
    fails_with(
        " emit(match Shape.Empty { Shape.Boxed(x) | Shape.Circle(x) => x + 1, _ => 0 })",
        "Cannot apply binary operator",
    );
}

#[test]
fn or_pattern_warnings_are_reported_once() {
    let src = format!("{MATCH_PRELUDE}fn main() -> Int {{\n match Shape.Empty {{\n Shape.Empty | Shape.Circle(_) => {{ let note = 1 }}\n _ => {{ emit(1) }}\n }}\n return 0\n}}\n");
    let (_, warnings) = Compiler::check_with_warnings(&src).expect("valid");
    assert_eq!(
        warnings
            .iter()
            .filter(|w| w.message.contains("'note'"))
            .count(),
        1,
        "{warnings:?}"
    );
}

#[test]
fn range_patterns_match_inclusive_bounds() {
    let out = run_main(
        " let f = 0\n emit(match 5 { 1..=5 => \"in\", _ => \"out\" })\n emit(match 6 { 1..=5 => \"in\", _ => \"out\" })\n emit(match 1 { 1..=5 => \"in\", _ => \"out\" })\n emit(match -3 { -5..=-1 => \"neg\", _ => \"out\" })\n emit(match 4 { 1..=3 | 4..=6 => \"two\", _ => \"out\" })\n emit(match 9 { 0..=5 => 1, 6..=100 if false => 2, _ => 3 })",
    );
    assert_eq!(out, vec!["in", "out", "in", "neg", "two", "3"]);
}

#[test]
fn range_pattern_rules_are_checked() {
    fails_with(" emit(match 3 { 1..=5 => 1 })", "not exhaustive");
    fails_with(
        " emit(match 3 { 1..=5 => 1, 2..=4 => 2, _ => 0 })",
        "can never run",
    );
    fails_with(
        " emit(match 3 { 1..=5 => 1, 3 => 2, _ => 0 })",
        "can never run",
    );
    fails_with(" emit(match 3 { 5..=1 => 1, _ => 0 })", "is empty");
    fails_with(
        " emit(match true { 1..=2 => 1, _ => 0 })",
        "cannot match a value of type Bool",
    );
    fails_with(" emit(match 3 { 1..= => 1, _ => 0 })", "..=");
}

const STRUCT_PRELUDE: &str =
    "struct Pt {\n x: Int,\n y: Int,\n}\nenum Wrap {\n Hold(Pt),\n Nothing,\n}\n";

fn struct_main(body: &str) -> Result<Vec<String>, String> {
    let src = format!("{STRUCT_PRELUDE}fn main() -> Int {{\n{body}\n return 0\n}}\n");
    Compiler::compile_and_run(&src)
        .map(|r| r.output_lines)
        .map_err(|e| {
            e.iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
                .join(" | ")
        })
}

#[test]
fn struct_patterns_match_and_bind_fields() {
    let out = struct_main(
        " let p = Pt { x: 3, y: 0 }\n emit(match p { Pt { x: 0, .. } => 100, Pt { y: 0, x } => x, _ => 99 })\n emit(match p { Pt { x, y } if x == y => 0 - 1, Pt { x, y } => x + y })\n let w = Wrap.Hold(Pt { x: 1, y: 8 })\n emit(match w { Wrap.Hold(Pt { y, .. }) => y, Wrap.Nothing => 0 })\n emit(match w { Wrap.Hold(Pt { x: 2, .. }) => 1, _ => 2 })",
    )
    .unwrap();
    assert_eq!(out, vec!["3", "3", "8", "2"]);
}

#[test]
fn struct_pattern_rules_are_checked() {
    let err = |b: &str| struct_main(b).unwrap_err();
    assert!(
        err(" let p = Pt { x: 1, y: 2 }\n emit(match p { Pt { z, .. } => z, _ => 0 })")
            .contains("has no field 'z'")
    );
    assert!(err(" emit(match 3 { Pt { x, .. } => x, _ => 0 })")
        .contains("cannot match a value of type Int"));
    assert!(
        err(" let p = Pt { x: 1, y: 2 }\n emit(match p { Pt { x: 0, .. } => 1 })")
            .contains("not exhaustive")
    );
    assert!(err(
        " let p = Pt { x: 1, y: 2 }\n emit(match p { Pt { x, y } => x + y, Pt { x: 0, .. } => 1 })"
    )
    .contains("can never run"));
    assert!(
        err(" let p = Pt { x: 1, y: 2 }\n emit(match p { Pt { x: true, .. } => 1, _ => 0 })")
            .contains("cannot match a value of type Int")
    );
    assert!(
        err(" let p = Pt { x: 1, y: 2 }\n emit(match p { Pt { x, x } => 1, _ => 0 })")
            .contains("listed twice")
    );
}

#[test]
fn a_guard_may_end_in_a_name() {
    let out = run_main(" let k = 4\n emit(match 4 { n if n == k => \"same\", _ => \"diff\" })\n emit(match 5 { n if n == k => \"same\", _ => \"diff\" })");
    assert_eq!(out, vec!["same", "diff"]);
}
