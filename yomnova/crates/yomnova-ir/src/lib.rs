//! Intermediate Representation (IR) with Basic Blocks and 3-Address Instructions.

use std::fmt;
use yomnova_ast::BinaryOp;

pub type VarId = usize;
pub type BlockId = usize;

#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<VarId>,
    pub blocks: Vec<BasicBlock>,
    pub next_var: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    LoadConst { dest: VarId, val: Constant },
    Move { dest: VarId, src: VarId },
    Binary {
        dest: VarId,
        op: BinaryOp,
        left: VarId,
        right: VarId,
    },
    Call {
        dest: VarId,
        func: String,
        args: Vec<VarId>,
    },
    Emit { src: VarId },
    ArrayCreate { dest: VarId, elements: Vec<VarId> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Jump(BlockId),
    BranchIf {
        cond: VarId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<VarId>),
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for func in &self.functions {
            writeln!(f, "fn {}() {{", func.name)?;
            for block in &func.blocks {
                writeln!(f, "  bb{}:", block.id)?;
                for inst in &block.instructions {
                    match inst {
                        Instruction::LoadConst { dest, val } => {
                            writeln!(f, "    v{dest} = const {val:?}")?;
                        }
                        Instruction::Move { dest, src } => {
                            writeln!(f, "    v{dest} = v{src}")?;
                        }
                        Instruction::Binary { dest, op, left, right } => {
                            writeln!(f, "    v{dest} = v{left} {op:?} v{right}")?;
                        }
                        Instruction::Call { dest, func, args } => {
                            let args_str: Vec<String> = args.iter().map(|a| format!("v{a}")).collect();
                            writeln!(f, "    v{dest} = call {func}({})", args_str.join(", "))?;
                        }
                        Instruction::Emit { src } => {
                            writeln!(f, "    emit v{src}")?;
                        }
                        Instruction::ArrayCreate { dest, elements } => {
                            let elem_str: Vec<String> = elements.iter().map(|e| format!("v{e}")).collect();
                            writeln!(f, "    v{dest} = [{}]", elem_str.join(", "))?;
                        }
                    }
                }
                match &block.terminator {
                    Terminator::Jump(target) => writeln!(f, "    jmp bb{target}")?,
                    Terminator::BranchIf { cond, then_block, else_block } => {
                        writeln!(f, "    br_if v{cond} -> bb{then_block} else bb{else_block}")?;
                    }
                    Terminator::Return(Some(v)) => writeln!(f, "    ret v{v}")?,
                    Terminator::Return(None) => writeln!(f, "    ret")?,
                }
            }
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}
