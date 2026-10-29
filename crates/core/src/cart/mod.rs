//! 卡带（platform 通道，T50/T50b；PLAN §3.6）。
//!
//! 冻结部分（D001）：`Cart` trait、`from_rom`、`CartError`。
//! T02 临时实现：头部类型为 ROM only、MBC1、MBC3 且 ROM 不超过 32 KiB 时，一律按 ROM only 处理
//! （忽略 MBC 寄存器写入，无卡带 RAM），足够运行 32 KiB 的 Blargg individual ROM；T50 替换。

mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

use std::fmt;

use crate::state::{Snapshot, StateError, StateReader, StateWriter};

/// 卡带：ROM 区 0000–7FFF 与外部 RAM 区 A000–BFFF。读方法必须无副作用（`peek` 也走它们）。
pub trait Cart: Snapshot + Send {
    /// 读 ROM 区，`addr` 为 0x0000..=0x7FFF。
    fn read_rom(&self, addr: u16) -> u8;
    /// 写 ROM 区（MBC 寄存器），`addr` 为 0x0000..=0x7FFF。
    fn write_rom(&mut self, addr: u16, v: u8);
    /// 读外部 RAM 区，`addr` 为 0xA000..=0xBFFF；RAM 未使能或不存在时读 0xFF。
    fn read_ram(&self, addr: u16) -> u8;
    /// 写外部 RAM 区，`addr` 为 0xA000..=0xBFFF；RAM 未使能或不存在时忽略。
    fn write_ram(&mut self, addr: u16, v: u8);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartError {
    /// ROM 小于 0x150 字节（放不下头部）或不是 16 KiB 的整数倍。
    BadSize(usize),
    /// 不支持的卡带类型（头部 0x147）。
    UnsupportedType(u8),
}

impl fmt::Display for CartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CartError::BadSize(n) => write!(f, "bad ROM size: {n} bytes"),
            CartError::UnsupportedType(t) => write!(f, "unsupported cartridge type 0x{t:02X}"),
        }
    }
}

impl std::error::Error for CartError {}

/// 解析头部并选择 MBC。未知类型返回 `CartError`，不 panic。
pub fn from_rom(rom: Vec<u8>) -> Result<Box<dyn Cart>, CartError> {
    if rom.len() < 0x150 {
        return Err(CartError::BadSize(rom.len()));
    }
    let kind = rom[0x147];
    match kind {
        0x00..=0x03 | 0x0F..=0x13 if rom.len() <= 0x8000 => Ok(Box::new(RomOnly::new(rom))),
        _ => Err(CartError::UnsupportedType(kind)),
    }
}

/// ROM only（类型 0x00）：32 KiB 直接映射，无 RAM。
pub struct RomOnly {
    rom: Vec<u8>,
}

impl RomOnly {
    pub fn new(mut rom: Vec<u8>) -> Self {
        rom.resize(0x8000, 0xFF);
        Self { rom }
    }
}

impl Cart for RomOnly {
    fn read_rom(&self, addr: u16) -> u8 {
        self.rom[(addr & 0x7FFF) as usize]
    }
    fn write_rom(&mut self, _addr: u16, _v: u8) {}
    fn read_ram(&self, _addr: u16) -> u8 {
        0xFF
    }
    fn write_ram(&mut self, _addr: u16, _v: u8) {}
}

impl Snapshot for RomOnly {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CART");
    }
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CART")
    }
}
