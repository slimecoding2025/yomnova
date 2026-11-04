//! Stack-based Virtual Machine and Runtime for YOMNOVA.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use yomnova_bytecode::{Chunk, OpCode, Value};

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
}

impl Vm {
    pub const DEFAULT_MAX_STEPS: usize = 1_000_000;
    pub const MAX_STACK_SIZE: usize = 65_536;
    pub const MAX_CALL_DEPTH: usize = 10_000;
    pub const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
    pub const MAX_ARRAY_LEN: usize = 1_000_000;

    pub fn new() -> Self {
        let mut vm = Self {
            chunks: Vec::new(),
            stack: Vec::with_capacity(256),
            frames: Vec::new(),
            globals: HashMap::new(),
            output: Vec::new(),
            max_steps: Self::DEFAULT_MAX_STEPS,
            step_count: 0,
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
        for name in ["len", "push"] {
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
                    let idx = self.read_u16()?;
                    let val = self.chunks[self.current_frame().chunk_index].constants[idx as usize]
                        .clone();
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
                    let frame = self.current_frame_mut();
                    if slot >= frame.locals.len() {
                        frame.locals.resize(slot + 1, Value::Unit);
                    }
                    frame.locals[slot] = val;
                }
                OpCode::GetGlobal => {
                    let idx = self.read_u16()?;
                    let name_val =
                        &self.chunks[self.current_frame().chunk_index].constants[idx as usize];
                    if let Value::Text(name) = name_val {
                        if let Some(val) = self.globals.get(name) {
                            let v = val.clone();
                            self.push(v)?;
                        } else {
                            return Err(VmError::UndefinedGlobal(name.clone()));
                        }
                    }
                }
                OpCode::SetGlobal => {
                    let idx = self.read_u16()?;
                    let name_val = self.chunks[self.current_frame().chunk_index].constants
                        [idx as usize]
                        .clone();
                    let val = self.peek(0)?.clone();
                    if let Value::Text(name) = name_val {
                        self.globals.insert(name, val);
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
                    "IPv4" | "Port" | "len" => 1,
                    "Endpoint" | "push" => 2,
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
                    other => {
                        return Err(VmError::TypeError(format!(
                            "Unknown native function '{other}'"
                        )))
                    }
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

    /// Length of an Array (elements) or Text (characters) as an `Int`.
    fn length_of(v: &Value) -> Result<i64, VmError> {
        match v {
            Value::Array(a) => Ok(a.len() as i64),
            Value::Text(t) => Ok(t.chars().count() as i64),
            other => Err(VmError::TypeError(format!(
                "len expects an Array or Text, got {other:?}"
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
