//! Stack-based Virtual Machine and Runtime for YOMNOVA.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use yomnova_bytecode::{Chunk, OpCode, Value};

mod sec;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VmError {
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Stack overflow")]
    StackOverflow,
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Undefined global variable '{0}'")]
    UndefinedGlobal(String),
    #[error("Instruction pointer out of bounds: {ip}")]
    InvalidIp { ip: usize },
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Execution step limit exceeded ({limit} steps)")]
    StepLimitExceeded { limit: usize },
    #[error("Network protocol error: {0}")]
    ProtocolError(String),
    #[error("Integer overflow in {0}")]
    IntegerOverflow(String),
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
    #[error("Program failed: {0}")]
    Failed(String),
}

pub struct CallFrame {
    pub chunk_index: usize,
    pub ip: usize,
    pub locals: Vec<Value>,
}

pub struct Vm {
    pub chunks: Vec<Chunk>,
    pub stack: Vec<Value>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, Value>,
    pub output: Vec<String>,
    pub max_steps: usize,
    pub step_count: usize,
    /// Directory the `file_*` natives may touch. `None` (the default) disables file access entirely.
    pub fs_root: Option<std::path::PathBuf>,
}

impl Vm {
    pub const DEFAULT_MAX_STEPS: usize = 1_000_000;
    pub const MAX_STACK_SIZE: usize = 65_536;
    pub const MAX_CALL_DEPTH: usize = 10_000;
    pub const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
    pub const MAX_ARRAY_LEN: usize = 1_000_000;
    /// Local-variable slots alive across all call frames at once.
    pub const MAX_TOTAL_LOCALS: usize = 1_000_000;

