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
| `IPv6` | Implemented | `IPv6("::1")` validates with `std::net::Ipv6Addr`; displays canonically; `==` works. |
| `NetworkBuffer` | Implemented | `NetworkBuffer(n)` (zeroed, max 1 MiB), `bytes_of(text)`, `buf_get`/`buf_get16`/`buf_get32` (big-endian, unsigned), `buf_set`/`buf_set16`/`buf_set32` (value must fit the width), `buf_slice(b, start, len)`, `buf_concat(a, b)`, `len(b)`. Every access is bounds-checked (runtime error); writes return a new buffer, so a buffer passed to a write is moved. Reads borrow. |
| `Packet` | Implemented | `Packet(header, payload)` of two `NetworkBuffer`s, `packet_header(p)`, `packet_payload(p)`. A container only: no protocol parsing or checksums. |
| `TCP`, `UDP` | Planned | Names are reserved and parse as types; there is no constructor, value or behaviour. |
| `struct` | Implemented | `struct P { x: Int, y: Int }`, literals `P { x: 1, y: 2 }` (any field order, every field exactly once), field reads `p.x`, structs as parameters, return values, array elements and fields of other structs. Display: `P { x: 1, y: 2 }`; equality is by value. Field assignment `p.x = v` (and `a.b.c = v`) requires a `mut` variable. Not supported: methods, generics, recursive structs (rejected: infinite size). |
| `enum` | Implemented | `enum Shape { Circle(Int), Empty }` — each variant carries at most one value. Values are built as `Shape.Circle(2)` / `Shape.Empty`, displayed the same way and compared by value. Empty enums, duplicate variants and directly recursive payloads are rejected. |
| Generics, traits, lambdas, methods, field access | Unsupported | Rejected with an "is not supported yet" error. |

Type inference is local: `let x = expr` takes the type of `expr`. Parameters and return types must be annotated.
A function with a non-`Unit` return type must return on every path (`MissingReturn`).

## 4. Statements

| Form | Status |
|---|---|
| `let name[: T] = expr` (immutable) | Implemented |
| `mut name[: T] = expr` (mutable) | Implemented |
| `const name[: T] = expr` inside a function | Implemented (an immutable binding) |
| `name = expr` | Implemented (the target must be `mut`) |
| `xs[i] = expr`, `p.f = expr`, `a[i].f[j] = expr` | Implemented — the root variable must be `mut`; the value type must match; arrays and structs have value semantics, so the container is copied (O(n) per write). In a nested target (more than one step) an index may not contain a call, so it is evaluated once. Out-of-range indices are runtime errors. |
| `if cond { } else if cond { } else { }` | Implemented |
| `while cond { }` | Implemented |
| `for name in array { }` | Implemented — iterates the elements of an `Array<T>`; `name` is immutable and scoped to the loop |
| `return [expr]` | Implemented |
| `flow name = source |> stage …` | Implemented (see §6) |
| expression statement | Implemented |
| `match value { Enum.Variant(x) => { … } … }` | Implemented, as a statement (not an expression). Patterns are `Enum.Variant`, `Enum.Variant(name)`, `Enum.Variant(_)` and `_`. The match must be exhaustive (every variant, or a `_` arm), may not repeat a variant, and arms after `_` are rejected. A function whose arms all `return` needs no `return` after the `match`. No nested patterns, guards or literal patterns. |
| `import`, `type` | Planned — the words are reserved, but there is no syntax for them |

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
* Moving a value declared outside a `while` or `for` loop from inside the loop is rejected, unless it is reassigned.
* A struct or enum value is a moved (non-copy) value; `match` only borrows it, and a value moved inside one arm is "maybe moved" afterwards. A struct is a moved value; reading a field (`p.x`) does not move it. `for x in xs` only borrows `xs` (it can be used afterwards); `push` consumes its array argument and `len` only borrows.
* **Borrowing (implemented, deliberately small).** A parameter may be `&T` (read-only) or `&mut T` (read and write). The
  caller passes `&x` / `&mut x`, which is only legal as a call argument and only for a plain variable (`&mut x` needs `x`
  to be `mut`). Because references cannot be stored, returned, put in a `let`, a struct or an array, a borrow can never
  outlive its call, so **there are no lifetime annotations**. The checker rejects: `&mut x` together with any other use of
  `x` in the same call, `&x` together with a by-value move of `x` or a `&mut x`, borrowing a moved variable, moving a
  non-copy value out of a reference parameter, and writing through `&T`. Inside the callee a reference parameter is used
  like the value itself (`xs[i]`, `len(xs)`, `n + 1`); a `&mut` parameter can also be assigned (`n = 0`, `xs[i] = v`).
  Implementation: `&mut` is **copy-in, copy-out**: the callee returns its final value and the caller writes it back,
  which is equivalent to a real reference only because `&mut` is exclusive and cannot be stored. Cost: O(size) per call
  like any by-value argument. Pipeline stages and `flow` cannot take reference parameters.
* **Imports and standard library.** `import std.math` and `import std.arrays` at the start of a line; functions are always
  qualified: `math.abs math.sign math.min math.max math.clamp math.pow`, `arrays.sum arrays.contains arrays.index_of
  arrays.max_of arrays.min_of` (arrays are `Array<Int>`, passed as `&xs`). Implemented as source expansion before lexing;
  there are no user-defined modules, no file imports and no bare names. An unknown module or function is a compile error.
* **File imports (CLI only).** `import "lib.yom" as lib` inlines the file; use `lib.area(...)`, `lib.Rect { ... }`. Only items marked
  `pub` (`pub fn`, `pub struct`, `pub enum`) are visible; a private or missing item is an error. Functions, structs and enums are
  renamed with the alias, so names cannot collide. Paths are relative to the importing file; cycles, missing files and a library
  `main` are errors; imports are not re-exported. It is source renaming (types print as `Lib_Rect`), not separate compilation.
* **Not implemented:** stored or returned references, borrowing fields or elements (`&p.x`), reborrow rules beyond the call,
  lifetime parameters, resource types, typestate.

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
with code 1. Warnings are printed to stderr as `file:line:col: warning: …` before the program runs and never change the exit code. There are two: a `let`/`mut`/`flow`/`for`/`match` binding that is never read (assignments do not count; names starting with `_` and function parameters are exempt), and a statement that follows `return` in the same block. Warnings are only produced for a program that has no errors.

## 10. Formatting

`yomnova fmt` re-indents by brace depth (4 spaces), trims trailing whitespace, collapses blank-line runs and
normalises line endings. It never edits inside strings or block comments and is idempotent. It does not wrap
lines or reorder anything.
