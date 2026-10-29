//! 整机（T02 由 integrator 写骨架，之后归 sys 通道）。
//!
//! 冻结部分（D001）：`Machine` 的全部公开方法与 `Regs`（PLAN §3.2）。cli、ffi、Python 只经由它们访问核心。

use crate::bus::Bus;
use crate::cart::{self, CartError};
use crate::cpu::Cpu;
use crate::state::StateError;
use crate::{Frame, FRAME_T};

/// 寄存器快照（`Machine::regs`）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Regs {
    pub a: u8,
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
}

pub struct Machine {
    cpu: Cpu,
    bus: Bus,
    /// `run_frame` 的 T-cycle 余量：每帧加 70224，执行到不大于 0；超出部分累计到下一帧。
    frame_budget: i64,
}

impl Machine {
    /// 解析头部、选择 MBC、设置 DMG 开机状态。
    pub fn new(rom: Vec<u8>) -> Result<Self, CartError> {
        if rom.len() < 0x150 {
            return Err(CartError::BadSize(rom.len()));
        }
        let header_checksum = rom[0x14D];
        let cart = cart::from_rom(rom)?;
        let mut cpu = Cpu::new();
        // Power_Up_Sequence：头部校验和为 0 时 H、C 清零，否则置位（Z=1、N=0 不变）。
        cpu.f = if header_checksum == 0 { 0x80 } else { 0xB0 };
        Ok(Self { cpu, bus: Bus::new(cart), frame_budget: 0 })
    }

    /// 执行 1 条指令或 1 次中断分派，返回消耗的 T 数；HALT/STOP/锁死时也至少返回 4，绝不返回 0。
    pub fn step(&mut self) -> u32 {
        let start = self.bus.mcycles();
        self.cpu.step(&mut self.bus);
        ((self.bus.mcycles() - start) * 4) as u32
    }

    /// 精确推进 70224 T（余量累计到下一帧），与 LCD 开关无关。
    pub fn run_frame(&mut self) {
        self.frame_budget += FRAME_T as i64;
        while self.frame_budget > 0 {
            self.frame_budget -= self.step() as i64;
        }
    }

    /// bit0..7 = A B Select Start Right Left Up Down，1 = 按下（见 `crate::buttons`）。
    pub fn set_buttons(&mut self, mask: u8) {
        self.bus.set_buttons(mask);
    }

    /// 最近一个完整帧（前缓冲），值为 0xFF/0xAA/0x55/0x00。
    pub fn frame(&self) -> &Frame {
        self.bus.frame()
    }

    /// 无副作用读（不推进时钟，不触发任何寄存器副作用）。
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.peek(addr)
    }

    /// 取走自上次调用以来串口发出的字节（内部时钟传输开始时追加）。
    pub fn take_serial(&mut self) -> Vec<u8> {
        self.bus.take_serial()
    }

    pub fn regs(&self) -> Regs {
        let c = &self.cpu;
        Regs {
            a: c.a,
            f: c.f,
            b: c.b,
            c: c.c,
            d: c.d,
            e: c.e,
            h: c.h,
            l: c.l,
            sp: c.sp,
            pc: c.pc,
            ime: c.ime,
            halted: c.halted,
        }
    }

    /// 自上次查询以来是否执行过 0x40（LD B,B），读后清零。
    pub fn ld_b_b_hit(&mut self) -> bool {
        std::mem::take(&mut self.cpu.ld_b_b)
    }

    /// 开机以来累计的 T-cycle 数。
    pub fn cycles(&self) -> u64 {
        self.bus.mcycles() * 4
    }

    /// 存档：magic `GBEMUST`、版本 u16、ROM 指纹，然后依次是各模块的 `Snapshot`（PLAN §3.8）。
    pub fn save_state(&self) -> Vec<u8> {
        todo!("sys: Machine::save_state (T13)")
    }

    /// 读档；ROM 指纹不匹配时返回 `StateError::RomMismatch`，失败时机器状态保持不变。
    pub fn load_state(&mut self, _data: &[u8]) -> Result<(), StateError> {
        todo!("sys: Machine::load_state (T13)")
    }
}
