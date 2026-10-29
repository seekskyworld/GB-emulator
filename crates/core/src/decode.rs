//! 指令译码（core 通道，T10；PLAN §8 T10、Pan Docs `CPU_Instruction_Set.md`）。
//!
//! 按 `x = op>>6, y = (op>>3)&7, z = op&7, p = y>>1, q = y&1` 分解，配合
//! r / rp / rp2 / cc / alu / rot 表把操作码译成 [`Instr`]；CB 前缀由 [`decode_cb`] 单独译码。
//! 只描述"做什么"，M-cycle 序列在 `cpu.rs` 中实现。

/// 无前缀的 11 个非法操作码；执行后 CPU 锁死。
pub const ILLEGAL: [u8; 11] = [0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD];

/// 操作码的 x/y/z/p/q 字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fields {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub p: u8,
    pub q: u8,
}

impl Fields {
    #[inline(always)]
    pub const fn of(op: u8) -> Self {
        let y = (op >> 3) & 7;
        Self { x: op >> 6, y, z: op & 7, p: y >> 1, q: y & 1 }
    }
}

/// r 表：`B C D E H L (HL) A`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum R8 {
    B,
    C,
    D,
    E,
    H,
    L,
    /// `(HL)`：内存操作数，访问消耗 1 个 M-cycle。
    HlInd,
    A,
}

impl R8 {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 7 {
            0 => R8::B,
            1 => R8::C,
            2 => R8::D,
            3 => R8::E,
            4 => R8::H,
            5 => R8::L,
            6 => R8::HlInd,
            _ => R8::A,
        }
    }
}

/// rp 表：`BC DE HL SP`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rp {
    BC,
    DE,
    HL,
    SP,
}

impl Rp {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 3 {
            0 => Rp::BC,
            1 => Rp::DE,
            2 => Rp::HL,
            _ => Rp::SP,
        }
    }
}

/// rp2 表（PUSH/POP）：`BC DE HL AF`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rp2 {
    BC,
    DE,
    HL,
    AF,
}

impl Rp2 {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 3 {
            0 => Rp2::BC,
            1 => Rp2::DE,
            2 => Rp2::HL,
            _ => Rp2::AF,
        }
    }
}

/// 间接寻址表（`LD (rr),A` / `LD A,(rr)`）：`(BC) (DE) (HL+) (HL-)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpMem {
    BC,
    DE,
    HLI,
    HLD,
}

impl RpMem {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 3 {
            0 => RpMem::BC,
            1 => RpMem::DE,
            2 => RpMem::HLI,
            _ => RpMem::HLD,
        }
    }
}

/// cc 表：`NZ Z NC C`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cc {
    NZ,
    Z,
    NC,
    C,
}

impl Cc {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 3 {
            0 => Cc::NZ,
            1 => Cc::Z,
            2 => Cc::NC,
            _ => Cc::C,
        }
    }

    /// 对 F 寄存器求值。
    #[inline(always)]
    pub const fn eval(self, f: u8) -> bool {
        match self {
            Cc::NZ => f & 0x80 == 0,
            Cc::Z => f & 0x80 != 0,
            Cc::NC => f & 0x10 == 0,
            Cc::C => f & 0x10 != 0,
        }
    }
}

/// alu 表：`ADD ADC SUB SBC AND XOR OR CP`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp {
    Add,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Cp,
}

impl AluOp {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 7 {
            0 => AluOp::Add,
            1 => AluOp::Adc,
            2 => AluOp::Sub,
            3 => AluOp::Sbc,
            4 => AluOp::And,
            5 => AluOp::Xor,
            6 => AluOp::Or,
            _ => AluOp::Cp,
        }
    }

    pub const fn mnemonic(self) -> &'static str {
        match self {
            AluOp::Add => "ADD",
            AluOp::Adc => "ADC",
            AluOp::Sub => "SUB",
            AluOp::Sbc => "SBC",
            AluOp::And => "AND",
            AluOp::Xor => "XOR",
            AluOp::Or => "OR",
            AluOp::Cp => "CP",
        }
    }
}

/// rot 表（CB 前缀 x=0）：`RLC RRC RL RR SLA SRA SWAP SRL`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotOp {
    Rlc,
    Rrc,
    Rl,
    Rr,
    Sla,
    Sra,
    Swap,
    Srl,
}

impl RotOp {
    #[inline(always)]
    pub const fn from_index(i: u8) -> Self {
        match i & 7 {
            0 => RotOp::Rlc,
            1 => RotOp::Rrc,
            2 => RotOp::Rl,
            3 => RotOp::Rr,
            4 => RotOp::Sla,
            5 => RotOp::Sra,
            6 => RotOp::Swap,
            _ => RotOp::Srl,
        }
    }

    pub const fn mnemonic(self) -> &'static str {
        match self {
            RotOp::Rlc => "RLC",
            RotOp::Rrc => "RRC",
            RotOp::Rl => "RL",
            RotOp::Rr => "RR",
            RotOp::Sla => "SLA",
            RotOp::Sra => "SRA",
            RotOp::Swap => "SWAP",
            RotOp::Srl => "SRL",
        }
    }
}

