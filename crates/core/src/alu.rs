//! ALU 纯函数（core 通道，T11；PLAN §3.2）。
//!
//! 约定：所有函数都是纯函数，Flags 进 Flags 出；不涉及的标志位原样返回。
//! 语义依据 `refs/gbz80.7.txt` 与 Pan Docs `CPU_Registers_and_Flags.md`：
//! - INC/DEC 保留 C；
//! - ADC/SBC 的 H 把进位/借位一并算入低 4 位；
//! - `ADD SP,e` 与 `LD HL,SP+e` 的 H/C 按 SP 低字节与 e（视为无符号字节）相加计算，Z=N=0；
//! - BIT 置 H=1、N=0，C 不变；
//! - F 的低 4 位恒为 0（`Flags::to_u8` 只产生高 4 位，`Flags::from_u8` 忽略低 4 位，POP AF 由此清零低 4 位）。

/// Z N H C 四个标志位（F 的 bit7..bit4）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl Flags {
    pub const Z: u8 = 0x80;
    pub const N: u8 = 0x40;
    pub const H: u8 = 0x20;
    pub const C: u8 = 0x10;

    /// 由 F 寄存器的值构造；低 4 位被忽略。
    #[inline(always)]
    pub const fn from_u8(f: u8) -> Self {
        Self {
            z: f & Self::Z != 0,
            n: f & Self::N != 0,
            h: f & Self::H != 0,
            c: f & Self::C != 0,
        }
    }

    /// 转成 F 寄存器的值；低 4 位恒为 0。
    #[inline(always)]
    pub const fn to_u8(self) -> u8 {
        (self.z as u8) << 7 | (self.n as u8) << 6 | (self.h as u8) << 5 | (self.c as u8) << 4
    }

    #[inline(always)]
    pub const fn new(z: bool, n: bool, h: bool, c: bool) -> Self {
        Self { z, n, h, c }
    }
}

/// ADD（`use_carry=false`）/ ADC（`use_carry=true`）：Z 0 H C。
#[inline]
pub fn add8(a: u8, b: u8, f: Flags, use_carry: bool) -> (u8, Flags) {
    let cin = (use_carry && f.c) as u16;
    let sum = a as u16 + b as u16 + cin;
    let r = sum as u8;
    let h = (a & 0x0F) as u16 + (b & 0x0F) as u16 + cin > 0x0F;
    (r, Flags::new(r == 0, false, h, sum > 0xFF))
}

/// SUB（`use_carry=false`）/ SBC（`use_carry=true`）；CP 用它并丢弃结果：Z 1 H C。
#[inline]
pub fn sub8(a: u8, b: u8, f: Flags, use_carry: bool) -> (u8, Flags) {
    let cin = (use_carry && f.c) as u16;
    let r = (a as u16).wrapping_sub(b as u16).wrapping_sub(cin) as u8;
    let h = ((a & 0x0F) as u16) < (b & 0x0F) as u16 + cin;
    let c = (a as u16) < b as u16 + cin;
    (r, Flags::new(r == 0, true, h, c))
}

/// AND：Z 0 1 0。
#[inline]
pub fn and8(a: u8, b: u8, _f: Flags) -> (u8, Flags) {
    let r = a & b;
    (r, Flags::new(r == 0, false, true, false))
}

/// XOR：Z 0 0 0。
#[inline]
pub fn xor8(a: u8, b: u8, _f: Flags) -> (u8, Flags) {
    let r = a ^ b;
    (r, Flags::new(r == 0, false, false, false))
}

/// OR：Z 0 0 0。
#[inline]
pub fn or8(a: u8, b: u8, _f: Flags) -> (u8, Flags) {
    let r = a | b;
    (r, Flags::new(r == 0, false, false, false))
}

/// CP：与 SUB 相同的标志位，结果丢弃（返回值只有 Flags）。
#[inline]
pub fn cp8(a: u8, b: u8, f: Flags) -> Flags {
    sub8(a, b, f, false).1
}

/// INC r8 / INC (HL)：Z 0 H -（保留 C）。
#[inline]
pub fn inc8(v: u8, f: Flags) -> (u8, Flags) {
    let r = v.wrapping_add(1);
    (r, Flags::new(r == 0, false, v & 0x0F == 0x0F, f.c))
}

/// DEC r8 / DEC (HL)：Z 1 H -（保留 C）。
#[inline]
pub fn dec8(v: u8, f: Flags) -> (u8, Flags) {
    let r = v.wrapping_sub(1);
    (r, Flags::new(r == 0, true, v & 0x0F == 0x00, f.c))
}

/// DAA（gbz80(7) 的算法）：Z - 0 C。
#[inline]
pub fn daa(a: u8, f: Flags) -> (u8, Flags) {
    let mut adj = 0u8;
    let mut c = f.c;
    let r = if f.n {
        if f.h {
            adj |= 0x06;
        }
        if f.c {
            adj |= 0x60;
        }
        a.wrapping_sub(adj)
    } else {
        if f.h || a & 0x0F > 0x09 {
            adj |= 0x06;
        }
        if f.c || a > 0x99 {
            adj |= 0x60;
            c = true;
        }
        a.wrapping_add(adj)
    };
    (r, Flags::new(r == 0, f.n, false, c))
}

