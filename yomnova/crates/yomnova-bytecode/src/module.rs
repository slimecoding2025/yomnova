//! The `.yomc` file format: a compiled program that can be saved and run later.
//!
//! Layout (all integers big-endian):
//!
//! ```text
//! "YOMC"  u16 format version  u16 opcode count  u16 function count
//! main chunk
//! per function: u16 name length, name (UTF-8), u8 arity, chunk
//! chunk: u32 code length, code bytes, u32 line per code byte, u16 constant count, constants
//! constant: u8 tag + payload  (0 Int i64, 1 Float f64, 2 Text u32+UTF-8, 3 Bool u8, 4 Unit, 5 Array u32 + constants)
//! ```
//!
//! Decoding treats the input as untrusted: every length is checked against the bytes that are
//! actually left, tags and UTF-8 are validated, nesting and sizes are capped, and trailing bytes
//! are rejected. The result is still only *structurally* valid; the VM re-checks every operand
//! while executing and has step, stack, memory and call-depth limits.

use crate::{Chunk, OpCode, Value};

pub const MAGIC: &[u8; 4] = b"YOMC";
/// Bumped whenever the layout or the opcode numbering changes.
pub const FORMAT_VERSION: u16 = 1;

const MAX_CODE_BYTES: usize = 1 << 20;
const MAX_FUNCTIONS: usize = 4096;
const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARRAY_CONST: usize = 65_536;
const MAX_CONST_DEPTH: usize = 8;

/// A whole program: the entry chunk plus the user functions (index = chunk index in the VM).
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub main: Chunk,
    pub functions: Vec<(String, Chunk, usize)>,
}

/// True when `bytes` starts with the `.yomc` magic number.
pub fn looks_like_module(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

pub fn encode(module: &Module) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&(OpCode::ALL.len() as u16).to_be_bytes());
    let n = u16::try_from(module.functions.len()).map_err(|_| "too many functions")?;
    out.extend_from_slice(&n.to_be_bytes());
    encode_chunk(&module.main, &mut out)?;
    for (name, chunk, arity) in &module.functions {
        let len = u16::try_from(name.len()).map_err(|_| "function name too long")?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(u8::try_from(*arity).map_err(|_| "too many parameters")?);
        encode_chunk(chunk, &mut out)?;
    }
    Ok(out)
}

fn encode_chunk(chunk: &Chunk, out: &mut Vec<u8>) -> Result<(), String> {
    if chunk.code.len() != chunk.lines.len() {
        return Err("chunk line table does not match its code".to_string());
    }
    let len = u32::try_from(chunk.code.len()).map_err(|_| "chunk too large")?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&chunk.code);
    for line in &chunk.lines {
        let l = u32::try_from(*line).map_err(|_| "line number too large")?;
        out.extend_from_slice(&l.to_be_bytes());
    }
    let n = u16::try_from(chunk.constants.len()).map_err(|_| "too many constants")?;
    out.extend_from_slice(&n.to_be_bytes());
    for c in &chunk.constants {
        encode_const(c, out)?;
    }
    Ok(())
}

