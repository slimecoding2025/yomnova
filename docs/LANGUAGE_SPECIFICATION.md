# YOMNOVA Language Specification (v0.1, experimental)

This document describes what the **current implementation** accepts and does. It is not a design wishlist:
anything not listed as *implemented* is marked *planned*, *parsed but rejected*, or *unsupported*.
Behaviour is checked by the regression suite in `yomnova/crates/yomnova-compiler/tests/regression.rs`.

Status labels: **Implemented** · **Partial** · **Parsed-only** (accepted by the parser, rejected with an
"is not supported yet" error) · **Planned** · **Unsupported**.

## 1. Source files

* UTF-8 text, extension `.yom`.
* Line comments `// …` and block comments `/* … */` (block comments do not nest). Unterminated comments and
  string literals are lexer errors.
* Whitespace is insignificant except inside string literals. Statements are not terminated by semicolons.

## 2. Program structure — Implemented

```yomnova
module name          // optional
const LIMIT: Int = 10
fn helper(a: Int, b: Int) -> Int { return a + b }
fn main() -> Int { emit(helper(1, 2))  return 0 }
```

* A program runs `fn main()`. `main` must take no parameters.
* If there is no `main`, top-level statements are executed in order instead. Mixing the two is an error.
  A file with neither is an error (`no entry point`).
* Functions may be called before they are declared. Duplicate function names are an error.
* Top-level `const` values are evaluated in source order **before** `main` runs and are visible inside every
  function. Consts are immutable.
* The value returned from `main` is printed by the CLI as `=> value` unless it is `Unit`. There is no process
  exit-code mapping.

## 3. Types

| Type | Status | Notes |
|---|---|---|
| `Int` | Implemented | 64-bit signed. Overflow, `MIN / -1` and `-MIN` are runtime errors, never wraparound or panic. |
| `Float` | Implemented | 64-bit IEEE. No mixing with `Int`. Division by `0.0` is an error. |
| `Bool` | Implemented | Conditions and `&&`/`||`/`!` require `Bool`; there is no truthiness. |
| `Text` | Implemented | UTF-8, `+` concatenates (16 MiB cap). Escapes: `\n \t \r \\ \" \0`. May span lines. |
| `Unit` | Implemented | The "no value" type; printed as `()`. |
| `Array<T>` | Partial | Literal `[a, b]`, indexing `xs[i]` (bounds-checked, negative indices are errors). No push, length, slicing or iteration. All elements must have one type. |
| `IPv4` | Implemented | `IPv4("a.b.c.d")`; strictly validated, leading zeros rejected. |
| `Port` | Implemented | `Port(n)`, `1 ≤ n ≤ 65535`. |
| `Endpoint` | Implemented | `Endpoint(ip, port)`; the port may be a `Port` or an `Int` (range-checked at runtime). |
| `IPv6`, `TCP`, `UDP`, `Packet` | Planned | Names are reserved and parse as types; there is no constructor, value or behaviour. |
| `struct`, `enum` | Parsed-only | Declarations are rejected: `struct 'P' is not supported yet`. |
| Generics, traits, lambdas, methods, field access | Unsupported | Rejected with an "is not supported yet" error. |

Type inference is local: `let x = expr` takes the type of `expr`. Parameters and return types must be annotated.
A function with a non-`Unit` return type must return on every path (`MissingReturn`).

## 4. Statements

| Form | Status |
|---|---|
| `let name[: T] = expr` (immutable) | Implemented |
| `mut name[: T] = expr` (mutable) | Implemented |
| `const name[: T] = expr` inside a function | Implemented (an immutable binding) |
| `name = expr` | Implemented (only identifiers are assignable; the target must be `mut`) |
| `if cond { } else if cond { } else { }` | Implemented |
| `while cond { }` | Implemented |
| `return [expr]` | Implemented |
| `flow name = source |> stage …` | Implemented (see §6) |
| expression statement | Implemented |
| `for … in …`, `match`, `import`, `type` | Planned — the words are reserved, but there is no syntax for them |

Blocks introduce lexical scopes. A name can be shadowed in an inner block but not redeclared in the same scope.

## 5. Expressions

Precedence, lowest to highest: `|>` · `||` · `&&` · `== !=` · `< <= > >=` · `+ -` · `* / %` · unary `- !` ·
call `f(x)` and index `a[i]`.

* `&&` and `||` **short-circuit**.
* `==`/`!=` require both sides to have the same type.
* Ordering comparisons require two `Int`s or two `Float`s.
* `emit(x)` is a built-in that prints `x` followed by a newline. It *borrows* `x` (it does not move it) and
  takes exactly one argument.
* Calls check the argument count and every argument type at compile time.

## 6. Pipelines and `flow` — Partial

```yomnova
fn inc(n: Int) -> Int { return n + 1 }
fn scale(n: Int, k: Int) -> Int { return n * k }

flow r = 5 |> inc |> scale(10)   // scale(inc(5), 10) == 60
5 |> inc |> emit                 // prints 6
```

* `x |> f` is `f(x)`; `x |> f(a)` is `f(x, a)`. Stages are applied strictly left to right.
* A stage must name a declared function (or `emit`) and is type-checked like an ordinary call.
* This is sequential function application. There is **no** parallelism, back-pressure or streaming.

## 7. Ownership — Partial

Ownership is checked statically after type checking; violations stop the program before it runs.

* Copy types: `Int`, `Float`, `Bool`, `Unit`. Everything else is *moved* by: passing it to a function (other than
  `emit`), `let b = a`, assigning it, putting it in an array literal, returning it, or using it as a pipeline
  source/argument.
* Using a moved value is `Use of moved value 'x'`.
* A value moved in only one branch of an `if` is *maybe moved* afterwards and is rejected on later use.
* Moving a value declared outside a `while` loop from inside the loop is rejected, unless it is reassigned.
* **Not implemented:** references/borrows, mutable borrows, lifetimes, resource types, typestate.

## 8. Runtime and limits

A program is compiled to bytecode and run on a single-threaded stack VM.

| Limit | Value |
|---|---|
| Executed instructions | 1,000,000 (`Execution step limit exceeded`) |
| Value stack | 65,536 slots |
| Call depth | 10,000 frames |
| Text length | 16 MiB |
| Source nesting depth (parser) | 150 |
| Call arguments / array literal length | 255 / 65,535 |
| Constants per function / function size (jump targets) | 65,536 / 65,535 bytes |

The language has no file, process, network or time access.

## 9. Diagnostics

Every diagnostic carries `file:line:col`, the source line and a caret. A run reports several errors where it can:
the parser resumes at the next statement or declaration, and semantic analysis and the ownership check continue after
a failing statement or function (at most 20 errors per stage). A name whose declaration failed does not produce
follow-on "undefined" errors. Stages are still sequential: semantic analysis runs only if parsing succeeded, and the
ownership check only if semantic analysis succeeded, so fixing the reported errors can reveal errors from the next
stage. Lexical errors are still reported one at a time. Runtime failures are reported as `Runtime error: …` and exit
with code 1. There are no warnings yet.

## 10. Formatting

`yomnova fmt` re-indents by brace depth (4 spaces), trims trailing whitespace, collapses blank-line runs and
normalises line endings. It never edits inside strings or block comments and is idempotent. It does not wrap
lines or reorder anything.
