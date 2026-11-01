//! Bytecode instruction set, chunks, and disassembler for YOMNOVA.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    Constant,     // [u16: index]
    Nil,
    True,
    False,
    Pop,
    GetLocal,     // [u16: slot]
    SetLocal,     // [u16: slot]
    GetGlobal,    // [u16: name_idx]
    SetGlobal,    // [u16: name_idx]
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    And,
    Or,
    Not,
    Negate,
    Jump,         // [u16: offset]
    JumpIfFalse,  // [u16: offset]
    Call,         // [u8: arg_count]
    Return,
    Emit,
    Array,        // [u16: length]
    Index,
    Halt,
}

impl OpCode {
    /// Every opcode, in discriminant order. `ALL[op as usize] == op`.
    pub const ALL: [OpCode; 32] = [
        OpCode::Constant,
        OpCode::Nil,
        OpCode::True,
        OpCode::False,
        OpCode::Pop,
        OpCode::GetLocal,
        OpCode::SetLocal,
        OpCode::GetGlobal,
        OpCode::SetGlobal,
        OpCode::Equal,
        OpCode::NotEqual,
        OpCode::Greater,
        OpCode::GreaterEqual,
        OpCode::Less,
        OpCode::LessEqual,
        OpCode::Add,
        OpCode::Subtract,
        OpCode::Multiply,
        OpCode::Divide,
        OpCode::Modulo,
        OpCode::And,
        OpCode::Or,
        OpCode::Not,
        OpCode::Negate,
        OpCode::Jump,
        OpCode::JumpIfFalse,
        OpCode::Call,
        OpCode::Return,
        OpCode::Emit,
        OpCode::Array,
        OpCode::Index,
        OpCode::Halt,
    ];