/// CPL：- 1 1 -。
#[inline]
pub fn cpl(a: u8, f: Flags) -> (u8, Flags) {
    (!a, Flags::new(f.z, true, true, f.c))
}

/// SCF：- 0 0 1。
#[inline]
pub fn scf(f: Flags) -> Flags {
    Flags::new(f.z, false, false, true)
}

/// CCF：- 0 0 !C。
#[inline]
pub fn ccf(f: Flags) -> Flags {
    Flags::new(f.z, false, false, !f.c)
}

/// RLC：Z 0 0 C（C = 原 bit7）。
#[inline]
pub fn rlc(v: u8, _f: Flags) -> (u8, Flags) {
    let r = v.rotate_left(1);
    (r, Flags::new(r == 0, false, false, v & 0x80 != 0))
}

/// RRC：Z 0 0 C（C = 原 bit0）。
#[inline]
pub fn rrc(v: u8, _f: Flags) -> (u8, Flags) {
    let r = v.rotate_right(1);
    (r, Flags::new(r == 0, false, false, v & 0x01 != 0))
}

/// RL（经过 C 左移）：Z 0 0 C。
#[inline]
pub fn rl(v: u8, f: Flags) -> (u8, Flags) {
    let r = v << 1 | f.c as u8;
    (r, Flags::new(r == 0, false, false, v & 0x80 != 0))
}

/// RR（经过 C 右移）：Z 0 0 C。
#[inline]
pub fn rr(v: u8, f: Flags) -> (u8, Flags) {
    let r = v >> 1 | (f.c as u8) << 7;
    (r, Flags::new(r == 0, false, false, v & 0x01 != 0))
}

/// SLA：Z 0 0 C。
#[inline]
pub fn sla(v: u8, _f: Flags) -> (u8, Flags) {
    let r = v << 1;
    (r, Flags::new(r == 0, false, false, v & 0x80 != 0))
}

/// SRA（bit7 不变）：Z 0 0 C。
#[inline]
pub fn sra(v: u8, _f: Flags) -> (u8, Flags) {
    let r = v >> 1 | v & 0x80;
    (r, Flags::new(r == 0, false, false, v & 0x01 != 0))
}

/// SRL：Z 0 0 C。
#[inline]
pub fn srl(v: u8, _f: Flags) -> (u8, Flags) {
    let r = v >> 1;
    (r, Flags::new(r == 0, false, false, v & 0x01 != 0))
}

/// SWAP：Z 0 0 0。
#[inline]
pub fn swap(v: u8, _f: Flags) -> (u8, Flags) {
    let r = v.rotate_left(4);
    (r, Flags::new(r == 0, false, false, false))
}

/// RLCA：与 RLC 相同但 Z=0。
#[inline]
pub fn rlca(a: u8, f: Flags) -> (u8, Flags) {
    let (r, f) = rlc(a, f);
    (r, Flags { z: false, ..f })
}

/// RRCA：与 RRC 相同但 Z=0。
#[inline]
pub fn rrca(a: u8, f: Flags) -> (u8, Flags) {
    let (r, f) = rrc(a, f);
    (r, Flags { z: false, ..f })
}

/// RLA：与 RL 相同但 Z=0。
#[inline]
pub fn rla(a: u8, f: Flags) -> (u8, Flags) {
    let (r, f) = rl(a, f);
    (r, Flags { z: false, ..f })
}

/// RRA：与 RR 相同但 Z=0。
#[inline]
pub fn rra(a: u8, f: Flags) -> (u8, Flags) {
    let (r, f) = rr(a, f);
    (r, Flags { z: false, ..f })
}

/// BIT b：Z 0 1 -（Z = 该位为 0）。
#[inline]
pub fn bit(b: u8, v: u8, f: Flags) -> Flags {
    Flags::new(v & (1 << (b & 7)) == 0, false, true, f.c)
}

/// RES b：不影响标志位。
#[inline]
pub fn res(b: u8, v: u8) -> u8 {
    v & !(1 << (b & 7))
}

/// SET b：不影响标志位。
#[inline]
pub fn set(b: u8, v: u8) -> u8 {
    v | 1 << (b & 7)
}

/// ADD HL,rr：- 0 H C（H = bit11 进位，C = bit15 进位）。
#[inline]
pub fn add16_hl(hl: u16, v: u16, f: Flags) -> (u16, Flags) {
    let sum = hl as u32 + v as u32;
    let h = (hl & 0x0FFF) + (v & 0x0FFF) > 0x0FFF;
    (sum as u16, Flags::new(f.z, false, h, sum > 0xFFFF))
}

/// ADD SP,e 与 LD HL,SP+e：结果为 SP 加上符号扩展的 e；0 0 H C，
/// H/C 按 SP 低字节与 e（作为无符号字节）相加时 bit3/bit7 的进位计算。
#[inline]
pub fn add_sp_e(sp: u16, e: u8, _f: Flags) -> (u16, Flags) {
    let r = sp.wrapping_add(e as i8 as i16 as u16);
    let lo = sp & 0x00FF;
    let h = (lo & 0x0F) + (e as u16 & 0x0F) > 0x0F;
    let c = lo + e as u16 > 0xFF;
    (r, Flags::new(false, false, h, c))
}