/// 无前缀指令。注释中的 `n`/`nn`/`e` 分别是 8 位立即数、16 位立即数、有符号 8 位偏移。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    Nop,
    /// `LD (nn),SP`
    LdImmSp,
    Stop,
    /// `JR e`
    Jr,
    /// `JR cc,e`
    JrCc(Cc),
    /// `LD rp,nn`
    LdRpImm(Rp),
    /// `ADD HL,rp`
    AddHlRp(Rp),
    /// `LD (rr),A`
    LdMemA(RpMem),
    /// `LD A,(rr)`
    LdAMem(RpMem),
    IncRp(Rp),
    DecRp(Rp),
    Inc8(R8),
    Dec8(R8),
    /// `LD r,n`
    LdR8Imm(R8),
    Rlca,
    Rrca,
    Rla,
    Rra,
    Daa,
    Cpl,
    Scf,
    Ccf,
    Halt,
    /// `LD dst,src`
    Ld8(R8, R8),
    /// `alu A,r`
    Alu(AluOp, R8),
    /// `alu A,n`
    AluImm(AluOp),
    RetCc(Cc),
    /// `LDH (n),A`
    LdhImmA,
    /// `ADD SP,e`
    AddSpE,
    /// `LDH A,(n)`
    LdhAImm,
    /// `LD HL,SP+e`
    LdHlSpE,
    Pop(Rp2),
    Ret,
    Reti,
    JpHl,
    LdSpHl,
    /// `JP cc,nn`
    JpCc(Cc),
    /// `LDH (C),A`
    LdhCA,
    /// `LD (nn),A`
    LdImmA,
    /// `LDH A,(C)`
    LdhAC,
    /// `LD A,(nn)`
    LdAImm,
    /// `JP nn`
    Jp,
    /// CB 前缀：第二个字节由 [`decode_cb`] 译码。
    Prefix,
    Di,
    Ei,
    /// `CALL cc,nn`
    CallCc(Cc),
    Push(Rp2),
    /// `CALL nn`
    Call,
    /// `RST vec`，参数为目标地址（0x00..=0x38）。
    Rst(u8),
    /// 非法操作码：CPU 锁死。
    Illegal,
}

impl Instr {
    /// 助记符，与 `tests/data/Opcodes.json` 的 `mnemonic` 一致（非法操作码为 `ILLEGAL`）。
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Instr::Nop => "NOP",
            Instr::LdImmSp
            | Instr::LdRpImm(_)
            | Instr::LdMemA(_)
            | Instr::LdAMem(_)
            | Instr::LdR8Imm(_)
            | Instr::Ld8(..)
            | Instr::LdHlSpE
            | Instr::LdSpHl
            | Instr::LdImmA
            | Instr::LdAImm => "LD",
            Instr::LdhImmA | Instr::LdhAImm | Instr::LdhCA | Instr::LdhAC => "LDH",
            Instr::Stop => "STOP",
            Instr::Jr | Instr::JrCc(_) => "JR",
            Instr::AddHlRp(_) | Instr::AddSpE => "ADD",
            Instr::IncRp(_) | Instr::Inc8(_) => "INC",
            Instr::DecRp(_) | Instr::Dec8(_) => "DEC",
            Instr::Rlca => "RLCA",
            Instr::Rrca => "RRCA",
            Instr::Rla => "RLA",
            Instr::Rra => "RRA",
            Instr::Daa => "DAA",
            Instr::Cpl => "CPL",
            Instr::Scf => "SCF",
            Instr::Ccf => "CCF",
            Instr::Halt => "HALT",
            Instr::Alu(op, _) | Instr::AluImm(op) => op.mnemonic(),
            Instr::RetCc(_) | Instr::Ret => "RET",
            Instr::Reti => "RETI",
            Instr::Pop(_) => "POP",
            Instr::Push(_) => "PUSH",
            Instr::JpHl | Instr::JpCc(_) | Instr::Jp => "JP",
            Instr::CallCc(_) | Instr::Call => "CALL",
            Instr::Rst(_) => "RST",
            Instr::Prefix => "PREFIX",
            Instr::Di => "DI",
            Instr::Ei => "EI",
            Instr::Illegal => "ILLEGAL",
        }
    }
}

/// CB 前缀指令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbInstr {
    Rot(RotOp, R8),
    Bit(u8, R8),
    Res(u8, R8),
    Set(u8, R8),
}

impl CbInstr {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            CbInstr::Rot(op, _) => op.mnemonic(),
            CbInstr::Bit(..) => "BIT",
            CbInstr::Res(..) => "RES",
            CbInstr::Set(..) => "SET",
        }
    }
}

/// 是否为 11 个非法操作码之一。
#[inline(always)]
pub const fn is_illegal(op: u8) -> bool {
    matches!(op, 0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD)
}

