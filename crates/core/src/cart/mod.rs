//! 卡带（platform 通道，T50/T50b；PLAN §3.6）。
//!
//! 冻结部分（D001）：`Cart` trait、`from_rom`、`CartError`（可新增变体）。
//! `from_rom` 按头部 0x147 选择类型、按 0x149 分配 RAM；bank 号按 ROM 实际 bank 数取模。
//! 已实现：ROM only（0x00，以及 0x08/0x09 ROM+RAM）、MBC1（0x01–0x03）、MBC3（0x0F–0x13）。
//! MBC2（0x05–0x06）、MBC5（0x19–0x1E）在 T50b 补齐，之前返回 `CartError::UnsupportedType`。

mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

use std::fmt;

use crate::state::{Snapshot, StateError, StateReader, StateWriter};

pub use mbc1::Mbc1;
pub use mbc3::Mbc3;

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
    /// ROM 小于 0x150 字节（放不下头部）。不足 32 KiB 或不是 16 KiB 整数倍的 ROM 用 0xFF 补齐，不报错。
    BadSize(usize),
    /// 不支持的卡带类型（头部 0x147）。
    UnsupportedType(u8),
    /// 不认识的 RAM 大小代码（头部 0x149）。
    UnsupportedRamSize(u8),
}

impl fmt::Display for CartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CartError::BadSize(n) => write!(f, "bad ROM size: {n} bytes"),
            CartError::UnsupportedType(t) => write!(f, "unsupported cartridge type 0x{t:02X}"),
            CartError::UnsupportedRamSize(c) => write!(f, "unsupported RAM size code 0x{c:02X}"),
        }
    }
}

impl std::error::Error for CartError {}

/// 一个 ROM bank 的大小。
pub(crate) const ROM_BANK: usize = 0x4000;
/// 一个 RAM bank 的大小。
pub(crate) const RAM_BANK: usize = 0x2000;

/// 头部 0x149 的 RAM 字节数（Pan Docs The_Cartridge_Header：0/–/8K/32K/128K/64K；
/// 代码 0x01 未被使用，按 2 KiB 处理）。
pub(crate) fn ram_bytes(code: u8) -> Result<usize, CartError> {
    Ok(match code {
        0x00 => 0,
        0x01 => 0x800,
        0x02 => 0x2000,
        0x03 => 0x8000,
        0x04 => 0x20000,
        0x05 => 0x10000,
        _ => return Err(CartError::UnsupportedRamSize(code)),
    })
}

/// 把 ROM 补齐到 16 KiB 的整数倍、至少 2 个 bank（补 0xFF），返回 bank 数。
pub(crate) fn pad_rom(rom: &mut Vec<u8>) -> usize {
    let banks = rom.len().div_ceil(ROM_BANK).max(2);
    rom.resize(banks * ROM_BANK, 0xFF);
    banks
}

/// 解析头部并选择 MBC。未知类型返回 `CartError`，不 panic。
pub fn from_rom(mut rom: Vec<u8>) -> Result<Box<dyn Cart>, CartError> {
    if rom.len() < 0x150 {
        return Err(CartError::BadSize(rom.len()));
    }
    let kind = rom[0x147];
    let ram_code = rom[0x149];
    // 类型名中带 RAM 的才有外部 RAM；大小以 0x149 为准。
    let (has_ram, ram_cap) = match kind {
        0x00 => (false, 0),
        0x08 | 0x09 => (true, RAM_BANK),
        0x01 => (false, 0),
        0x02 | 0x03 => (true, 4 * RAM_BANK),
        0x0F | 0x11 => (false, 0),
        0x10 | 0x12 | 0x13 => (true, 4 * RAM_BANK),
        _ => return Err(CartError::UnsupportedType(kind)),
    };
    let ram_len = if has_ram { ram_bytes(ram_code)?.min(ram_cap) } else { 0 };
    let banks = pad_rom(&mut rom);
    Ok(match kind {
        0x00 | 0x08 | 0x09 => Box::new(RomOnly::new(rom, ram_len)),
        0x01..=0x03 => Box::new(Mbc1::new(rom, banks, ram_len)),
        _ => Box::new(Mbc3::new(rom, banks, ram_len, matches!(kind, 0x0F | 0x10))),
    })
}

/// 读回 RAM 内容：长度必须与当前卡带一致。
pub(crate) fn load_ram(r: &mut StateReader, expect: usize) -> Result<Vec<u8>, StateError> {
    let ram = r.vec()?;
    if ram.len() != expect {
        return Err(StateError::Corrupt("cart RAM size"));
    }
    Ok(ram)
}

/// ROM only（类型 0x00；0x08/0x09 带最多 8 KiB RAM，无使能寄存器）：前 32 KiB 直接映射。
pub struct RomOnly {
    rom: Vec<u8>,
    ram: Vec<u8>,
}

/// 存档里的卡带种类编号。
const KIND_ROM_ONLY: u8 = 0;

impl RomOnly {
    /// `rom` 不足 32 KiB 时补 0xFF。
    pub fn new(mut rom: Vec<u8>, ram_len: usize) -> Self {
        if rom.len() < 2 * ROM_BANK {
            rom.resize(2 * ROM_BANK, 0xFF);
        }
        Self { rom, ram: vec![0; ram_len] }
    }
}

impl Cart for RomOnly {
    fn read_rom(&self, addr: u16) -> u8 {
        self.rom[(addr & 0x7FFF) as usize]
    }
    fn write_rom(&mut self, _addr: u16, _v: u8) {}
    fn read_ram(&self, addr: u16) -> u8 {
        if self.ram.is_empty() {
            return 0xFF;
        }
        self.ram[(addr as usize & 0x1FFF) % self.ram.len()]
    }
    fn write_ram(&mut self, addr: u16, v: u8) {
        if self.ram.is_empty() {
            return;
        }
        let i = (addr as usize & 0x1FFF) % self.ram.len();
        self.ram[i] = v;
    }
}

impl Snapshot for RomOnly {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CART");
        w.u8(KIND_ROM_ONLY);
        w.vec(&self.ram);
    }
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CART")?;
        if r.u8()? != KIND_ROM_ONLY {
            return Err(StateError::Corrupt("cart kind"));
        }
        self.ram = load_ram(r, self.ram.len())?;
        Ok(())
    }
}
