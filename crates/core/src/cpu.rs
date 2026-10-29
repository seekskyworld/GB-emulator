//! SM83 CPU（core 通道，T10/T11/T14）。
//!
//! 冻结部分（D001）：`CpuBus` trait、`Cpu` 的公开字段、`Cpu::new`、`Cpu::step`、`Snapshot for Cpu`。
//! 新增的公开字段 `ei_pending`/`halt_bug`/`locked` 与 `af/bc/de/hl` 访问器不改变冻结签名。
//!
//! 时序模型（PLAN §3.3）：每条指令按真实的 M-cycle 序列调用 `read/write/idle`，每次调用 = 1 M-cycle。
//! `step` 的取指是第 1 个 M-cycle（PC 指向操作码本身），与 SingleStepTests 的约定一致。
//! 内部周期位置：
//! - `PUSH rr`、`RST`：取指后 1 个 idle，再写高字节、低字节；
//! - `CALL nn` / `CALL cc`（成立）：读完 nn 后 1 个 idle，再压栈；
//! - `RET`/`RETI`：弹栈后 1 个 idle；`RET cc`：判断条件 1 个 idle，成立时弹栈后再 1 个 idle；
//! - `JP nn`、`JP cc`（成立）、`JR e`、`JR cc`（成立）：最后 1 个 idle；`JP HL` 没有内部周期；
//! - `ADD HL,rr`、`INC/DEC rr`、`LD SP,HL`：1 个 idle；`ADD SP,e`：2 个 idle；`LD HL,SP+e`：1 个 idle。
//!
//! 访问时点（mem_timing）：`INC/DEC (HL)` 读在第 2、写在第 3 个 M-cycle；CB 前缀 `BIT b,(HL)` 读在第 3 个；
//! 其余 CB `(HL)` 读在第 3、写在第 4 个；`LD (nn),A` 写在第 4 个；`LD A,(nn)` 读在第 4 个。
//!
//! 中断（PLAN §3.3、Pan Docs Interrupts.md / halt.md）：硬件在每条指令的最后一个 M-cycle 取下一条操作码，
//! 取指之后才检查 `IME && pending()`。本模型里取指是 `step` 的第 1 个 M-cycle，所以检查放在取指之后：
//! 外设在取指那个 M-cycle 里提出的中断（D002：先推进外设、再访问）也会在这条指令执行之前被响应。
//! 要分派时丢弃取到的操作码、PC 回到它的地址，这次取指算作分派的第 1 个 M-cycle（Pan Docs 所说的 2 个等待周期之一），
//! 随后 1 个 idle、写 PCH、写 PCL、1 个 idle，PC=向量，共 5 M-cycle。
//! 向量在写完 PCH 之后才决定：若压栈写到了 IE（SP 指向 0xFFFF）并使中断不再挂起，则 PC=0x0000、IF 不清除。
//! `EI` 延迟一条指令生效（紧跟 `DI` 则不生效），`DI` 立即生效，`RETI` 立即置 IME。
//!
//! HALT：停机期间每个 M-cycle 仍在 PC 处取指（PC 不前进），这个周期结束时若有挂起中断即唤醒——
//! IME=1 时这次取指就是分派的第 1 个 M-cycle，IME=0 时取到的就是 HALT 之后的指令并立即执行；
//! 因此 HALT 等待与用一串 NOP 等待中断的时序完全相同（Pan Docs halt.md）。

use crate::alu::{self, Flags};
use crate::decode::{self, AluOp, CbInstr, Instr, RotOp, Rp, Rp2, RpMem, R8};
use crate::state::{Snapshot, StateError, StateReader, StateWriter};

