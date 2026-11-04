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
        "expected Array<_> or Text",
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
    err(
        &with_shape("fn main() -> Int {\n match 5 { _ => { emit(1) } }\n return 0\n}"),
        "expected an enum value",
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