/// 译码无前缀操作码。
#[inline(always)]
pub const fn decode(op: u8) -> Instr {
    let Fields { x, y, z, p, q } = Fields::of(op);
    match x {
        0 => match z {
            0 => match y {
                0 => Instr::Nop,
                1 => Instr::LdImmSp,
                2 => Instr::Stop,
                3 => Instr::Jr,
                _ => Instr::JrCc(Cc::from_index(y - 4)),
            },
            1 => {
                if q == 0 {
                    Instr::LdRpImm(Rp::from_index(p))
                } else {
                    Instr::AddHlRp(Rp::from_index(p))
                }
            }
            2 => {
                if q == 0 {
                    Instr::LdMemA(RpMem::from_index(p))
                } else {
                    Instr::LdAMem(RpMem::from_index(p))
                }
            }
            3 => {
                if q == 0 {
                    Instr::IncRp(Rp::from_index(p))
                } else {
                    Instr::DecRp(Rp::from_index(p))
                }
            }
            4 => Instr::Inc8(R8::from_index(y)),
            5 => Instr::Dec8(R8::from_index(y)),
            6 => Instr::LdR8Imm(R8::from_index(y)),
            _ => match y {
                0 => Instr::Rlca,
                1 => Instr::Rrca,
                2 => Instr::Rla,
                3 => Instr::Rra,
                4 => Instr::Daa,
                5 => Instr::Cpl,
                6 => Instr::Scf,
                _ => Instr::Ccf,
            },
        },
        1 => {
            if y == 6 && z == 6 {
                Instr::Halt
            } else {
                Instr::Ld8(R8::from_index(y), R8::from_index(z))
            }
        }
        2 => Instr::Alu(AluOp::from_index(y), R8::from_index(z)),
        _ => match z {
            0 => match y {
                0..=3 => Instr::RetCc(Cc::from_index(y)),
                4 => Instr::LdhImmA,
                5 => Instr::AddSpE,
                6 => Instr::LdhAImm,
                _ => Instr::LdHlSpE,
            },
            1 => {
                if q == 0 {
                    Instr::Pop(Rp2::from_index(p))
                } else {
                    match p {
                        0 => Instr::Ret,
                        1 => Instr::Reti,
                        2 => Instr::JpHl,
                        _ => Instr::LdSpHl,
                    }
                }
            }
            2 => match y {
                0..=3 => Instr::JpCc(Cc::from_index(y)),
                4 => Instr::LdhCA,
                5 => Instr::LdImmA,
                6 => Instr::LdhAC,
                _ => Instr::LdAImm,
            },
            3 => match y {
                0 => Instr::Jp,
                1 => Instr::Prefix,
                6 => Instr::Di,
                7 => Instr::Ei,
                _ => Instr::Illegal,
            },
            4 => match y {
                0..=3 => Instr::CallCc(Cc::from_index(y)),
                _ => Instr::Illegal,
            },
            5 => {
                if q == 0 {
                    Instr::Push(Rp2::from_index(p))
                } else if p == 0 {
                    Instr::Call
                } else {
                    Instr::Illegal
                }
            }
            6 => Instr::AluImm(AluOp::from_index(y)),
            _ => Instr::Rst(y * 8),
        },
    }
}

/// 译码 CB 前缀后的第二个字节。
#[inline(always)]
pub const fn decode_cb(op: u8) -> CbInstr {
    let Fields { x, y, z, .. } = Fields::of(op);
    let r = R8::from_index(z);
    match x {
        0 => CbInstr::Rot(RotOp::from_index(y), r),
        1 => CbInstr::Bit(y, r),
        2 => CbInstr::Res(y, r),
        _ => CbInstr::Set(y, r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illegal_set_matches_decoder() {
        for op in 0..=255u8 {
            assert_eq!(decode(op) == Instr::Illegal, is_illegal(op), "op {op:02X}");
            assert_eq!(ILLEGAL.contains(&op), is_illegal(op), "op {op:02X}");
        }
    }

    #[test]
    fn spot_checks() {
        assert_eq!(decode(0x76), Instr::Halt);
        assert_eq!(decode(0x40), Instr::Ld8(R8::B, R8::B));
        assert_eq!(decode(0x7E), Instr::Ld8(R8::A, R8::HlInd));
        assert_eq!(decode(0x36), Instr::LdR8Imm(R8::HlInd));
        assert_eq!(decode(0xE9), Instr::JpHl);
        assert_eq!(decode(0xFF), Instr::Rst(0x38));
        assert_eq!(decode(0xF1), Instr::Pop(Rp2::AF));
        assert_eq!(decode(0x3A), Instr::LdAMem(RpMem::HLD));
        assert_eq!(decode_cb(0x46), CbInstr::Bit(0, R8::HlInd));
        assert_eq!(decode_cb(0x37), CbInstr::Rot(RotOp::Swap, R8::A));
        assert_eq!(decode_cb(0xFE), CbInstr::Set(7, R8::HlInd));
    }
}