    /// Safe byte -> opcode decoding (no `unsafe`).
    pub fn from_u8(byte: u8) -> Option<OpCode> {
        Self::ALL.get(byte as usize).copied()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Unit,
    Array(Vec<Value>),
    IPv4([u8; 4]),
    IPv6([u8; 16]),
    Port(u16),
    Endpoint([u8; 4], u16),
    Packet { header: Vec<u8>, payload: Vec<u8> },
    NativeFunction(String),
    UserFunction { name: String, chunk_index: usize, arity: usize },
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Unit => false,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Text(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            _ => true,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Text(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Unit => write!(f, "()"),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::IPv4(bytes) => write!(f, "{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]),
            Value::IPv6(bytes) => write!(f, "IPv6({:?})", bytes),
            Value::Port(p) => write!(f, "{p}"),
            Value::Endpoint(ip, p) => write!(f, "{}.{}.{}.{}:{}", ip[0], ip[1], ip[2], ip[3], p),
            Value::Packet { payload, .. } => write!(f, "Packet(len={})", payload.len()),
            Value::NativeFunction(name) => write!(f, "<native fn {name}>"),
            Value::UserFunction { name, .. } => write!(f, "<fn {name}>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {

    pub fn write_byte(&mut self, byte: u8, line: usize) {
        self.code.push(byte);
        self.lines.push(line);
    }

    pub fn write_op(&mut self, op: OpCode, line: usize) {
        self.write_byte(op as u8, line);
    }

    pub fn write_u16(&mut self, val: u16, line: usize) {
        self.write_byte((val >> 8) as u8, line);
        self.write_byte((val & 0xFF) as u8, line);
    }

    /// Add a constant, or return `None` if the 16-bit constant index space is full.
    pub fn try_add_constant(&mut self, value: Value) -> Option<u16> {
        let idx = u16::try_from(self.constants.len()).ok()?;
        self.constants.push(value);
        Some(idx)
    }

    pub fn add_constant(&mut self, value: Value) -> u16 {
        self.constants.push(value);
        (self.constants.len() - 1) as u16
    }

    pub fn disassemble(&self, name: &str) -> String {
        let mut out = format!("== {name} ==\n");
        let mut offset = 0;
        while offset < self.code.len() {
            let (desc, next_offset) = self.disassemble_instruction(offset);
            out.push_str(&desc);
            out.push('\n');
            offset = next_offset;
        }
        out
    }

    pub fn disassemble_instruction(&self, offset: usize) -> (String, usize) {
        let mut out = format!("{:04} ", offset);
        if offset > 0 && self.lines[offset] == self.lines[offset - 1] {
            out.push_str("   | ");
        } else {
            out.push_str(&format!("{:4} ", self.lines[offset]));
        }

        let op_byte = self.code[offset];
        let Some(op) = OpCode::from_u8(op_byte) else {
            return (format!("{out}Unknown opcode {op_byte}"), offset + 1);
        };
        match op {
            OpCode::Constant => {
                let const_idx = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                let val = &self.constants[const_idx as usize];
                (format!("{out}OP_CONSTANT {const_idx} ('{val}')"), offset + 3)
            }
            OpCode::GetLocal => {
                let slot = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                (format!("{out}OP_GET_LOCAL {slot}"), offset + 3)
            }
            OpCode::SetLocal => {
                let slot = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                (format!("{out}OP_SET_LOCAL {slot}"), offset + 3)
            }
            OpCode::GetGlobal => {
                let idx = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                let name = &self.constants[idx as usize];
                (format!("{out}OP_GET_GLOBAL {idx} ('{name}')"), offset + 3)
            }
            OpCode::SetGlobal => {
                let idx = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                let name = &self.constants[idx as usize];
                (format!("{out}OP_SET_GLOBAL {idx} ('{name}')"), offset + 3)
            }
            OpCode::Jump => {
                let target = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                (format!("{out}OP_JUMP -> {target}"), offset + 3)
            }
            OpCode::JumpIfFalse => {
                let target = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                (format!("{out}OP_JUMP_IF_FALSE -> {target}"), offset + 3)
            }
            OpCode::Call => {
                let count = self.code[offset + 1];
                (format!("{out}OP_CALL ({count} args)"), offset + 2)
            }
            OpCode::Array => {
                let len = ((self.code[offset + 1] as u16) << 8) | (self.code[offset + 2] as u16);
                (format!("{out}OP_ARRAY (len={len})"), offset + 3)
            }
            OpCode::Nil => (format!("{out}OP_NIL"), offset + 1),
            OpCode::True => (format!("{out}OP_TRUE"), offset + 1),
            OpCode::False => (format!("{out}OP_FALSE"), offset + 1),
            OpCode::Pop => (format!("{out}OP_POP"), offset + 1),
            OpCode::Equal => (format!("{out}OP_EQUAL"), offset + 1),
            OpCode::NotEqual => (format!("{out}OP_NOT_EQUAL"), offset + 1),
            OpCode::Greater => (format!("{out}OP_GREATER"), offset + 1),
            OpCode::GreaterEqual => (format!("{out}OP_GREATER_EQUAL"), offset + 1),
            OpCode::Less => (format!("{out}OP_LESS"), offset + 1),
            OpCode::LessEqual => (format!("{out}OP_LESS_EQUAL"), offset + 1),
            OpCode::Add => (format!("{out}OP_ADD"), offset + 1),
            OpCode::Subtract => (format!("{out}OP_SUBTRACT"), offset + 1),
            OpCode::Multiply => (format!("{out}OP_MULTIPLY"), offset + 1),
            OpCode::Divide => (format!("{out}OP_DIVIDE"), offset + 1),
            OpCode::Modulo => (format!("{out}OP_MODULO"), offset + 1),
            OpCode::And => (format!("{out}OP_AND"), offset + 1),
            OpCode::Or => (format!("{out}OP_OR"), offset + 1),
            OpCode::Not => (format!("{out}OP_NOT"), offset + 1),
            OpCode::Negate => (format!("{out}OP_NEGATE"), offset + 1),
            OpCode::Emit => (format!("{out}OP_EMIT"), offset + 1),
            OpCode::Index => (format!("{out}OP_INDEX"), offset + 1),
            OpCode::Return => (format!("{out}OP_RETURN"), offset + 1),
            OpCode::Halt => (format!("{out}OP_HALT"), offset + 1),
        }
    }
}

#[cfg(test)]
mod opcode_tests {
    use super::*;

    #[test]
    fn all_table_matches_discriminants() {
        for (i, op) in OpCode::ALL.iter().enumerate() {
            assert_eq!(*op as usize, i);
            assert_eq!(OpCode::from_u8(i as u8), Some(*op));
        }
        assert_eq!(OpCode::ALL[OpCode::ALL.len() - 1], OpCode::Halt);
        assert_eq!(OpCode::from_u8(OpCode::ALL.len() as u8), None);
        assert_eq!(OpCode::from_u8(255), None);
    }
}