/// CPU 看到的总线。每次调用恰好消耗 1 个 M-cycle（`pending`/`ack` 除外，它们不耗时）。
pub trait CpuBus {
    /// 读一个字节：1 M-cycle。
    fn read(&mut self, addr: u16) -> u8;
    /// 写一个字节：1 M-cycle。
    fn write(&mut self, addr: u16, v: u8);
    /// 内部周期：1 M-cycle，无总线访问。
    fn idle(&mut self);
    /// `IE & IF & 0x1F`，不耗时。
    fn pending(&self) -> u8;
    /// 清除 IF 的第 `bit` 位（`bit` 为序号 0..=4，0 = VBlank），不耗时。
    fn ack(&mut self, bit: u8);
    /// 执行 STOP 时调用（不耗时）：Bus 借此清零 DIV。测试用的假总线可以不实现。
    fn stop(&mut self) {}
}

/// DI 的操作码：紧跟在 EI 之后时取消 EI 的延迟生效。
const OP_DI: u8 = 0xF3;

/// 寄存器与 CPU 状态。`Cpu::new` 给出 DMG 开机（boot ROM 结束、PC=0x0100）时的值；
/// F 的 H/C 取决于头部校验和，由 `Machine::new` 修正。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    /// 低 4 位恒为 0。
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
    pub halted: bool,
    /// 执行 0x40（LD B,B）时置位；`Machine::ld_b_b_hit` 读取后清零。
    pub ld_b_b: bool,
    /// 刚执行过 EI：IME 将在下一条指令执行完之后置 1（若那条指令是 DI 则取消）。
    pub ei_pending: bool,
    /// HALT bug 已触发：下一次取操作码时 PC 不前进（该字节被读两次）。
    pub halt_bug: bool,
    /// 执行了非法操作码，CPU 锁死：此后每次 `step` 只做 1 次 `idle`。
    pub locked: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    /// DMG 开机状态（Pan Docs Power_Up_Sequence，DMG 列）：A=01 F=B0 B=00 C=13 D=00 E=D8 H=01 L=4D SP=FFFE PC=0100。
    pub fn new() -> Self {
        Self {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            halted: false,
            ld_b_b: false,
            ei_pending: false,
            halt_bug: false,
            locked: false,
        }
    }

    /// 执行 1 条指令，或 1 次中断分派，或 HALT/锁死状态下的 1 个 M-cycle。
    /// 每次调用至少消耗 1 个 M-cycle（至少调用一次 read/write/idle），绝不为 0。
    pub fn step<B: CpuBus>(&mut self, bus: &mut B) {
        if self.locked {
            bus.idle();
            return;
        }
        if self.halted {
            // 停机期间仍在 PC 处取指（PC 不前进）；外设由 bus 在这个 M-cycle 里照常推进。
            let op = bus.read(self.pc);
            if bus.pending() == 0 {
                return;
            }
            // 有挂起中断即唤醒，不额外耗时。
            self.halted = false;
            if self.ime {
                // 这次取指是分派的第 1 个 M-cycle；返回地址是 HALT 之后的指令。
                self.dispatch(bus);
            } else {
                // IME=0：这次取到的就是 HALT 之后的指令，照常执行。
                self.pc = self.pc.wrapping_add(1);
                self.execute(bus, op);
            }
            return;
        }
        let enable_ime = self.ei_pending;
        self.ei_pending = false;
        let op = bus.read(self.pc);
        // HALT bug：这次取指不递增 PC。
        let next = if self.halt_bug { self.pc } else { self.pc.wrapping_add(1) };
        self.halt_bug = false;
        if self.ime && bus.pending() != 0 {
            // 取指之后检查中断：丢弃取到的操作码，PC 回退 1。HALT bug 之后（`EI; HALT` 且已有挂起中断）
            // 回退的结果是 HALT 自身的地址，返回后 HALT 再执行一次（Pan Docs halt.md）。
            self.pc = next.wrapping_sub(1);
            self.dispatch(bus);
            return;
        }
        self.pc = next;
        self.execute(bus, op);
        if enable_ime && op != OP_DI {
            self.ime = true;
        }
    }

    // ---- 16 位寄存器对 ----

    #[inline(always)]
    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }
    #[inline(always)]
    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    #[inline(always)]
    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    #[inline(always)]
    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    /// 写 AF；F 的低 4 位被清零。
    #[inline(always)]
    pub fn set_af(&mut self, v: u16) {
        let [a, f] = v.to_be_bytes();
        self.a = a;
        self.f = f & 0xF0;
    }
    #[inline(always)]
    pub fn set_bc(&mut self, v: u16) {
        [self.b, self.c] = v.to_be_bytes();
    }
    #[inline(always)]
    pub fn set_de(&mut self, v: u16) {
        [self.d, self.e] = v.to_be_bytes();
    }
    #[inline(always)]
    pub fn set_hl(&mut self, v: u16) {
        [self.h, self.l] = v.to_be_bytes();
    }

    #[inline(always)]
    fn rp(&self, rp: Rp) -> u16 {
        match rp {
            Rp::BC => self.bc(),
            Rp::DE => self.de(),
            Rp::HL => self.hl(),
            Rp::SP => self.sp,
        }
    }
    #[inline(always)]
    fn set_rp(&mut self, rp: Rp, v: u16) {
        match rp {
            Rp::BC => self.set_bc(v),
            Rp::DE => self.set_de(v),
            Rp::HL => self.set_hl(v),
            Rp::SP => self.sp = v,
        }
    }
    #[inline(always)]
    fn rp2(&self, rp: Rp2) -> u16 {
        match rp {
            Rp2::BC => self.bc(),
            Rp2::DE => self.de(),
            Rp2::HL => self.hl(),
            Rp2::AF => self.af(),
        }
    }
    #[inline(always)]
    fn set_rp2(&mut self, rp: Rp2, v: u16) {
        match rp {
            Rp2::BC => self.set_bc(v),
            Rp2::DE => self.set_de(v),
            Rp2::HL => self.set_hl(v),
            Rp2::AF => self.set_af(v),
        }
    }

    #[inline(always)]
    fn flags(&self) -> Flags {
        Flags::from_u8(self.f)
    }
    #[inline(always)]
    fn set_flags(&mut self, f: Flags) {
        self.f = f.to_u8();
    }

    // ---- 总线辅助：每次 read/write/idle = 1 M-cycle ----

    #[inline(always)]
    fn imm8<B: CpuBus>(&mut self, bus: &mut B) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }
    #[inline(always)]
    fn imm16<B: CpuBus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.imm8(bus);
        let hi = self.imm8(bus);
        u16::from_le_bytes([lo, hi])
    }
    /// 压栈：先写高字节到 SP-1，再写低字节到 SP-2（2 M-cycle）。
    #[inline(always)]
    fn push16<B: CpuBus>(&mut self, bus: &mut B, v: u16) {
        let [hi, lo] = v.to_be_bytes();
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, hi);
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, lo);
    }
    /// 弹栈：先读低字节，再读高字节（2 M-cycle）。
    #[inline(always)]
    fn pop16<B: CpuBus>(&mut self, bus: &mut B) -> u16 {
        let lo = bus.read(self.sp);
        self.sp = self.sp.wrapping_add(1);
        let hi = bus.read(self.sp);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }
    /// 读 r 表操作数；`(HL)` 消耗 1 M-cycle。
    #[inline(always)]
    fn read_r8<B: CpuBus>(&mut self, bus: &mut B, r: R8) -> u8 {
        match r {
            R8::B => self.b,
            R8::C => self.c,
            R8::D => self.d,
            R8::E => self.e,
            R8::H => self.h,
            R8::L => self.l,
            R8::HlInd => bus.read(self.hl()),
            R8::A => self.a,
        }
    }
    /// 写 r 表操作数；`(HL)` 消耗 1 M-cycle。
    #[inline(always)]
    fn write_r8<B: CpuBus>(&mut self, bus: &mut B, r: R8, v: u8) {
        match r {
            R8::B => self.b = v,
            R8::C => self.c = v,
            R8::D => self.d = v,
            R8::E => self.e = v,
            R8::H => self.h = v,
            R8::L => self.l = v,
            R8::HlInd => bus.write(self.hl(), v),
            R8::A => self.a = v,
        }
    }
    /// `(BC) (DE) (HL+) (HL-)` 的地址；HL+/HL- 在此处完成增减。
    #[inline(always)]
    fn mem_addr(&mut self, m: RpMem) -> u16 {
        match m {
            RpMem::BC => self.bc(),
            RpMem::DE => self.de(),
            RpMem::HLI => {
                let a = self.hl();
                self.set_hl(a.wrapping_add(1));
                a
            }
            RpMem::HLD => {
                let a = self.hl();
                self.set_hl(a.wrapping_sub(1));
                a
            }
        }
    }

    #[inline(always)]
    fn alu_a(&mut self, op: AluOp, v: u8) {
        let f = self.flags();
        let a = self.a;
        let (r, nf) = match op {
            AluOp::Add => alu::add8(a, v, f, false),
            AluOp::Adc => alu::add8(a, v, f, true),
            AluOp::Sub => alu::sub8(a, v, f, false),
            AluOp::Sbc => alu::sub8(a, v, f, true),
            AluOp::And => alu::and8(a, v, f),
            AluOp::Xor => alu::xor8(a, v, f),
            AluOp::Or => alu::or8(a, v, f),
            AluOp::Cp => (a, alu::cp8(a, v, f)),
        };
        self.a = r;
        self.set_flags(nf);
    }

    /// 中断分派的后 4 个 M-cycle（第 1 个是调用方已经完成的、被丢弃的取指）：
    /// 1 个 idle、写 PCH、写 PCL、1 个 idle。调用前 PC 已是要压栈的返回地址。
    fn dispatch<B: CpuBus>(&mut self, bus: &mut B) {
        self.ime = false;
        // 分派取代了"EI 之后的那条指令"，EI 的延迟生效随之作废，避免处理程序第一条指令后 IME 被重新打开。
        self.ei_pending = false;
        bus.idle();
        let [hi, lo] = self.pc.to_be_bytes();
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, hi);
        // 写完 PCH 之后才选择向量：这次写若改了 IE，可能取消分派（PC=0x0000）或换成别的中断。
        let pending = bus.pending();
        let vector = if pending == 0 {
            0x0000
        } else {
            let bit = pending.trailing_zeros() as u8;
            bus.ack(bit);
            0x0040 + 8 * bit as u16
        };
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, lo);
        bus.idle();
        self.pc = vector;
    }

    /// 执行已取出的无前缀操作码（取指的 1 个 M-cycle 已经发生）。
    #[inline(always)]
    fn execute<B: CpuBus>(&mut self, bus: &mut B, op: u8) {
        match decode::decode(op) {
            Instr::Nop => {}
            Instr::LdImmSp => {
                let a = self.imm16(bus);
                let [hi, lo] = self.sp.to_be_bytes();
                bus.write(a, lo);
                bus.write(a.wrapping_add(1), hi);
            }
            Instr::Stop => {
                // 2 字节指令：第二个字节被跳过（不读取），共 1 M-cycle（Opcodes.json：bytes 2、cycles 4）。
                self.pc = self.pc.wrapping_add(1);
                bus.stop();
            }
            Instr::Jr => {
                let e = self.imm8(bus);
                bus.idle();
                self.pc = self.pc.wrapping_add(e as i8 as u16);
            }
            Instr::JrCc(cc) => {
                let e = self.imm8(bus);
                if cc.eval(self.f) {
                    bus.idle();
                    self.pc = self.pc.wrapping_add(e as i8 as u16);
                }
            }
            Instr::LdRpImm(rp) => {
                let v = self.imm16(bus);
                self.set_rp(rp, v);
            }
            Instr::AddHlRp(rp) => {
                let (r, f) = alu::add16_hl(self.hl(), self.rp(rp), self.flags());
                bus.idle();
                self.set_hl(r);
                self.set_flags(f);
            }
            Instr::LdMemA(m) => {
                let a = self.mem_addr(m);
                bus.write(a, self.a);
            }
            Instr::LdAMem(m) => {
                let a = self.mem_addr(m);
                self.a = bus.read(a);
            }
            Instr::IncRp(rp) => {
                bus.idle();
                self.set_rp(rp, self.rp(rp).wrapping_add(1));
            }
            Instr::DecRp(rp) => {
                bus.idle();
                self.set_rp(rp, self.rp(rp).wrapping_sub(1));
            }
            Instr::Inc8(r) => {
                let v = self.read_r8(bus, r);
                let (res, f) = alu::inc8(v, self.flags());
                self.set_flags(f);
                self.write_r8(bus, r, res);
            }
            Instr::Dec8(r) => {
                let v = self.read_r8(bus, r);
                let (res, f) = alu::dec8(v, self.flags());
                self.set_flags(f);
                self.write_r8(bus, r, res);
            }
            Instr::LdR8Imm(r) => {
                let v = self.imm8(bus);
                self.write_r8(bus, r, v);
            }
            Instr::Rlca => {
                let (r, f) = alu::rlca(self.a, self.flags());
                self.a = r;
                self.set_flags(f);
            }
            Instr::Rrca => {
                let (r, f) = alu::rrca(self.a, self.flags());
                self.a = r;
                self.set_flags(f);
            }
            Instr::Rla => {
                let (r, f) = alu::rla(self.a, self.flags());
                self.a = r;
                self.set_flags(f);
            }
            Instr::Rra => {
                let (r, f) = alu::rra(self.a, self.flags());
                self.a = r;
                self.set_flags(f);
            }
            Instr::Daa => {
                let (r, f) = alu::daa(self.a, self.flags());
                self.a = r;
                self.set_flags(f);
            }
            Instr::Cpl => {
                let (r, f) = alu::cpl(self.a, self.flags());
                self.a = r;
                self.set_flags(f);
            }
            Instr::Scf => self.set_flags(alu::scf(self.flags())),
            Instr::Ccf => self.set_flags(alu::ccf(self.flags())),
            Instr::Halt => {
                // 此处的 self.ime 是 HALT 执行时的值（`EI; HALT` 中 EI 尚未生效）。
                if !self.ime && bus.pending() != 0 {
                    // HALT bug：立即结束，下一次取指不递增 PC（Pan Docs halt.md）。
                    self.halt_bug = true;
                } else {
                    self.halted = true;
                }
            }
            Instr::Ld8(dst, src) => {
                if dst == R8::B && src == R8::B {
                    self.ld_b_b = true;
                }
                let v = self.read_r8(bus, src);
                self.write_r8(bus, dst, v);
            }
            Instr::Alu(op, r) => {
                let v = self.read_r8(bus, r);
                self.alu_a(op, v);
            }
            Instr::AluImm(op) => {
                let v = self.imm8(bus);
                self.alu_a(op, v);
            }
            Instr::RetCc(cc) => {
                bus.idle();
                if cc.eval(self.f) {
                    self.pc = self.pop16(bus);
                    bus.idle();
                }
            }
            Instr::LdhImmA => {
                let n = self.imm8(bus);
                bus.write(0xFF00 | n as u16, self.a);
            }
            Instr::AddSpE => {
                let e = self.imm8(bus);
                let (r, f) = alu::add_sp_e(self.sp, e, self.flags());
                bus.idle();
                bus.idle();
                self.sp = r;
                self.set_flags(f);
            }
            Instr::LdhAImm => {
                let n = self.imm8(bus);
                self.a = bus.read(0xFF00 | n as u16);
            }
            Instr::LdHlSpE => {
                let e = self.imm8(bus);
                let (r, f) = alu::add_sp_e(self.sp, e, self.flags());
                bus.idle();
                self.set_hl(r);
                self.set_flags(f);
            }
            Instr::Pop(rp) => {
                let v = self.pop16(bus);
                self.set_rp2(rp, v);
            }
            Instr::Ret => {
                self.pc = self.pop16(bus);
                bus.idle();
            }
            Instr::Reti => {
                self.pc = self.pop16(bus);
                bus.idle();
                self.ime = true;
            }
            Instr::JpHl => self.pc = self.hl(),
            Instr::LdSpHl => {
                bus.idle();
                self.sp = self.hl();
            }
            Instr::JpCc(cc) => {
                let a = self.imm16(bus);
                if cc.eval(self.f) {
                    bus.idle();
                    self.pc = a;
                }
            }
            Instr::LdhCA => bus.write(0xFF00 | self.c as u16, self.a),
            Instr::LdImmA => {
                let a = self.imm16(bus);
                bus.write(a, self.a);
            }
            Instr::LdhAC => self.a = bus.read(0xFF00 | self.c as u16),
            Instr::LdAImm => {
                let a = self.imm16(bus);
                self.a = bus.read(a);
            }
            Instr::Jp => {
                let a = self.imm16(bus);
                bus.idle();
                self.pc = a;
            }
            Instr::Prefix => {
                let op = self.imm8(bus);
                self.execute_cb(bus, op);
            }
            Instr::Di => self.ime = false,
            Instr::Ei => self.ei_pending = true,
            Instr::CallCc(cc) => {
                let a = self.imm16(bus);
                if cc.eval(self.f) {
                    bus.idle();
                    let pc = self.pc;
                    self.push16(bus, pc);
                    self.pc = a;
                }
            }
            Instr::Push(rp) => {
                bus.idle();
                let v = self.rp2(rp);
                self.push16(bus, v);
            }
            Instr::Call => {
                let a = self.imm16(bus);
                bus.idle();
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = a;
            }
            Instr::Rst(vec) => {
                bus.idle();
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = vec as u16;
            }
            Instr::Illegal => self.locked = true,
        }
    }

    /// 执行 CB 前缀指令（前缀与第二个字节的 2 个 M-cycle 已经发生）。
    #[inline(always)]
    fn execute_cb<B: CpuBus>(&mut self, bus: &mut B, op: u8) {
        match decode::decode_cb(op) {
            CbInstr::Rot(rot, r) => {
                let v = self.read_r8(bus, r);
                let f = self.flags();
                let (res, nf) = match rot {
                    RotOp::Rlc => alu::rlc(v, f),
                    RotOp::Rrc => alu::rrc(v, f),
                    RotOp::Rl => alu::rl(v, f),
                    RotOp::Rr => alu::rr(v, f),
                    RotOp::Sla => alu::sla(v, f),
                    RotOp::Sra => alu::sra(v, f),
                    RotOp::Swap => alu::swap(v, f),
                    RotOp::Srl => alu::srl(v, f),
                };
                self.set_flags(nf);
                self.write_r8(bus, r, res);
            }
            CbInstr::Bit(b, r) => {
                let v = self.read_r8(bus, r);
                let f = alu::bit(b, v, self.flags());
                self.set_flags(f);
            }
            CbInstr::Res(b, r) => {
                let v = self.read_r8(bus, r);
                self.write_r8(bus, r, alu::res(b, v));
            }
            CbInstr::Set(b, r) => {
                let v = self.read_r8(bus, r);
                self.write_r8(bus, r, alu::set(b, v));
            }
        }
    }
}

impl Snapshot for Cpu {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CPU ");
        for v in [self.a, self.f, self.b, self.c, self.d, self.e, self.h, self.l] {
            w.u8(v);
        }
        w.u16(self.sp);
        w.u16(self.pc);
        w.bool(self.ime);
        w.bool(self.halted);
        w.bool(self.ld_b_b);
        w.bool(self.ei_pending);
        w.bool(self.halt_bug);
        w.bool(self.locked);
    }

    /// 全部读完并校验后才写回；失败时 CPU 状态不变。
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CPU ")?;
        let mut regs = [0u8; 8];
        r.bytes_into(&mut regs)?;
        let [a, f, b, c, d, e, h, l] = regs;
        if f & 0x0F != 0 {
            return Err(StateError::Corrupt("CPU F low nibble"));
        }
        let sp = r.u16()?;
        let pc = r.u16()?;
        let ime = r.bool()?;
        let halted = r.bool()?;
        let ld_b_b = r.bool()?;
        let ei_pending = r.bool()?;
        let halt_bug = r.bool()?;
        let locked = r.bool()?;
        *self = Cpu { a, f, b, c, d, e, h, l, sp, pc, ime, halted, ld_b_b, ei_pending, halt_bug, locked };
        Ok(())
    }
}
