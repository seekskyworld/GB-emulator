//! SM83 CPU（core 通道，T10/T11/T14）。
//!
//! 冻结部分（D001）：`CpuBus` trait、`Cpu` 的公开字段、`Cpu::new`、`Cpu::step`、`Snapshot for Cpu`。
//! 内部状态（EI 延迟、HALT bug、锁死等）由 core 自行增加私有或公开字段；增加字段不算改签名。

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

/// 寄存器与 CPU 状态。`Cpu::new` 给出 DMG 开机（boot ROM 结束、PC=0x0100）时的值；
/// F 的 H/C 取决于头部校验和，由 `Machine::new` 修正。
#[derive(Debug, Clone)]
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
        }
    }

    /// 执行 1 条指令，或 1 次中断分派，或 HALT/STOP/锁死状态下的 1 个 M-cycle。
    /// 每次调用至少消耗 1 个 M-cycle（至少调用一次 read/write/idle），绝不为 0。
    pub fn step<B: CpuBus>(&mut self, bus: &mut B) {
        // T02 空壳：T10/T11 替换为真正的取指、译码、执行。
        bus.idle();
    }
}

impl Snapshot for Cpu {
    fn save(&self, _w: &mut StateWriter) {
        todo!("core: Snapshot for Cpu")
    }
    fn load(&mut self, _r: &mut StateReader) -> Result<(), StateError> {
        todo!("core: Snapshot for Cpu")
    }
}
