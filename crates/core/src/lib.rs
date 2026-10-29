//! gb-core：DMG（初代 Game Boy）模拟器核心，零依赖。
//!
//! 时钟模型（DECISIONS D002）：CPU 驱动时钟，每次总线访问或内部周期 = 1 个 M-cycle；
//! 每个 M-cycle 内 Bus 先把所有外设推进 4 T，再执行访问。
//!
//! 本文件归 integrator 所有（PLAN §6.4）；跨通道接口在 T02 冻结（DECISIONS D001），
//! 改动签名前必须先记 DECISIONS.md。

pub mod alu;
pub mod bus;
pub mod cart;
pub mod cpu;
pub mod decode;
pub mod dma;
pub mod interrupts;
pub mod joypad;
pub mod machine;
pub mod ppu;
pub mod serial;
pub mod state;
pub mod timer;

pub use cart::{Cart, CartError};
pub use cpu::{Cpu, CpuBus};
pub use machine::{Machine, Regs};
pub use state::{Snapshot, StateError, StateReader, StateWriter};

/// 屏幕宽度（像素）。
pub const SCREEN_W: usize = 160;
/// 屏幕高度（像素）。
pub const SCREEN_H: usize = 144;
/// 一帧的像素数；帧缓冲每字节一个像素，取值只有 0xFF/0xAA/0x55/0x00（色号 0 = 0xFF）。
pub const FRAME_PIXELS: usize = SCREEN_W * SCREEN_H;
/// 一帧的 T-cycle 数（154 行 × 456 dot）。`Machine::run_frame` 与 LCD 开关无关地按它推进。
pub const FRAME_T: u32 = 70224;

/// 一帧画面：160×144，行优先。
pub type Frame = [u8; FRAME_PIXELS];

/// 中断位（IF/IE 的 bit 序号与掩码）。`CpuBus::ack` 接收的是 bit 序号 0..=4。
pub mod irq {
    pub const VBLANK: u8 = 1 << 0;
    pub const STAT: u8 = 1 << 1;
    pub const TIMER: u8 = 1 << 2;
    pub const SERIAL: u8 = 1 << 3;
    pub const JOYPAD: u8 = 1 << 4;
}

/// `Machine::set_buttons` 的位定义：1 = 按下。
pub mod buttons {
    pub const A: u8 = 1 << 0;
    pub const B: u8 = 1 << 1;
    pub const SELECT: u8 = 1 << 2;
    pub const START: u8 = 1 << 3;
    pub const RIGHT: u8 = 1 << 4;
    pub const LEFT: u8 = 1 << 5;
    pub const UP: u8 = 1 << 6;
    pub const DOWN: u8 = 1 << 7;
}

/// 外设统一形态（timer/serial/joypad/ppu 都遵守；DMA 另有接口，见 `dma.rs`）。
///
/// Bus 每个 M-cycle 依次调用 timer → serial → joypad → ppu 的 `tick_mcycle`，再推进 DMA，
/// 并把返回的中断位或进 IF。`read_reg` 必须无副作用（`Machine::peek` 也走它）。
pub trait Peripheral: Snapshot {
    /// 推进 1 个 M-cycle（4 T），返回本周期新请求的中断位（IF 位掩码，见 `irq`）。
    fn tick_mcycle(&mut self) -> u8;
    /// 读 I/O 寄存器，`addr` 为绝对地址（如 0xFF04）。
    fn read_reg(&self, addr: u16) -> u8;
    /// 写 I/O 寄存器，`addr` 为绝对地址。
    fn write_reg(&mut self, addr: u16, v: u8);
}
