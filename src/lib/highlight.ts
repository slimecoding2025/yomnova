/** Tiny YOMNOVA highlighter. Keywords mirror crates/yomnova-lexer/src/lib.rs. */
const KEYWORDS = new Set([
  "module", "fn", "return", "let", "mut", "const", "if", "else", "while", "for", "in", "flow",
  "struct", "enum", "match", "emit", "import", "type",
]);
const LITERALS = new Set(["true", "false"]);
const TYPES = new Set([
  "Int", "Float", "Bool", "Text", "Unit", "Array", "IPv4", "IPv6", "Port", "Endpoint", "TCP", "UDP", "Packet",
]);

export type TokenClass = "kw" | "lit" | "ty" | "num" | "str" | "cm" | "op" | "fn" | "id" | "ws";

export interface Piece {
  text: string;
  cls: TokenClass;
}

const OPERATORS = /^(\|>|->|==|!=|<=|>=|&&|\|\||[+\-*/%=<>!:,.;(){}[\]])/;

/** Split source into classified pieces. Concatenating `text` reproduces the input exactly. */
export function highlight(src: string): Piece[] {
  const out: Piece[] = [];
  let i = 0;
  const push = (text: string, cls: TokenClass) => out.push({ text, cls });
  while (i < src.length) {
    const rest = src.slice(i);
    let m: RegExpMatchArray | null;
    if ((m = rest.match(/^\s+/))) {
      push(m[0], "ws");
    } else if ((m = rest.match(/^\/\/[^\n]*/))) {
      push(m[0], "cm");
    } else if ((m = rest.match(/^\/\*[\s\S]*?(\*\/|$)/))) {
      push(m[0], "cm");
    } else if ((m = rest.match(/^"(?:\\.|[^"\\])*("|$)/))) {
      push(m[0], "str");
    } else if ((m = rest.match(/^\d+(\.\d+)?/))) {
      push(m[0], "num");
    } else if ((m = rest.match(/^[A-Za-z_][A-Za-z0-9_]*/))) {
      const w = m[0];
      const after = src.slice(i + w.length).match(/^\s*\(/);
      if (KEYWORDS.has(w)) push(w, "kw");
      else if (LITERALS.has(w)) push(w, "lit");
      else if (TYPES.has(w)) push(w, "ty");
      else if (after) push(w, "fn");
      else push(w, "id");
    } else if ((m = rest.match(OPERATORS))) {
      push(m[0], "op");
    } else {
      m = [rest[0]] as unknown as RegExpMatchArray;
      push(rest[0], "id");
    }
    i += m[0].length;
  }
  return out;
}