    pub fn new() -> Self {
        let mut vm = Self {
            chunks: Vec::new(),
            stack: Vec::with_capacity(256),
            frames: Vec::new(),
            globals: HashMap::new(),
            output: Vec::new(),
            max_steps: Self::DEFAULT_MAX_STEPS,
            step_count: 0,
            fs_root: None,
        };

        // Register builtin functions
        vm.globals.insert(
            "IPv4".to_string(),
            Value::NativeFunction("IPv4".to_string()),
        );
        vm.globals.insert(
            "Port".to_string(),
            Value::NativeFunction("Port".to_string()),
        );
        vm.globals.insert(
            "Endpoint".to_string(),
            Value::NativeFunction("Endpoint".to_string()),
        );
        for name in [
            "IPv6",
            "len",
            "push",
            "NetworkBuffer",
            "bytes_of",
            "buf_get",
            "buf_get16",
            "buf_get32",
            "buf_set",
            "buf_set16",
            "buf_set32",
            "buf_slice",
            "buf_concat",
            "Packet",
            "packet_header",
            "packet_payload",
            "str_upper",
            "str_lower",
            "str_trim",
            "str_contains",
            "str_starts_with",
            "str_ends_with",
            "str_find",
            "str_slice",
            "str_repeat",
            "str_replace",
            "str_split",
            "str_from_int",
            "str_parse_int",
            "fail",
            "file_read",
            "file_write",
            "file_append",
            "file_exists",
            "sha256",
            "hmac_sha256",
            "hex_encode",
            "hex_decode",
            "base64_encode",
            "base64_decode",
            "buf_find",
            "buf_entropy",
            "buf_find_wild",
            "file_read_bytes",
        ] {
            vm.globals
                .insert(name.to_string(), Value::NativeFunction(name.to_string()));
        }

        vm
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn run_chunk(&mut self, chunk: Chunk) -> Result<Value, VmError> {
        let chunk_idx = self.chunks.len();
        self.chunks.push(chunk);

        self.frames.push(CallFrame {
            chunk_index: chunk_idx,
            ip: 0,
            locals: Vec::new(),
        });

        self.execute()
    }

    fn execute(&mut self) -> Result<Value, VmError> {
        while !self.frames.is_empty() {
            self.step_count += 1;
            if self.step_count > self.max_steps {
                return Err(VmError::StepLimitExceeded {
                    limit: self.max_steps,
                });
            }

            let frame = self.frames.last_mut().unwrap();
            let chunk = &self.chunks[frame.chunk_index];

            if frame.ip >= chunk.code.len() {
                break;
            }

            let op_byte = chunk.code[frame.ip];
            frame.ip += 1;

            let Some(op) = OpCode::from_u8(op_byte) else {
                return Err(VmError::InvalidIp { ip: frame.ip });
            };

            match op {
                OpCode::Constant => {
                    let idx = self.read_u16()? as usize;
                    let val = self.constant_at(idx).ok_or_else(|| {
                        VmError::TypeError(format!("constant #{idx} does not exist"))
                    })?;
                    self.push(val)?;
                }
                OpCode::Nil => self.push(Value::Unit)?,
                OpCode::True => self.push(Value::Bool(true))?,
                OpCode::False => self.push(Value::Bool(false))?,
                OpCode::Pop => {
                    self.pop()?;
                }
                OpCode::GetLocal => {
                    let slot = self.read_u16()? as usize;
                    let frame = self.current_frame();
                    if slot < frame.locals.len() {
                        let val = frame.locals[slot].clone();
                        self.push(val)?;
                    } else {
                        self.push(Value::Unit)?;
                    }
                }
                OpCode::SetLocal => {
                    let slot = self.read_u16()? as usize;
                    let val = self.pop()?;
                    let grow = (slot + 1).saturating_sub(self.current_frame().locals.len());
                    // Ordinary small frames are bounded by the call-depth limit; only large ones are summed.
                    if grow > 0 && slot >= 256 {
                        let total: usize = self.frames.iter().map(|f| f.locals.len()).sum();
                        if total + grow > Self::MAX_TOTAL_LOCALS {
                            return Err(VmError::ResourceLimit(format!(
                                "more than {} local variable slots in use",
                                Self::MAX_TOTAL_LOCALS
                            )));
                        }
                    }
                    let frame = self.current_frame_mut();
                    if slot >= frame.locals.len() {
                        frame.locals.resize(slot + 1, Value::Unit);
                    }
                    frame.locals[slot] = val;
                }
                OpCode::GetGlobal => {
                    let idx = self.read_u16()? as usize;
                    match self.constant_at(idx) {
                        Some(Value::Text(name)) => {
                            if let Some(val) = self.globals.get(&name) {
                                let v = val.clone();
                                self.push(v)?;
                            } else {
                                return Err(VmError::UndefinedGlobal(name));
                            }
                        }
                        _ => {
                            return Err(VmError::TypeError(format!(
                                "constant #{idx} is not a global name"
                            )))
                        }
                    }
                }
                OpCode::SetGlobal => {
                    let idx = self.read_u16()? as usize;
                    let val = self.peek(0)?.clone();
                    match self.constant_at(idx) {
                        Some(Value::Text(name)) => {
                            self.globals.insert(name, val);
                        }
                        _ => {
                            return Err(VmError::TypeError(format!(
                                "constant #{idx} is not a global name"
                            )))
                        }
                    }
                }
                OpCode::Equal => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(a == b))?;
                }
                OpCode::NotEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(a != b))?;
                }
                OpCode::Greater => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x > y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x > y))?,
                        (x, y) => {
                            return Err(VmError::TypeError(format!("Cannot compare {x:?} > {y:?}")))
                        }
                    }
                }
                OpCode::GreaterEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x >= y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x >= y))?,
                        (x, y) => {
                            return Err(VmError::TypeError(format!(
                                "Cannot compare {x:?} >= {y:?}"
                            )))
                        }
                    }
                }
                OpCode::Less => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x < y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x < y))?,
                        (x, y) => {
                            return Err(VmError::TypeError(format!("Cannot compare {x:?} < {y:?}")))
                        }
                    }
                }
                OpCode::LessEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x <= y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x <= y))?,
                        (x, y) => {
                            return Err(VmError::TypeError(format!(
                                "Cannot compare {x:?} <= {y:?}"
                            )))
                        }
                    }
                }
                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self
                            .push(Value::Int(x.checked_add(y).ok_or_else(|| {
                                VmError::IntegerOverflow(format!("{x} + {y}"))
                            })?))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x + y))?,
                        (Value::Text(x), Value::Text(y)) => {
                            if x.len() + y.len() > Self::MAX_TEXT_BYTES {
                                return Err(VmError::ResourceLimit(format!(
                                    "text longer than {} bytes",
                                    Self::MAX_TEXT_BYTES
                                )));
                            }
                            self.push(Value::Text(format!("{x}{y}")))?
                        }
                        (x, y) => {
                            return Err(VmError::TypeError(format!("Cannot add {x:?} and {y:?}")))
                        }
                    }
                }
                OpCode::Subtract => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self
                            .push(Value::Int(x.checked_sub(y).ok_or_else(|| {
                                VmError::IntegerOverflow(format!("{x} - {y}"))
                            })?))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x - y))?,
                        (x, y) => {
                            return Err(VmError::TypeError(format!(
                                "Cannot subtract {y:?} from {x:?}"
                            )))
                        }
                    }
                }
                OpCode::Multiply => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self
                            .push(Value::Int(x.checked_mul(y).ok_or_else(|| {
                                VmError::IntegerOverflow(format!("{x} * {y}"))
                            })?))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x * y))?,
                        (x, y) => {
                            return Err(VmError::TypeError(format!(
                                "Cannot multiply {x:?} by {y:?}"
                            )))
                        }
                    }
                }
                OpCode::Divide => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if y == 0 {
                                return Err(VmError::DivisionByZero);
                            }
                            self.push(Value::Int(
                                x.checked_div(y).ok_or_else(|| {
                                    VmError::IntegerOverflow(format!("{x} / {y}"))
                                })?,
                            ))?;
                        }
                        (Value::Float(x), Value::Float(y)) => {
                            if y == 0.0 {
                                return Err(VmError::DivisionByZero);
                            }
                            self.push(Value::Float(x / y))?;
                        }
                        (x, y) => {
                            return Err(VmError::TypeError(format!("Cannot divide {x:?} by {y:?}")))
                        }
                    }
                }
                OpCode::Modulo => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if y == 0 {
                                return Err(VmError::DivisionByZero);
                            }
                            self.push(Value::Int(
                                x.checked_rem(y).ok_or_else(|| {
                                    VmError::IntegerOverflow(format!("{x} % {y}"))
                                })?,
                            ))?;
                        }
                        (x, y) => {
                            return Err(VmError::TypeError(format!("Cannot modulo {x:?} by {y:?}")))
                        }
                    }
                }
                OpCode::And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(a.is_truthy() && b.is_truthy()))?;
                }
                OpCode::Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(a.is_truthy() || b.is_truthy()))?;
                }
                OpCode::Not => {
                    let val = self.pop()?;
                    self.push(Value::Bool(!val.is_truthy()))?;
                }
                OpCode::Negate => {
                    let val = self.pop()?;
                    match val {
                        Value::Int(n) => self.push(Value::Int(
                            n.checked_neg()
                                .ok_or_else(|| VmError::IntegerOverflow(format!("-{n}")))?,
                        ))?,
                        Value::Float(f) => self.push(Value::Float(-f))?,
                        _ => return Err(VmError::TypeError(format!("Cannot negate {val:?}"))),
                    }
                }
                OpCode::Jump => {
                    let target = self.read_u16()? as usize;
                    self.current_frame_mut().ip = target;
                }
                OpCode::JumpIfFalse => {
                    let target = self.read_u16()? as usize;
                    let cond = self.pop()?;
                    if !cond.is_truthy() {
                        self.current_frame_mut().ip = target;
                    }
                }
                OpCode::Call => {
                    let arg_count = self.read_byte()? as usize;
                    self.call_value(arg_count)?;
                }
                OpCode::Emit => {
                    let val = self.pop()?;
                    let out_str = format!("{val}");
                    self.output.push(out_str.clone());
                    println!("{out_str}");
                    self.push(Value::Unit)?;
                }
                OpCode::Array => {
                    let len = self.read_u16()? as usize;
                    let mut elements = Vec::with_capacity(len);
                    for _ in 0..len {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    self.push(Value::Array(elements))?;
                }
                OpCode::Index => {
                    let idx_val = self.pop()?;
                    let col_val = self.pop()?;
                    match (col_val, idx_val) {
                        (Value::Array(arr), Value::Int(idx)) => {
                            if idx >= 0 && (idx as usize) < arr.len() {
                                self.push(arr[idx as usize].clone())?;
                            } else {
                                return Err(VmError::TypeError(format!(
                                    "Index out of bounds: index {idx} in array of length {}",
                                    arr.len()
                                )));
                            }
                        }
                        (c, i) => {
                            return Err(VmError::TypeError(format!(
                                "Cannot index {c:?} with {i:?}"
                            )))
                        }
                    }
                }
                OpCode::SetIndex => {
                    let value = self.pop()?;
                    let idx_val = self.pop()?;
                    let col_val = self.pop()?;
                    match (col_val, idx_val) {
                        (Value::Array(mut arr), Value::Int(idx)) => {
                            if idx >= 0 && (idx as usize) < arr.len() {
                                arr[idx as usize] = value;
                                self.push(Value::Array(arr))?;
                            } else {
                                return Err(VmError::TypeError(format!(
                                    "Index out of bounds: index {idx} in array of length {}",
                                    arr.len()
                                )));
                            }
                        }
                        (c, i) => {
                            return Err(VmError::TypeError(format!(
                                "Cannot assign to element {i:?} of {c:?}"
                            )))
                        }
                    }
                }
                OpCode::SetField => {
                    let name_idx = self.read_u16()? as usize;
                    let field = match self.constant_at(name_idx) {
                        Some(Value::Text(s)) => s,
                        _ => {
                            return Err(VmError::TypeError("malformed field constant".to_string()))
                        }
                    };
                    let value = self.pop()?;
                    match self.pop()? {
                        Value::Struct { name, mut fields } => {
                            match fields.iter_mut().find(|(k, _)| *k == field) {
                                Some(slot) => slot.1 = value,
                                None => {
                                    return Err(VmError::TypeError(format!(
                                        "{name} has no field '{field}'"
                                    )))
                                }
                            }
                            self.push(Value::Struct { name, fields })?;
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "cannot assign field '{field}' of {other:?}"
                            )))
                        }
                    }
                }
                OpCode::Struct => {
                    let name_idx = self.read_u16()? as usize;
                    let names_idx = self.read_u16()? as usize;
                    let (name, names) = (self.constant_at(name_idx), self.constant_at(names_idx));
                    let (Some(Value::Text(name)), Some(Value::Array(names))) = (name, names) else {
                        return Err(VmError::TypeError("malformed struct constant".to_string()));
                    };
                    let mut values = Vec::with_capacity(names.len());
                    for _ in 0..names.len() {
                        values.push(self.pop()?);
                    }
                    values.reverse();
                    let fields = names
                        .into_iter()
                        .zip(values)
                        .map(|(n, v)| match n {
                            Value::Text(s) => (s, v),
                            _ => (String::new(), v),
                        })
                        .collect();
                    self.push(Value::Struct { name, fields })?;
                }
                OpCode::Field => {
                    let name_idx = self.read_u16()? as usize;
                    let field = match self.constant_at(name_idx) {
                        Some(Value::Text(s)) => s,
                        _ => {
                            return Err(VmError::TypeError("malformed field constant".to_string()))
                        }
                    };
                    match self.pop()? {
                        Value::Struct { name, fields } => {
                            match fields.into_iter().find(|(k, _)| *k == field) {
                                Some((_, v)) => self.push(v)?,
                                None => {
                                    return Err(VmError::TypeError(format!(
                                        "{name} has no field '{field}'"
                                    )))
                                }
                            }
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "cannot read field '{field}' of {other:?}"
                            )))
                        }
                    }
                }
                OpCode::EnumNew => {
                    let idx = self.read_u16()? as usize;
                    let carries = self.read_byte()? == 1;
                    let full = match self.constant_at(idx) {
                        Some(Value::Text(s)) => s,
                        _ => return Err(VmError::TypeError("malformed enum constant".to_string())),
                    };
                    let Some((name, variant)) = full.split_once('.') else {
                        return Err(VmError::TypeError("malformed enum constant".to_string()));
                    };
                    let payload = if carries {
                        Some(Box::new(self.pop()?))
                    } else {
                        None
                    };
                    self.push(Value::Enum {
                        name: name.to_string(),
                        variant: variant.to_string(),
                        payload,
                    })?;
                }
                OpCode::EnumIs => {
                    let idx = self.read_u16()? as usize;
                    let full = match self.constant_at(idx) {
                        Some(Value::Text(s)) => s,
                        _ => return Err(VmError::TypeError("malformed enum constant".to_string())),
                    };
                    match self.pop()? {
                        Value::Enum { name, variant, .. } => {
                            self.push(Value::Bool(format!("{name}.{variant}") == full))?;
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "match expects an enum value, got {other:?}"
                            )))
                        }
                    }
                }
                OpCode::EnumPayload => match self.pop()? {
                    Value::Enum {
                        payload: Some(v), ..
                    } => self.push(*v)?,
                    other => {
                        return Err(VmError::TypeError(format!(
                            "enum value {other:?} carries no value"
                        )))
                    }
                },
                OpCode::Len => {
                    let v = self.pop()?;
                    let n = Self::length_of(&v)?;
                    self.push(Value::Int(n))?;
                }
                OpCode::Return => {
                    let result = self.pop()?;
                    self.frames.pop();
                    if self.frames.is_empty() {
                        return Ok(result);
                    }
                    self.push(result)?;
                }
                OpCode::Halt => {
                    let res = self.pop().unwrap_or(Value::Unit);
                    return Ok(res);
                }
            }
        }

        Ok(self.pop().unwrap_or(Value::Unit))
    }

    fn call_value(&mut self, arg_count: usize) -> Result<(), VmError> {
        let callee_idx =
            (self.stack.len().checked_sub(1 + arg_count)).ok_or(VmError::StackUnderflow)?;
        let callee = self.stack[callee_idx].clone();

        match callee {
            Value::UserFunction {
                chunk_index, arity, ..
            } => {
                if arg_count != arity {
                    return Err(VmError::TypeError(format!(
                        "Function expects {arity} arguments, but {arg_count} were provided"
                    )));
                }
                let mut args = Vec::with_capacity(arg_count);
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();
                self.pop()?; // pop callee

                if self.frames.len() >= Self::MAX_CALL_DEPTH {
                    return Err(VmError::ResourceLimit(format!(
                        "call depth exceeds {}",
                        Self::MAX_CALL_DEPTH
                    )));
                }
                self.frames.push(CallFrame {
                    chunk_index,
                    ip: 0,
                    locals: args,
                });
                Ok(())
            }
            Value::NativeFunction(name) => {
                let mut args = Vec::with_capacity(arg_count);
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();
                self.pop()?; // pop callee

                let expected = match name.as_str() {
                    "IPv4" | "IPv6" | "Port" | "len" => 1,
                    "Endpoint" | "push" | "buf_get" | "buf_get16" | "buf_get32" | "buf_concat"
                    | "Packet" => 2,
                    "buf_set" | "buf_set16" | "buf_set32" | "buf_slice" | "buf_find"
                    | "buf_find_wild" => 3,
                    "NetworkBuffer" | "bytes_of" | "packet_header" | "packet_payload"
                    | "str_upper" | "str_lower" | "str_trim" | "str_from_int" | "str_parse_int" => {
                        1
                    }
                    "str_contains" | "str_starts_with" | "str_ends_with" | "str_find"
                    | "str_repeat" | "str_split" | "file_write" | "file_append" => 2,
                    "file_read" | "file_exists" | "fail" | "file_read_bytes" | "sha256"
                    | "hex_encode" | "hex_decode" | "base64_encode" | "base64_decode"
                    | "buf_entropy" => 1,
                    "hmac_sha256" => 2,
                    "str_slice" | "str_replace" => 3,
                    other => {
                        return Err(VmError::TypeError(format!(
                            "Unknown native function '{other}'"
                        )))
                    }
                };
                if args.len() != expected {
                    return Err(VmError::TypeError(format!(
                        "{name} expects {expected} argument(s), but {} were provided",
                        args.len()
                    )));
                }
                let result = match name.as_str() {
                    "IPv4" => {
                        let text = match &args[0] {
                            Value::Text(s) => s.as_str(),
                            _ => {
                                return Err(VmError::TypeError(
                                    "IPv4 expects a string argument".to_string(),
                                ))
                            }
                        };
                        match text.parse::<Ipv4Addr>() {
                            Ok(ip) => Value::IPv4(ip.octets()),
                            Err(e) => {
                                return Err(VmError::ProtocolError(format!(
                                    "Invalid IPv4 address: {e}"
                                )))
                            }
                        }
                    }
                    "IPv6" => {
                        let text = match &args[0] {
                            Value::Text(s) => s.as_str(),
                            _ => {
                                return Err(VmError::TypeError(
                                    "IPv6 expects a string argument".to_string(),
                                ))
                            }
                        };
                        match text.parse::<std::net::Ipv6Addr>() {
                            Ok(ip) => Value::IPv6(ip.octets()),
                            Err(e) => {
                                return Err(VmError::ProtocolError(format!(
                                    "Invalid IPv6 address: {e}"
                                )))
                            }
                        }
                    }
                    "Port" => {
                        let port_num = match args[0] {
                            Value::Int(n) => n,
                            _ => {
                                return Err(VmError::TypeError(
                                    "Port expects an integer argument".to_string(),
                                ))
                            }
                        };
                        if !(1..=65535).contains(&port_num) {
                            return Err(VmError::ProtocolError(format!(
                                "Port out of range (1-65535): {port_num}"
                            )));
                        }
                        Value::Port(port_num as u16)
                    }
                    "Endpoint" => {
                        let ip_bytes = match args[0] {
                            Value::IPv4(b) => b,
                            _ => {
                                return Err(VmError::TypeError(
                                    "Endpoint arg 1 must be IPv4".to_string(),
                                ))
                            }
                        };
                        let port = match args[1] {
                            Value::Port(p) => p,
                            Value::Int(n) if (1..=65535).contains(&n) => n as u16,
                            Value::Int(n) => {
                                return Err(VmError::ProtocolError(format!(
                                    "Port out of range (1-65535): {n}"
                                )))
                            }
                            _ => {
                                return Err(VmError::TypeError(
                                    "Endpoint arg 2 must be Port".to_string(),
                                ))
                            }
                        };
                        Value::Endpoint(ip_bytes, port)
                    }
                    "len" => Value::Int(Self::length_of(&args[0])?),
                    "push" => {
                        let mut it = args.into_iter();
                        match (it.next(), it.next()) {
                            (Some(Value::Array(mut items)), Some(v)) => {
                                if items.len() >= Self::MAX_ARRAY_LEN {
                                    return Err(VmError::ResourceLimit(format!(
                                        "array length exceeds {}",
                                        Self::MAX_ARRAY_LEN
                                    )));
                                }
                                items.push(v);
                                Value::Array(items)
                            }
                            (first, _) => {
                                return Err(VmError::TypeError(format!(
                                    "push expects an Array as its first argument, got {first:?}"
                                )))
                            }
                        }
                    }
                    "fail" => match &args[0] {
                        Value::Text(m) => {
                            return Err(VmError::Failed(m.chars().take(500).collect()))
                        }
                        other => {
                            return Err(VmError::TypeError(format!(
                                "fail expects Text, got {other:?}"
                            )))
                        }
                    },
                    "sha256" | "hmac_sha256" | "hex_encode" | "hex_decode" | "base64_encode"
                    | "base64_decode" | "buf_find" | "buf_find_wild" | "buf_entropy" => {
                        sec::sec_native(name.as_str(), &args, Self::MAX_BUFFER_BYTES)?
                    }
                    other if other.starts_with("file_") => self.file_native(other, &args)?,
                    other if other.starts_with("str_") => Self::text_native(other, &args)?,
                    other => Self::net_native(other, args)?,
                };

                self.push(result)?;
                Ok(())
            }
            _ => Err(VmError::TypeError(format!(
                "Value {callee:?} is not callable"
            ))),
        }
    }

    fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn read_byte(&mut self) -> Result<u8, VmError> {
        let frame_idx = self.frames.len() - 1;
        let chunk_idx = self.frames[frame_idx].chunk_index;
        let ip = self.frames[frame_idx].ip;
        let chunk = &self.chunks[chunk_idx];
        if ip >= chunk.code.len() {
            return Err(VmError::InvalidIp { ip });
        }
        let b = chunk.code[ip];
        self.frames[frame_idx].ip += 1;
        Ok(b)
    }

    fn constant_at(&self, idx: usize) -> Option<Value> {
        self.chunks[self.current_frame().chunk_index]
            .constants
            .get(idx)
            .cloned()
    }

    fn read_u16(&mut self) -> Result<u16, VmError> {
        let high = self.read_byte()? as u16;
        let low = self.read_byte()? as u16;
        Ok((high << 8) | low)
    }

    /// Largest NetworkBuffer a program may create or grow to.
    pub const MAX_BUFFER_BYTES: usize = 1 << 20;

    /// `NetworkBuffer` and `Packet` natives. Every access is bounds-checked; writes return a new buffer.
    /// `file_*` natives. Disabled unless `fs_root` is set. Paths are relative to that directory;
    /// absolute paths, `..` and anything that resolves (through symlinks) outside it are refused.
    fn file_native(&self, name: &str, args: &[Value]) -> Result<Value, VmError> {
        let fail = |m: String| VmError::ProtocolError(m);
        let Some(root) = &self.fs_root else {
            return Err(fail(format!(
                "{name}: file access is disabled (the command line enables it with --allow-fs <directory>)"
            )));
        };
        let root = root
            .canonicalize()
            .map_err(|e| fail(format!("{name}: cannot open the allowed directory: {e}")))?;
        let Some(Value::Text(rel)) = args.first() else {
            return Err(VmError::TypeError(format!("{name} expects a Text path")));
        };
        let rel_path = std::path::Path::new(rel);
        if rel.is_empty()
            || rel_path.is_absolute()
            || rel_path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(fail(format!(
                "{name}: '{rel}' must be a plain relative path inside the allowed directory"
            )));
        }
        let target = root.join(rel_path);
        // Resolve symlinks: the target (if it exists) or its parent must stay inside the root.
        let anchor = if target.exists() {
            target.canonicalize()
        } else {
            target
                .parent()
                .unwrap_or(&root)
                .canonicalize()
                .map(|p| p.join(target.file_name().unwrap_or_default()))
        }
        .map_err(|e| fail(format!("{name}: '{rel}': {e}")))?;
        if !anchor.starts_with(&root) {
            return Err(fail(format!(
                "{name}: '{rel}' resolves outside the allowed directory"
            )));
        }
        let limit = Self::MAX_TEXT_BYTES;
        match name {
            "file_exists" => Ok(Value::Bool(anchor.exists())),
            "file_read" => {
                let meta = std::fs::metadata(&anchor)
                    .map_err(|e| fail(format!("{name}: '{rel}': {e}")))?;
                if meta.len() as usize > limit {
                    return Err(VmError::ResourceLimit(format!(
                        "file '{rel}' is larger than {limit} bytes"
                    )));
                }
                let text = std::fs::read_to_string(&anchor)
                    .map_err(|e| fail(format!("{name}: '{rel}': {e}")))?;
                Ok(Value::Text(text))
            }
            "file_read_bytes" => {
                let meta = std::fs::metadata(&anchor)
                    .map_err(|e| fail(format!("{name}: '{rel}': {e}")))?;
                if meta.len() as usize > Self::MAX_BUFFER_BYTES {
                    return Err(VmError::ResourceLimit(format!(
                        "file '{rel}' is larger than {} bytes",
                        Self::MAX_BUFFER_BYTES
                    )));
                }
                let bytes =
                    std::fs::read(&anchor).map_err(|e| fail(format!("{name}: '{rel}': {e}")))?;
                Ok(Value::Buffer(bytes))
            }
            "file_write" | "file_append" => {
                let Some(Value::Text(data)) = args.get(1) else {
                    return Err(VmError::TypeError(format!("{name} expects Text data")));
                };
                use std::io::Write;
                let mut opts = std::fs::OpenOptions::new();
                opts.create(true);
                if name == "file_append" {
                    opts.append(true);
                } else {
                    opts.write(true).truncate(true);
                }
                if name == "file_append" {
                    let size = std::fs::metadata(&anchor)
                        .map(|m| m.len() as usize)
                        .unwrap_or(0);
                    if size.saturating_add(data.len()) > limit {
                        return Err(VmError::ResourceLimit(format!(
                            "file '{rel}' would exceed {limit} bytes"
                        )));
                    }
                }
                opts.open(&anchor)
                    .and_then(|mut f| f.write_all(data.as_bytes()))
                    .map_err(|e| fail(format!("{name}: '{rel}': {e}")))?;
                Ok(Value::Unit)
            }
            other => Err(VmError::TypeError(format!(
                "Unknown native function '{other}'"
            ))),
        }
    }

    /// Pure Text natives (`str_*`). Positions are in characters, like `len`; results are bounded by `MAX_TEXT_BYTES`.
    fn text_native(name: &str, args: &[Value]) -> Result<Value, VmError> {
        fn text<'a>(v: &'a Value, name: &str) -> Result<&'a str, VmError> {
            match v {
                Value::Text(t) => Ok(t),
                other => Err(VmError::TypeError(format!(
                    "{name} expects Text, got {other:?}"
                ))),
            }
        }
        fn int(v: &Value, name: &str) -> Result<i64, VmError> {
            match v {
                Value::Int(n) => Ok(*n),
                other => Err(VmError::TypeError(format!(
                    "{name} expects an Int, got {other:?}"
                ))),
            }
        }
        let limit = |n: usize| -> Result<(), VmError> {
            if n > Self::MAX_TEXT_BYTES {
                Err(VmError::ResourceLimit(format!(
                    "text longer than {} bytes",
                    Self::MAX_TEXT_BYTES
                )))
            } else {
                Ok(())
            }
        };
        Ok(match name {
            "str_upper" => Value::Text(text(&args[0], name)?.to_uppercase()),
            "str_lower" => Value::Text(text(&args[0], name)?.to_lowercase()),
            "str_trim" => Value::Text(text(&args[0], name)?.trim().to_string()),
            "str_contains" => Value::Bool(text(&args[0], name)?.contains(text(&args[1], name)?)),
            "str_starts_with" => {
                Value::Bool(text(&args[0], name)?.starts_with(text(&args[1], name)?))
            }
            "str_ends_with" => Value::Bool(text(&args[0], name)?.ends_with(text(&args[1], name)?)),
            "str_find" => {
                let (s, n) = (text(&args[0], name)?, text(&args[1], name)?);
                Value::Int(match s.find(n) {
                    Some(byte) => s[..byte].chars().count() as i64,
                    None => -1,
                })
            }
            "str_slice" => {
                let s = text(&args[0], name)?;
                let (from, to) = (int(&args[1], name)?, int(&args[2], name)?);
                let len = s.chars().count() as i64;
                if from < 0 || to < from || to > len {
                    return Err(VmError::TypeError(format!(
                        "Slice out of bounds: {from}..{to} in text of length {len}"
                    )));
                }
                Value::Text(
                    s.chars()
                        .skip(from as usize)
                        .take((to - from) as usize)
                        .collect(),
                )
            }
            "str_repeat" => {
                let s = text(&args[0], name)?;
                let n = int(&args[1], name)?;
                if n < 0 {
                    return Err(VmError::TypeError(format!(
                        "str_repeat count must not be negative, got {n}"
                    )));
                }
                limit(s.len().saturating_mul(n as usize))?;
                Value::Text(s.repeat(n as usize))
            }
            "str_replace" => {
                let (s, from, to) = (
                    text(&args[0], name)?,
                    text(&args[1], name)?,
                    text(&args[2], name)?,
                );
                if from.is_empty() {
                    return Err(VmError::TypeError(
                        "str_replace: the text to replace must not be empty".to_string(),
                    ));
                }
                let hits = s.matches(from).count();
                limit((s.len() + hits.saturating_mul(to.len())).saturating_sub(hits * from.len()))?;
                Value::Text(s.replace(from, to))
            }
            "str_split" => {
                let (s, sep) = (text(&args[0], name)?, text(&args[1], name)?);
                if sep.is_empty() {
                    return Err(VmError::TypeError(
                        "str_split: the separator must not be empty".to_string(),
                    ));
                }
                let parts: Vec<Value> = s
                    .split(sep)
                    .take(Self::MAX_ARRAY_LEN + 1)
                    .map(|p| Value::Text(p.to_string()))
                    .collect();
                if parts.len() > Self::MAX_ARRAY_LEN {
                    return Err(VmError::ResourceLimit(format!(
                        "array length exceeds {}",
                        Self::MAX_ARRAY_LEN
                    )));
                }
                Value::Array(parts)
            }
            "str_from_int" => Value::Text(int(&args[0], name)?.to_string()),
            "str_parse_int" => {
                let s = text(&args[0], name)?;
                Value::Int(s.parse::<i64>().map_err(|_| {
                    VmError::TypeError(format!("str_parse_int: '{s}' is not a valid Int"))
                })?)
            }
            other => {
                return Err(VmError::TypeError(format!(
                    "Unknown native function '{other}'"
                )))
            }
        })
    }

    fn net_native(name: &str, args: Vec<Value>) -> Result<Value, VmError> {
        fn int(v: &Value, what: &str) -> Result<i64, VmError> {
            match v {
                Value::Int(n) => Ok(*n),
                other => Err(VmError::TypeError(format!(
                    "{what} must be an Int, got {other:?}"
                ))),
            }
        }
        fn buf<'a>(v: &'a Value, what: &str) -> Result<&'a Vec<u8>, VmError> {
            match v {
                Value::Buffer(b) => Ok(b),
                other => Err(VmError::TypeError(format!(
                    "{what} must be a NetworkBuffer, got {other:?}"
                ))),
            }
        }
        // Checks that `width` bytes starting at `off` lie inside a buffer of length `len`.
        fn span(off: i64, width: usize, len: usize) -> Result<usize, VmError> {
            if off < 0 || (off as u64).saturating_add(width as u64) > len as u64 {
                return Err(VmError::ProtocolError(format!(
                    "buffer access out of bounds: {width} byte(s) at offset {off} in a buffer of {len}"
                )));
            }
            Ok(off as usize)
        }
        let too_big = |n: usize| {
            VmError::ResourceLimit(format!(
                "buffer of {n} bytes exceeds the limit of {}",
                Self::MAX_BUFFER_BYTES
            ))
        };
        let get = |args: &[Value], width: usize| -> Result<Value, VmError> {
            let b = buf(&args[0], "first argument")?;
            let at = span(int(&args[1], "offset")?, width, b.len())?;
            let mut n: i64 = 0;
            for byte in &b[at..at + width] {
                n = (n << 8) | *byte as i64;
            }
            Ok(Value::Int(n))
        };
        let set = |args: &[Value], width: usize| -> Result<Value, VmError> {
            let mut b = buf(&args[0], "first argument")?.clone();
            let at = span(int(&args[1], "offset")?, width, b.len())?;
            let v = int(&args[2], "value")?;
            if v < 0 || (width < 8 && v >= 1i64 << (8 * width)) {
                return Err(VmError::ProtocolError(format!(
                    "value {v} does not fit in {width} byte(s)"
                )));
            }
            for i in 0..width {
                b[at + i] = ((v >> (8 * (width - 1 - i))) & 0xFF) as u8;
            }
            Ok(Value::Buffer(b))
        };
        match name {
            "NetworkBuffer" => {
                let n = int(&args[0], "size")?;
                if n < 0 {
                    return Err(VmError::ProtocolError(format!("negative buffer size {n}")));
                }
                if n as usize > Self::MAX_BUFFER_BYTES {
                    return Err(too_big(n as usize));
                }
                Ok(Value::Buffer(vec![0; n as usize]))
            }
            "bytes_of" => match &args[0] {
                Value::Text(t) if t.len() <= Self::MAX_BUFFER_BYTES => {
                    Ok(Value::Buffer(t.as_bytes().to_vec()))
                }
                Value::Text(t) => Err(too_big(t.len())),
                other => Err(VmError::TypeError(format!(
                    "bytes_of expects Text, got {other:?}"
                ))),
            },
            "buf_get" => get(&args, 1),
            "buf_get16" => get(&args, 2),
            "buf_get32" => get(&args, 4),
            "buf_set" => set(&args, 1),
            "buf_set16" => set(&args, 2),
            "buf_set32" => set(&args, 4),
            "buf_slice" => {
                let b = buf(&args[0], "first argument")?;
                let len = int(&args[2], "length")?;
                if len < 0 {
                    return Err(VmError::ProtocolError(format!(
                        "negative slice length {len}"
                    )));
                }
                let at = span(int(&args[1], "start")?, len as usize, b.len())?;
                Ok(Value::Buffer(b[at..at + len as usize].to_vec()))
            }
            "buf_concat" => {
                let (a, b) = (
                    buf(&args[0], "first argument")?,
                    buf(&args[1], "second argument")?,
                );
                if a.len() + b.len() > Self::MAX_BUFFER_BYTES {
                    return Err(too_big(a.len() + b.len()));
                }
                let mut out = a.clone();
                out.extend_from_slice(b);
                Ok(Value::Buffer(out))
            }
            "Packet" => Ok(Value::Packet {
                header: buf(&args[0], "header")?.clone(),
                payload: buf(&args[1], "payload")?.clone(),
            }),
            "packet_header" | "packet_payload" => match &args[0] {
                Value::Packet { header, payload } => {
                    Ok(Value::Buffer(if name == "packet_header" {
                        header.clone()
                    } else {
                        payload.clone()
                    }))
                }
                other => Err(VmError::TypeError(format!(
                    "{name} expects a Packet, got {other:?}"
                ))),
            },
            other => Err(VmError::TypeError(format!(
                "Unknown native function '{other}'"
            ))),
        }
    }

    /// Length of an Array (elements) or Text (characters) as an `Int`.
    fn length_of(v: &Value) -> Result<i64, VmError> {
        match v {
            Value::Array(a) => Ok(a.len() as i64),
            Value::Text(t) => Ok(t.chars().count() as i64),
            Value::Buffer(b) => Ok(b.len() as i64),
            other => Err(VmError::TypeError(format!(
                "len expects an Array, Text or NetworkBuffer, got {other:?}"
            ))),
        }
    }

    fn push(&mut self, val: Value) -> Result<(), VmError> {
        if self.stack.len() >= Self::MAX_STACK_SIZE {
            return Err(VmError::StackOverflow);
        }
        self.stack.push(val);
        Ok(())
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn peek(&self, distance: usize) -> Result<&Value, VmError> {
        if distance >= self.stack.len() {
            Err(VmError::StackUnderflow)
        } else {
            Ok(&self.stack[self.stack.len() - 1 - distance])
        }
    }
}
