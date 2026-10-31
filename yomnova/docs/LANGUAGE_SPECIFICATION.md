# YOMNOVA Language Specification
**Version 0.1.0-dev**
**Author:** Mouhamed Salim Bousmina

## 1. Lexical Structure

### 1.1 Source Text
YOMNOVA source code is UTF-8 encoded text. Files use the `.yom` extension.

### 1.2 Comments
- Single-line comments start with `//` and extend to end of line.
- Multi-line comments are enclosed between `/*` and `*/`.

### 1.3 Identifiers
Identifiers start with `[a-zA-Z_]` followed by any sequence of `[a-zA-Z0-9_]`.
Keywords are reserved and cannot be used as identifiers.

### 1.4 Keywords
`module`, `fn`, `return`, `let`, `mut`, `const`, `if`, `else`, `while`, `for`, `in`,
`flow`, `struct`, `enum`, `match`, `true`, `false`, `emit`, `import`, `type`,
`IPv4`, `IPv6`, `Port`, `Endpoint`, `TCP`, `UDP`, `Packet`

### 1.5 Literals
- **Integer**: Decimal integers such as `0`, `42`, `-10`, `65535`.
- **Float**: Decimal floating-point numbers such as `3.1415`, `0.5`, `-12.8`.
- **Text**: Double-quoted string literals `"Hello YOMNOVA"`, supporting escape sequences `\n`, `\t`, `\"`, `\\`.
- **Bool**: `true` and `false`.
- **Unit**: Represented as empty tuple `()` or `Unit`.

---

## 2. Type System

### 2.1 Primitive Types
- `Int`: 64-bit signed integer.
- `Float`: 64-bit IEEE 754 floating-point.
- `Bool`: Boolean truth value.
- `Text`: UTF-8 immutable heap-allocated text string.
- `Unit`: The empty type `()`.

### 2.2 Network Types
- `IPv4`: Validated 4-byte IPv4 address (e.g. `127.0.0.1`).
- `IPv6`: Validated 16-byte IPv6 address.
- `Port`: Validated 16-bit integer port number (`1..=65535`).
- `Endpoint`: Pair of `(IPAddress, Port)`.
- `Packet`: Network packet with validated header and payload buffer.

### 2.3 Compound Types
- `Array<T>`: Homogeneous sequential collection.
- `(T1, T2, ...)`: Product tuple types.
- `Struct`: Named records with typed fields.
- `Enum`: Tagged unions with optional data variants.

---

## 3. Statements & Declarations

### 3.1 Module Declaration
```yom
module genesis
```

### 3.2 Variable Binding
- Immutable: `let x: Int = 10`
- Mutable: `mut y: Text = "Initial"`
- Constant: `const MAX: Int = 100`

### 3.3 Functions
```yom
fn name(param1: Type1, param2: Type2) -> ReturnType {
    // Body
    return value
}
```

### 3.4 Control Flow
```yom
if condition {
    // Then branch
} else {
    // Else branch
}

while condition {
    // Loop body
}
```

### 3.5 Builtin Emit
`emit(expression)` outputs the value to standard output (or virtual host buffer).

---

## 4. Ownership & Move Semantics

1. Every value has a single owner at any point in time.
2. Passing a non-copy value (like `Text`, `Array`, `Packet`, `Connection`) into a function transfers ownership (Move).
3. Attempting to read a moved variable results in a compile-time ownership error:
   `CompileError: Use of moved value 'message' at line X, col Y`.
4. Variable reassignment requires mutable declaration (`mut`).
