//! Linear intermediate representation with symbolic jump targets, and the optimiser that runs on it.
//!
//! The compiler emits bytecode; [`optimize_chunk`] lifts a chunk into [`Ir`] (jump operands become instruction
//! indices instead of byte offsets), applies a small set of peephole and control-flow optimisations until nothing
//! changes, and lowers the result back to a chunk with recomputed offsets. If anything about a chunk is unusual
//! (an unknown opcode, a jump into the middle of an instruction, code larger than a jump operand can address) the
//! original chunk is returned untouched, so the optimiser can only ever skip work, never fail a build.
//!
//! Optimisations (all preserve observable behaviour, including run-time errors):
//! * constant folding of `Int`/`Bool`/`Text` operations whose result cannot fail (no overflow, no division by zero);
//! * pure value pushed and immediately popped is removed;
//! * conditional jumps on a constant `true`/`false` are removed or made unconditional;
//! * jump threading (a jump to a jump goes straight to the final target), jumps to the next instruction are removed;
//! * unreachable instructions are removed.

use std::collections::HashSet;
use yomnova_bytecode::{Chunk, OpCode, Value};

/// The operand of an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    None,
    /// Constant index, local slot, global name index, array length or field-name index.
    U16(u16),
    /// Argument count of a call.
    Count(u8),
    /// Index of the target instruction.
    Target(usize),
    /// `Struct`: name constant and field-names constant.
    Struct(u16, u16),
    /// `EnumNew`: variant-name constant and whether a value is carried.
    EnumNew(u16, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ins {
    pub op: OpCode,
    pub arg: Arg,
    pub line: usize,
}

/// A chunk's code as a list of instructions plus its (growable) constant pool.
#[derive(Debug, Clone, PartialEq)]
pub struct Ir {
    pub code: Vec<Ins>,
    pub constants: Vec<Value>,
}

/// What the optimiser did, for tests and `--stats`-style reporting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub instructions_before: usize,
    pub instructions_after: usize,
    pub bytes_before: usize,
    pub bytes_after: usize,
    pub rounds: usize,
}

fn operand_width(op: OpCode) -> usize {
    match op {
        OpCode::Constant
        | OpCode::GetLocal
        | OpCode::SetLocal
        | OpCode::GetGlobal
        | OpCode::SetGlobal
        | OpCode::Jump
        | OpCode::JumpIfFalse
        | OpCode::Array
        | OpCode::Field
        | OpCode::EnumIs
        | OpCode::SetField => 2,
        OpCode::Call => 1,
        OpCode::Struct => 4,
        OpCode::EnumNew => 3,
        _ => 0,
    }
}

fn is_jump(op: OpCode) -> bool {
    matches!(op, OpCode::Jump | OpCode::JumpIfFalse)
}

/// Lifts a chunk into IR. `None` if the code is not well-formed.
pub fn lift(chunk: &Chunk) -> Option<Ir> {
    let code = &chunk.code;
    let mut offset_to_index = vec![usize::MAX; code.len() + 1];
    let mut raw: Vec<(OpCode, Arg, usize)> = Vec::new();
    let mut i = 0;
    while i < code.len() {
        let op = OpCode::from_u8(code[i])?;
        let w = operand_width(op);
        if i + 1 + w > code.len() {
            return None;
        }
        let u16_at = |k: usize| ((code[i + 1 + k] as u16) << 8) | code[i + 2 + k] as u16;
        let arg = match op {
            OpCode::Call => Arg::Count(code[i + 1]),
            OpCode::Struct => Arg::Struct(u16_at(0), u16_at(2)),
            OpCode::EnumNew => Arg::EnumNew(u16_at(0), code[i + 3]),
            OpCode::Jump | OpCode::JumpIfFalse => Arg::Target(u16_at(0) as usize), // byte offset for now
            _ if w == 2 => Arg::U16(u16_at(0)),
            _ => Arg::None,
        };
        offset_to_index[i] = raw.len();
        raw.push((op, arg, *chunk.lines.get(i)?));
        i += 1 + w;
    }
    offset_to_index[code.len()] = raw.len();
    let mut out = Vec::with_capacity(raw.len());
    for (op, arg, line) in raw {
        let arg = match arg {
            Arg::Target(off) => {
                let idx = *offset_to_index.get(off)?;
                if idx == usize::MAX {
                    return None;
                }
                Arg::Target(idx)
            }
            other => other,
        };
        out.push(Ins { op, arg, line });
    }
    Some(Ir {
        code: out,
        constants: chunk.constants.clone(),
    })
}

