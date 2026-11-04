# YOMNOVA Grammar (as implemented)

EBNF for what `yomnova-parser` accepts. Productions marked **†** parse but are rejected by semantic analysis
as unsupported. Lexical details are in `crates/yomnova-lexer/src/lib.rs`.

```ebnf
program      = [ "module" IDENT ] { item } ;
item         = function | const_decl | struct_decl | enum_decl † | statement ;

function     = "fn" IDENT "(" [ param { "," param } ] ")" [ "->" type ] [ effects ] block ;
param        = [ "mut" ] IDENT ":" type ;
effects      = "![" IDENT { "." IDENT } { "," IDENT { "." IDENT } } "]" ;   (* parsed, ignored *)
const_decl   = "const" IDENT [ ":" type ] "=" expr ;
struct_decl  = "struct" IDENT "{" field { "," field } [ "," ] "}" ;
enum_decl    = "enum" IDENT "{" variant { "," variant } [ "," ] "}" ;

block        = "{" { statement } "}" ;
statement    = "let" IDENT [ ":" type ] "=" expr
             | "mut" IDENT [ ":" type ] "=" expr
             | const_decl
             | IDENT "=" expr
             | "if" expr block [ "else" ( block | if_stmt ) ]
             | "while" expr block
             | "for" IDENT "in" expr block
             | "return" [ expr ]
             | "flow" IDENT "=" pipeline
             | expr ;
pipeline     = expr { "|>" stage } ;
stage        = ( IDENT | "emit" ) [ "(" [ expr ] ")" ] ;

expr         = or_expr { "|>" stage } ;
or_expr      = and_expr { "||" and_expr } ;
and_expr     = eq_expr  { "&&" eq_expr } ;
eq_expr      = cmp_expr { ( "==" | "!=" ) cmp_expr } ;
cmp_expr     = add_expr { ( "<" | "<=" | ">" | ">=" ) add_expr } ;
add_expr     = mul_expr { ( "+" | "-" ) mul_expr } ;
mul_expr     = unary    { ( "*" | "/" | "%" ) unary } ;
unary        = ( "-" | "!" ) unary | postfix ;
postfix      = primary { "(" [ args ] ")" | "[" expr "]" | "." IDENT [ "(" [ args ]  ")" ] † } ;
primary      = INT | FLOAT | STRING | "true" | "false" | "(" expr ")" | "[" [ args ] "]" | IDENT | struct_lit
             | "emit" | "IPv4" | "Port" | "Endpoint" ;
args         = expr { "," expr } ;
struct_lit   = UPPER_IDENT "{" IDENT ":" expr { "," IDENT ":" expr } [ "," ] "}" ;   (* only when an uppercase name is followed by `{ ident :` *)

type         = "Int" | "Float" | "Bool" | "Text" | "Unit"
             | "Array" "<" type ">"
             | "IPv4" | "IPv6" | "Port" | "Endpoint" | "TCP" | "UDP" | "Packet"
             | IDENT ;                                                       (* user type names; no declarations work yet *)
```

## Lexical rules

* `IDENT`: `[A-Za-z_][A-Za-z0-9_]*`. `INT`: decimal digits (must fit in `i64`). `FLOAT`: digits `.` digits.
* `STRING`: `"…"` with escapes `\n \t \r \\ \" \0`; may contain newlines.
* Reserved words: `module fn return let mut const if else while for in flow struct enum match emit import type`,
  `true false`, and the network names `IPv4 IPv6 Port Endpoint TCP UDP Packet`. `for in match import type` are
  reserved but have no grammar yet.
* Operators and punctuation: `+ - * / % = == != < <= > >= && || ! |> -> ![ . , : ( ) { } [ ]`.

## Limits

Syntactic nesting (blocks, parenthesised/unary expressions, array types) is limited to 150 levels; deeper input
is a `Nesting too deep` parse error rather than a crash.
