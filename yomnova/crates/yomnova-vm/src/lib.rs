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
        vm.globals.insert("IPv4".to_string(), Value::NativeFunction("IPv4".to_string()));
        vm.globals.insert("Port".to_string(), Value::NativeFunction("Port".to_string()));
        vm.globals.insert("Endpoint".to_string(), Value::NativeFunction("Endpoint".to_string()));

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
                return Err(VmError::StepLimitExceeded { limit: self.max_steps });
            }

            let frame = self.frames.last_mut().unwrap();
            let chunk = &self.chunks[frame.chunk_index];

            if frame.ip >= chunk.code.len() {
                break;
            }

            let op_byte = chunk.code[frame.ip];
            frame.ip += 1;

            if op_byte > OpCode::Halt as u8 {
                return Err(VmError::InvalidIp { ip: frame.ip });
            }

            let op: OpCode = unsafe { std::mem::transmute(op_byte) };

            match op {
                OpCode::Constant => {
                    let idx = self.read_u16()?;
                    let val = self.chunks[self.current_frame().chunk_index].constants[idx as usize].clone();
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
                    let name_val = &self.chunks[self.current_frame().chunk_index].constants[idx as usize];
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
                    let name_val = self.chunks[self.current_frame().chunk_index].constants[idx as usize].clone();
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
                        (x, y) => return Err(VmError::TypeError(format!("Cannot compare {x:?} > {y:?}"))),
                    }
                }
                OpCode::GreaterEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x >= y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x >= y))?,
                        (x, y) => return Err(VmError::TypeError(format!("Cannot compare {x:?} >= {y:?}"))),
                    }
                }
                OpCode::Less => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x < y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x < y))?,
                        (x, y) => return Err(VmError::TypeError(format!("Cannot compare {x:?} < {y:?}"))),
                    }
                }
                OpCode::LessEqual => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Bool(x <= y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Bool(x <= y))?,
                        (x, y) => return Err(VmError::TypeError(format!("Cannot compare {x:?} <= {y:?}"))),
                    }
                }
                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Int(x + y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x + y))?,
                        (Value::Text(x), Value::Text(y)) => self.push(Value::Text(format!("{x}{y}")))?,
                        (x, y) => return Err(VmError::TypeError(format!("Cannot add {x:?} and {y:?}"))),
                    }
                }
                OpCode::Subtract => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Int(x - y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x - y))?,
                        (x, y) => return Err(VmError::TypeError(format!("Cannot subtract {y:?} from {x:?}"))),
                    }
                }
                OpCode::Multiply => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => self.push(Value::Int(x * y))?,
                        (Value::Float(x), Value::Float(y)) => self.push(Value::Float(x * y))?,
                        (x, y) => return Err(VmError::TypeError(format!("Cannot multiply {x:?} by {y:?}"))),
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
                            self.push(Value::Int(x / y))?;
                        }
                        (Value::Float(x), Value::Float(y)) => {
                            if y == 0.0 {
                                return Err(VmError::DivisionByZero);
                            }
                            self.push(Value::Float(x / y))?;
                        }
                        (x, y) => return Err(VmError::TypeError(format!("Cannot divide {x:?} by {y:?}"))),
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
                            self.push(Value::Int(x % y))?;
                        }
                        (x, y) => return Err(VmError::TypeError(format!("Cannot modulo {x:?} by {y:?}"))),
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
                        Value::Int(n) => self.push(Value::Int(-n))?,
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
                    let cond = self.peek(0)?;
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
                            let i = if idx < 0 {
                                arr.len() as i64 + idx
                            } else {
                                idx
                            };
                            if i >= 0 && (i as usize) < arr.len() {
                                self.push(arr[i as usize].clone())?;
                            } else {
                                return Err(VmError::TypeError(format!(
                                    "Index out of bounds: index {idx} in array of length {}",
                                    arr.len()
                                )));
                            }
                        }
                        (c, i) => return Err(VmError::TypeError(format!("Cannot index {c:?} with {i:?}"))),
                    }
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
        let callee_idx = self.stack.len() - 1 - arg_count;
        let callee = self.stack[callee_idx].clone();

        match callee {
            Value::UserFunction { chunk_index, arity, .. } => {
                if arg_count != arity {
                    return Err(VmError::TypeError(format!("Function expects {arity} arguments, but {arg_count} were provided")));
                }
                let mut args = Vec::with_capacity(arg_count);
                for _ in 0..arg_count {
                    args.push(self.pop()?);
                }
                args.reverse();
                self.pop()?; // pop callee

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

                let result = match name.as_str() {
                    "IPv4" => {
                        let text = match &args[0] {
                            Value::Text(s) => s.as_str(),
                            _ => return Err(VmError::TypeError("IPv4 expects a string argument".to_string())),
                        };
                        match text.parse::<Ipv4Addr>() {
                            Ok(ip) => Value::IPv4(ip.octets()),
                            Err(e) => return Err(VmError::ProtocolError(format!("Invalid IPv4 address: {e}"))),
                        }
                    }
                    "Port" => {
                        let port_num = match args[0] {
                            Value::Int(n) => n,
                            _ => return Err(VmError::TypeError("Port expects an integer argument".to_string())),
                        };
                        if !(1..=65535).contains(&port_num) {
                            return Err(VmError::ProtocolError(format!("Port out of range (1-65535): {port_num}")));
                        }
                        Value::Port(port_num as u16)
                    }
                    "Endpoint" => {
                        let ip_bytes = match args[0] {
                            Value::IPv4(b) => b,
                            _ => return Err(VmError::TypeError("Endpoint arg 1 must be IPv4".to_string())),
                        };
                        let port = match args[1] {
                            Value::Port(p) => p,
                            Value::Int(n) => n as u16,
                            _ => return Err(VmError::TypeError("Endpoint arg 2 must be Port".to_string())),
                        };
                        Value::Endpoint(ip_bytes, port)
                    }
                    other => return Err(VmError::TypeError(format!("Unknown native function '{other}'"))),
                };

                self.push(result)?;
                Ok(())
            }
            _ => Err(VmError::TypeError(format!("Value {callee:?} is not callable"))),
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

    fn read_u16(&mut self) -> Result<u16, VmError> {
        let high = self.read_byte()? as u16;
        let low = self.read_byte()? as u16;
        Ok((high << 8) | low)
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