/// Lowers IR back to a chunk. `None` if the code no longer fits the 16-bit jump operands.
pub fn lower(ir: &Ir) -> Option<Chunk> {
    let mut offsets = Vec::with_capacity(ir.code.len() + 1);
    let mut at = 0usize;
    for ins in &ir.code {
        offsets.push(at);
        at += 1 + operand_width(ins.op);
    }
    offsets.push(at);
    if at > u16::MAX as usize || ir.constants.len() > u16::MAX as usize {
        return None;
    }
    let mut code = Vec::with_capacity(at);
    let mut lines = Vec::with_capacity(at);
    for ins in &ir.code {
        let before = code.len();
        code.push(ins.op as u8);
        let push16 = |code: &mut Vec<u8>, v: u16| {
            code.push((v >> 8) as u8);
            code.push((v & 0xFF) as u8);
        };
        match ins.arg {
            Arg::None => {}
            Arg::U16(v) => push16(&mut code, v),
            Arg::Count(c) => code.push(c),
            Arg::Target(t) => push16(&mut code, *offsets.get(t)? as u16),
            Arg::Struct(a, b) => {
                push16(&mut code, a);
                push16(&mut code, b);
            }
            Arg::EnumNew(a, f) => {
                push16(&mut code, a);
                code.push(f);
            }
        }
        lines.resize(code.len(), ins.line);
        debug_assert!(code.len() > before);
    }
    Some(Chunk {
        code,
        constants: ir.constants.clone(),
        lines,
    })
}

/// Optimises a chunk. Returns the original chunk (and zeroed stats) when it cannot be lifted or lowered.
pub fn optimize_chunk(chunk: &Chunk) -> (Chunk, Stats) {
    let Some(mut ir) = lift(chunk) else {
        return (chunk.clone(), Stats::default());
    };
    let before = ir.code.len();
    let mut rounds = 0;
    while rounds < 16 {
        rounds += 1;
        let changed =
            fold_and_peephole(&mut ir) | thread_jumps(&mut ir) | remove_unreachable(&mut ir);
        if !changed {
            break;
        }
    }
    match lower(&ir) {
        Some(out) => {
            let stats = Stats {
                instructions_before: before,
                instructions_after: ir.code.len(),
                bytes_before: chunk.code.len(),
                bytes_after: out.code.len(),
                rounds,
            };
            (out, stats)
        }
        None => (chunk.clone(), Stats::default()),
    }
}

// ---- passes ----

/// Rebuilds the code from `replacements`: `(old_start, old_len, new instructions)`, sorted and non-overlapping.
/// Jump targets that pointed at a replaced region move to its replacement (or to whatever follows it).
fn apply(ir: &mut Ir, replacements: Vec<(usize, usize, Vec<Ins>)>) {
    let n = ir.code.len();
    let mut map = vec![0usize; n + 1];
    let mut out: Vec<Ins> = Vec::with_capacity(n);
    let mut i = 0;
    let mut r = replacements.into_iter().peekable();
    while i < n {
        if r.peek().is_some_and(|(start, _, _)| *start == i) {
            let (_, len, new) = r.next().unwrap();
            for slot in &mut map[i..i + len] {
                *slot = out.len();
            }
            out.extend(new);
            i += len;
        } else {
            map[i] = out.len();
            out.push(ir.code[i]);
            i += 1;
        }
    }
    map[n] = out.len();
    for ins in &mut out {
        if let Arg::Target(t) = ins.arg {
            ins.arg = Arg::Target(map[t]);
        }
    }
    ir.code = out;
}

fn targets(ir: &Ir) -> HashSet<usize> {
    ir.code
        .iter()
        .filter_map(|i| match i.arg {
            Arg::Target(t) if is_jump(i.op) => Some(t),
            _ => None,
        })
        .collect()
}

