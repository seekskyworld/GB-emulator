//! 总线与内存映射（sys 通道，T13；PLAN §3.2、§3.3、T13）。
//!
//! 归属：WRAM、HRAM、APU 寄存器存储由本模块持有；VRAM/OAM 转发给 `Ppu`；卡带 ROM/RAM 转发给 `Cart`；
//! IF/IE 在 `interrupts.rs`。每个 M-cycle 先推进外设 4 T（timer → serial → joypad → ppu → dma），
//! 再执行访问（D002）。
//!
//! 本文件除 `Machine` 用到的方法外都由 sys 通道自定。

use crate::cart::Cart;
use crate::cpu::CpuBus;
use crate::dma::Dma;
use crate::interrupts::Interrupts;
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::serial::Serial;
use crate::timer::Timer;
use crate::Frame;

pub struct Bus {
    pub(crate) cart: Box<dyn Cart>,
    pub(crate) wram: Box<[u8; 0x2000]>,
    pub(crate) hram: [u8; 0x7F],
    pub(crate) ints: Interrupts,
    pub(crate) timer: Timer,
    pub(crate) serial: Serial,
    pub(crate) joypad: Joypad,
    pub(crate) ppu: Ppu,
    pub(crate) dma: Dma,
    /// 开机以来的 M-cycle 数。
    pub(crate) mcycles: u64,
}

impl Bus {
    pub fn new(cart: Box<dyn Cart>) -> Self {
        Self {
            cart,
            wram: Box::new([0; 0x2000]),
            hram: [0; 0x7F],
            ints: Interrupts::new(),
            timer: Timer::new(),
            serial: Serial::new(),
            joypad: Joypad::new(),
            ppu: Ppu::new(),
            dma: Dma::new(),
            mcycles: 0,
        }
    }

    pub fn mcycles(&self) -> u64 {
        self.mcycles
    }

    pub fn frame(&self) -> &Frame {
        self.ppu.frame()
    }

    pub fn set_buttons(&mut self, _mask: u8) {
        todo!("sys: Bus::set_buttons (T51)")
    }

    pub fn take_serial(&mut self) -> Vec<u8> {
        todo!("sys: Bus::take_serial (T16)")
    }

    /// 无副作用读，与 `read` 共用映射但不推进时钟。
    pub fn peek(&self, _addr: u16) -> u8 {
        todo!("sys: Bus::peek (T13)")
    }
}

impl CpuBus for Bus {
    fn read(&mut self, _addr: u16) -> u8 {
        todo!("sys: Bus::read (T13)")
    }
    fn write(&mut self, _addr: u16, _v: u8) {
        todo!("sys: Bus::write (T13)")
    }
    fn idle(&mut self) {
        todo!("sys: Bus::idle (T13)")
    }
    fn pending(&self) -> u8 {
        todo!("sys: Bus::pending (T14)")
    }
    fn ack(&mut self, _bit: u8) {
        todo!("sys: Bus::ack (T14)")
    }
}
