# YOMNOVA Grammar (EBNF)

```ebnf
Program         ::= ( ModuleDecl )? ( TopLevelItem )* EOF ;

ModuleDecl      ::= "module" Identifier ;

TopLevelItem    ::= FunctionDecl
                  | StructDecl
                  | EnumDecl
                  | ConstDecl
                  | Statement ;

FunctionDecl    ::= "fn" Identifier "(" ParamList? ")" ( "->" Type )? ( "![" EffectList "]" )? Block ;

ParamList       ::= Param ( "," Param )* ;
Param           ::= ( "mut" )? Identifier ":" Type ;

EffectList      ::= Identifier ( "." Identifier )* ( "," Identifier ( "." Identifier )* )* ;

Block           ::= "{" Statement* "}" ;

Statement       ::= LetStmt
                  | MutStmt
                  | ConstDecl
                  | AssignStmt
                  | IfStmt
                  | WhileStmt
                  | ReturnStmt
                  | FlowStmt
                  | ExprStmt ;

LetStmt         ::= "let" Identifier ( ":" Type )? "=" Expr ;
MutStmt         ::= "mut" Identifier ( ":" Type )? "=" Expr ;
ConstDecl       ::= "const" Identifier ( ":" Type )? "=" Expr ;
AssignStmt      ::= Identifier "=" Expr ;

IfStmt          ::= "if" Expr Block ( "else" ( IfStmt | Block ) )? ;
WhileStmt       ::= "while" Expr Block ;
ReturnStmt      ::= "return" Expr? ;

FlowStmt        ::= "flow" Identifier "=" Expr ( "|>" Expr )+ ;

ExprStmt        ::= Expr ;

Expr            ::= Assignment ;
Assignment      ::= LogicalOr ( "|>" PipeStage )* ;
LogicalOr       ::= LogicalAnd ( "||" LogicalAnd )* ;
LogicalAnd      ::= Equality ( "&&" Equality )* ;
Equality        ::= Comparison ( ( "==" | "!=" ) Comparison )* ;
Comparison      ::= Term ( ( "<" | "<=" | ">" | ">=" ) Term )* ;
Term            ::= Factor ( ( "+" | "-" ) Factor )* ;
Factor          ::= Unary ( ( "*" | "/" | "%" ) Unary )* ;
Unary           ::= ( "-" | "!" ) Unary | Postfix ;
Postfix         ::= Primary ( "(" ArgList? ")" | "[" Expr "]" | "." Identifier )* ;

Primary         ::= IntegerLiteral
                  | FloatLiteral
                  | StringLiteral
                  | "true"
                  | "false"
                  | "()"
                  | Identifier
                  | "[" ( Expr ( "," Expr )* )? "]"
                  | "(" Expr ")" ;

ArgList         ::= Expr ( "," Expr )* ;

Type            ::= "Int" | "Float" | "Bool" | "Text" | "Unit"
                  | "IPv4" | "IPv6" | "Port" | "Endpoint" | "Packet" | "TCP" | "UDP"
                  | "Array" "<" Type ">"
                  | Identifier ;
```