fn const_int(ir: &Ir, ins: &Ins) -> Option<i64> {
    match (ins.op, ins.arg) {
        (OpCode::Constant, Arg::U16(k)) => match ir.constants.get(k as usize)? {
            Value::Int(n) => Some(*n),
            _ => None,
        },
        _ => None,
    }
}

fn const_text<'a>(ir: &'a Ir, ins: &Ins) -> Option<&'a str> {
    match (ins.op, ins.arg) {
        (OpCode::Constant, Arg::U16(k)) => match ir.constants.get(k as usize)? {
            Value::Text(s) => Some(s),
            _ => None,
        },
        _ => None,
    }
}

fn const_bool(ins: &Ins) -> Option<bool> {
    match ins.op {
        OpCode::True => Some(true),
        OpCode::False => Some(false),
        _ => None,
    }
}

/// Pure pushes: no side effect and they cannot fail, so `push; pop` does nothing.
fn is_pure_push(op: OpCode) -> bool {
    matches!(
        op,
        OpCode::Constant | OpCode::Nil | OpCode::True | OpCode::False | OpCode::GetLocal
    )
}

fn intern(ir: &mut Ir, v: Value) -> Option<u16> {
    if let Some(p) = ir.constants.iter().position(|c| *c == v) {
        return Some(p as u16);
    }
    if ir.constants.len() >= u16::MAX as usize {
        return None;
    }
    ir.constants.push(v);
    Some((ir.constants.len() - 1) as u16)
}

const MAX_FOLDED_TEXT: usize = 4096;