fn encode_const(v: &Value, out: &mut Vec<u8>) -> Result<(), String> {
    match v {
        Value::Int(i) => {
            out.push(0);
            out.extend_from_slice(&i.to_be_bytes());
        }
        Value::Float(f) => {
            out.push(1);
            out.extend_from_slice(&f.to_bits().to_be_bytes());
        }
        Value::Text(s) => {
            out.push(2);
            let len = u32::try_from(s.len()).map_err(|_| "text constant too large")?;
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        Value::Bool(b) => {
            out.push(3);
            out.push(u8::from(*b));
        }
        Value::Unit => out.push(4),
        Value::Array(items) => {
            out.push(5);
            let len = u32::try_from(items.len()).map_err(|_| "array constant too large")?;
            out.extend_from_slice(&len.to_be_bytes());
            for i in items {
                encode_const(i, out)?;
            }
        }
        other => return Err(format!("a {other:?} cannot be stored as a constant")),
    }
    Ok(())
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn left(&self) -> usize {
        self.b.len() - self.at
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if n > self.left() {
            return Err(format!(
                "file is truncated (needed {n} more bytes at offset {})",
                self.at
            ));
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn u64(&mut self) -> Result<u64, String> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(u64::from_be_bytes(a))
    }
}

pub fn decode(bytes: &[u8]) -> Result<Module, String> {
    let mut r = Reader { b: bytes, at: 0 };
    if r.take(4)
        .map_err(|_| "not a YOMNOVA bytecode file (too short)".to_string())?
        != MAGIC
    {
        return Err("not a YOMNOVA bytecode file (bad magic number)".to_string());
    }
    let version = r.u16()?;
    if version != FORMAT_VERSION {
        return Err(format!(
            "unsupported bytecode format version {version} (this build reads version {FORMAT_VERSION})"
        ));
    }
    let ops = r.u16()? as usize;
    if ops != OpCode::ALL.len() {
        return Err(format!(
            "file was built for {ops} opcodes but this build has {}; rebuild it from source",
            OpCode::ALL.len()
        ));
    }
    let nfn = r.u16()? as usize;
    if nfn > MAX_FUNCTIONS {
        return Err(format!("too many functions ({nfn})"));
    }
    let main = decode_chunk(&mut r)?;
    let mut functions = Vec::with_capacity(nfn.min(r.left()));
    for _ in 0..nfn {
        let len = r.u16()? as usize;
        let name = std::str::from_utf8(r.take(len)?)
            .map_err(|_| "function name is not valid UTF-8".to_string())?
            .to_string();
        let arity = r.u8()? as usize;
        functions.push((name, decode_chunk(&mut r)?, arity));
    }
    if r.left() != 0 {
        return Err(format!(
            "{} unexpected trailing bytes after the program",
            r.left()
        ));
    }
    Ok(Module { main, functions })
}

fn decode_chunk(r: &mut Reader) -> Result<Chunk, String> {
    let len = r.u32()? as usize;
    if len > MAX_CODE_BYTES {
        return Err(format!(
            "chunk of {len} bytes exceeds the {MAX_CODE_BYTES} byte limit"
        ));
    }
    let code = r.take(len)?.to_vec();
    if r.left() < len.saturating_mul(4) {
        return Err("file is truncated (line table)".to_string());
    }
    let mut lines = Vec::with_capacity(len);
    for _ in 0..len {
        lines.push(r.u32()? as usize);
    }
    let n = r.u16()? as usize;
    let mut constants = Vec::with_capacity(n.min(r.left()));
    for _ in 0..n {
        constants.push(decode_const(r, 0)?);
    }
    Ok(Chunk {
        code,
        constants,
        lines,
    })
}

fn decode_const(r: &mut Reader, depth: usize) -> Result<Value, String> {
    if depth > MAX_CONST_DEPTH {
        return Err("constant nesting is too deep".to_string());
    }
    Ok(match r.u8()? {
        0 => Value::Int(r.u64()? as i64),
        1 => Value::Float(f64::from_bits(r.u64()?)),
        2 => {
            let n = r.u32()? as usize;
            if n > MAX_TEXT_BYTES {
                return Err("text constant is too large".to_string());
            }
            let s = std::str::from_utf8(r.take(n)?)
                .map_err(|_| "text constant is not valid UTF-8".to_string())?;
            Value::Text(s.to_string())
        }
        3 => match r.u8()? {
            0 => Value::Bool(false),
            1 => Value::Bool(true),
            other => return Err(format!("invalid Bool byte {other}")),
        },
        4 => Value::Unit,
        5 => {
            let n = r.u32()? as usize;
            if n > MAX_ARRAY_CONST {
                return Err("array constant is too large".to_string());
            }
            let mut items = Vec::with_capacity(n.min(r.left()));
            for _ in 0..n {
                items.push(decode_const(r, depth + 1)?);
            }
            Value::Array(items)
        }
        other => return Err(format!("unknown constant tag {other}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Module {
        let mut main = Chunk::new();
        main.write_op(OpCode::Constant, 1);
        main.write_u16(0, 1);
        main.write_op(OpCode::Emit, 2);
        main.write_op(OpCode::Halt, 3);
        main.constants.push(Value::Text("héllo".to_string()));
        main.constants.push(Value::Array(vec![
            Value::Int(-7),
            Value::Float(2.5),
            Value::Bool(true),
            Value::Unit,
            Value::Array(vec![Value::Text("x".to_string())]),
        ]));
        let mut f = Chunk::new();
        f.write_op(OpCode::Nil, 5);
        f.write_op(OpCode::Return, 5);
        Module {
            main,
            functions: vec![("f".to_string(), f, 2)],
        }
    }

    #[test]
    fn round_trip_is_exact() {
        let m = sample();
        assert_eq!(decode(&encode(&m).unwrap()).unwrap(), m);
    }

    #[test]
    fn every_truncation_is_an_error_not_a_panic() {
        let bytes = encode(&sample()).unwrap();
        for n in 0..bytes.len() {
            assert!(
                decode(&bytes[..n]).is_err(),
                "prefix of {n} bytes was accepted"
            );
        }
    }

    #[test]
    fn rejects_bad_magic_version_opcode_count_and_trailing_bytes() {
        let good = encode(&sample()).unwrap();
        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(decode(&bad).unwrap_err().contains("magic"));
        let mut bad = good.clone();
        bad[5] = 99;
        assert!(decode(&bad).unwrap_err().contains("version"));
        let mut bad = good.clone();
        bad[7] ^= 1;
        assert!(decode(&bad).unwrap_err().contains("opcodes"));
        let mut bad = good;
        bad.push(0);
        assert!(decode(&bad).unwrap_err().contains("trailing"));
    }

    #[test]
    fn huge_claimed_lengths_do_not_allocate_or_panic() {
        // magic, version, opcode count, 0 functions, then a chunk claiming u32::MAX code bytes.
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        b.extend_from_slice(&(OpCode::ALL.len() as u16).to_be_bytes());
        b.extend_from_slice(&0u16.to_be_bytes());
        b.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&b).is_err());
    }

    #[test]
    fn unsupported_constants_are_not_encodable() {
        let mut m = sample();
        m.main
            .constants
            .push(Value::NativeFunction("len".to_string()));
        assert!(encode(&m).is_err());
    }
}