fn fold_and_peephole(ir: &mut Ir) -> bool {
    let is_target = targets(ir);
    let n = ir.code.len();
    let mut repl: Vec<(usize, usize, Vec<Ins>)> = Vec::new();
    let mut i = 0;
    while i < n {
        let a = ir.code[i];
        // Window of three: [push c1][push c2][binary op]; the 2nd and 3rd must not be jump targets.
        if i + 2 < n && !is_target.contains(&(i + 1)) && !is_target.contains(&(i + 2)) {
            let (b, c) = (ir.code[i + 1], ir.code[i + 2]);
            let line = c.line;
            let folded: Option<Value> =
                if let (Some(x), Some(y)) = (const_int(ir, &a), const_int(ir, &b)) {
                    match c.op {
                        OpCode::Add => x.checked_add(y).map(Value::Int),
                        OpCode::Subtract => x.checked_sub(y).map(Value::Int),
                        OpCode::Multiply => x.checked_mul(y).map(Value::Int),
                        OpCode::Divide => x.checked_div(y).map(Value::Int),
                        OpCode::Modulo => x.checked_rem(y).map(Value::Int),
                        OpCode::Equal => Some(Value::Bool(x == y)),
                        OpCode::NotEqual => Some(Value::Bool(x != y)),
                        OpCode::Greater => Some(Value::Bool(x > y)),
                        OpCode::GreaterEqual => Some(Value::Bool(x >= y)),
                        OpCode::Less => Some(Value::Bool(x < y)),
                        OpCode::LessEqual => Some(Value::Bool(x <= y)),
                        _ => None,
                    }
                } else if let (Some(x), Some(y)) = (const_text(ir, &a), const_text(ir, &b)) {
                    match c.op {
                        OpCode::Add if x.len() + y.len() <= MAX_FOLDED_TEXT => {
                            Some(Value::Text(format!("{x}{y}")))
                        }
                        OpCode::Equal => Some(Value::Bool(x == y)),
                        OpCode::NotEqual => Some(Value::Bool(x != y)),
                        _ => None,
                    }
                } else if let (Some(x), Some(y)) = (const_bool(&a), const_bool(&b)) {
                    match c.op {
                        OpCode::Equal => Some(Value::Bool(x == y)),
                        OpCode::NotEqual => Some(Value::Bool(x != y)),
                        _ => None,
                    }
                } else {
                    None
                };
            if let Some(v) = folded {
                let new = match v {
                    Value::Bool(true) => Some(Ins {
                        op: OpCode::True,
                        arg: Arg::None,
                        line,
                    }),
                    Value::Bool(false) => Some(Ins {
                        op: OpCode::False,
                        arg: Arg::None,
                        line,
                    }),
                    other => intern(ir, other).map(|k| Ins {
                        op: OpCode::Constant,
                        arg: Arg::U16(k),
                        line,
                    }),
                };
                if let Some(new) = new {
                    repl.push((i, 3, vec![new]));
                    i += 3;
                    continue;
                }
            }
        }
        // Window of two.
        if i + 1 < n && !is_target.contains(&(i + 1)) {
            let b = ir.code[i + 1];
            // pure push immediately popped
            if is_pure_push(a.op) && b.op == OpCode::Pop {
                repl.push((i, 2, vec![]));
                i += 2;
                continue;
            }
            // constant condition
            if let (Some(cond), OpCode::JumpIfFalse) = (const_bool(&a), b.op) {
                let new = if cond {
                    vec![]
                } else {
                    vec![Ins {
                        op: OpCode::Jump,
                        arg: b.arg,
                        line: b.line,
                    }]
                };
                repl.push((i, 2, new));
                i += 2;
                continue;
            }
            // unary folds
            if b.op == OpCode::Not {
                if let Some(x) = const_bool(&a) {
                    let op = if x { OpCode::False } else { OpCode::True };
                    repl.push((
                        i,
                        2,
                        vec![Ins {
                            op,
                            arg: Arg::None,
                            line: b.line,
                        }],
                    ));
                    i += 2;
                    continue;
                }
            }
            if b.op == OpCode::Negate {
                if let Some(x) = const_int(ir, &a) {
                    if let Some(neg) = x.checked_neg() {
                        if let Some(k) = intern(ir, Value::Int(neg)) {
                            repl.push((
                                i,
                                2,
                                vec![Ins {
                                    op: OpCode::Constant,
                                    arg: Arg::U16(k),
                                    line: b.line,
                                }],
                            ));
                            i += 2;
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    if repl.is_empty() {
        return false;
    }
    apply(ir, repl);
    true
}

fn thread_jumps(ir: &mut Ir) -> bool {
    let mut changed = false;
    for i in 0..ir.code.len() {
        if !is_jump(ir.code[i].op) {
            continue;
        }
        let Arg::Target(mut t) = ir.code[i].arg else {
            continue;
        };
        let mut hops = 0;
        while t < ir.code.len() && ir.code[t].op == OpCode::Jump && hops < 32 {
            let Arg::Target(next) = ir.code[t].arg else {
                break;
            };
            if next == t {
                break; // a jump to itself: leave it alone
            }
            t = next;
            hops += 1;
        }
        if ir.code[i].arg != Arg::Target(t) {
            ir.code[i].arg = Arg::Target(t);
            changed = true;
        }
    }
    // A jump to the very next instruction does nothing (a conditional one still pops its condition).
    let mut repl = Vec::new();
    for (i, ins) in ir.code.iter().enumerate() {
        if let (true, Arg::Target(t)) = (is_jump(ins.op), ins.arg) {
            if t == i + 1 {
                let new = if ins.op == OpCode::Jump {
                    vec![]
                } else {
                    vec![Ins {
                        op: OpCode::Pop,
                        arg: Arg::None,
                        line: ins.line,
                    }]
                };
                repl.push((i, 1, new));
            }
        }
    }
    if !repl.is_empty() {
        apply(ir, repl);
        changed = true;
    }
    changed
}

fn remove_unreachable(ir: &mut Ir) -> bool {
    let n = ir.code.len();
    let mut seen = vec![false; n];
    let mut work = vec![0usize];
    while let Some(i) = work.pop() {
        if i >= n || seen[i] {
            continue;
        }
        seen[i] = true;
        let ins = ir.code[i];
        match ins.op {
            OpCode::Jump => {
                if let Arg::Target(t) = ins.arg {
                    work.push(t);
                }
            }
            OpCode::JumpIfFalse => {
                if let Arg::Target(t) = ins.arg {
                    work.push(t);
                }
                work.push(i + 1);
            }
            OpCode::Return | OpCode::Halt => {}
            _ => work.push(i + 1),
        }
    }
    if seen.iter().all(|s| *s) {
        return false;
    }
    let mut repl = Vec::new();
    let mut i = 0;
    while i < n {
        if seen[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && !seen[i] {
            i += 1;
        }
        repl.push((start, i - start, vec![]));
    }
    apply(ir, repl);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(code: Vec<u8>, constants: Vec<Value>) -> Chunk {
        let lines = vec![1; code.len()];
        Chunk {
            code,
            constants,
            lines,
        }
    }

    const C: u8 = OpCode::Constant as u8;
    const H: u8 = OpCode::Halt as u8;

    #[test]
    fn lift_and_lower_round_trip() {
        let c = chunk(
            vec![
                C,
                0,
                0,
                OpCode::JumpIfFalse as u8,
                0,
                8,
                OpCode::Nil as u8,
                OpCode::Pop as u8,
                H,
            ],
            vec![Value::Int(1)],
        );
        let ir = lift(&c).unwrap();
        assert_eq!(lower(&ir).unwrap(), c);
    }

    #[test]
    fn folds_constants_and_removes_dead_pushes() {
        // 2 + 3 * 4 compiled as 2 3 4 * +
        let m = OpCode::Multiply as u8;
        let a = OpCode::Add as u8;
        let c = chunk(
            vec![C, 0, 0, C, 0, 1, C, 0, 2, m, a, H],
            vec![Value::Int(2), Value::Int(3), Value::Int(4)],
        );
        let (o, s) = optimize_chunk(&c);
        assert_eq!(o.code.len(), 4, "one Constant + Halt: {:?}", o.code);
        assert_eq!(o.constants[(o.code[2]) as usize], Value::Int(14));
        assert!(s.instructions_after < s.instructions_before);
    }

    #[test]
    fn does_not_fold_what_would_fail_at_run_time() {
        let d = OpCode::Divide as u8;
        let c = chunk(
            vec![C, 0, 0, C, 0, 1, d, H],
            vec![Value::Int(1), Value::Int(0)],
        );
        assert_eq!(
            optimize_chunk(&c).0.code,
            c.code,
            "division by zero must stay a run-time error"
        );
        let a = OpCode::Add as u8;
        let c = chunk(
            vec![C, 0, 0, C, 0, 1, a, H],
            vec![Value::Int(i64::MAX), Value::Int(1)],
        );
        assert_eq!(
            optimize_chunk(&c).0.code,
            c.code,
            "overflow must stay a run-time error"
        );
    }

    #[test]
    fn constant_conditions_and_dead_code() {
        // true ? skip : ... ; the else-part after an unconditional jump becomes unreachable
        let t = OpCode::True as u8;
        let jf = OpCode::JumpIfFalse as u8;
        let j = OpCode::Jump as u8;
        let n = OpCode::Nil as u8;
        // 0:True 1:JumpIfFalse->else(11)  4:Nil 5:Pop 6:Jump->end(14)  9:?? we place else at 9
        let c = chunk(
            vec![t, jf, 0, 9, n, OpCode::Pop as u8, j, 0, 12, C, 0, 0, H],
            vec![Value::Int(7)],
        );
        let (o, _) = optimize_chunk(&c);
        assert_eq!(o.code, vec![H], "{:?}", o.code);
    }

    #[test]
    fn jump_threading_retargets_jumps_to_jumps() {
        let j = OpCode::Jump as u8;
        // 0:Jump->3  3:Jump->6  6:Halt
        let c = chunk(vec![j, 0, 3, j, 0, 6, H], vec![]);
        let (o, _) = optimize_chunk(&c);
        assert_eq!(o.code, vec![H]);
    }

    #[test]
    fn malformed_code_is_left_alone() {
        let c = chunk(vec![C, 0], vec![Value::Int(1)]);
        assert_eq!(optimize_chunk(&c).0, c);
        let c = chunk(
            vec![OpCode::Jump as u8, 0, 1, C, 0, 0, H],
            vec![Value::Int(1)],
        );
        assert_eq!(
            optimize_chunk(&c).0,
            c,
            "jump into the middle of an instruction"
        );
    }
}
