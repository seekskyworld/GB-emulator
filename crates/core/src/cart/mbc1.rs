//! MBC1（类型 0x01–0x03；platform 通道，PLAN §3.6，Pan Docs MBC1.md）。
//!
//! - 0000–1FFF：RAM 使能，写入值低 4 位为 0xA 时使能，其余值禁用。
//! - 2000–3FFF：5 位 ROM bank 寄存器；低 5 位为 0 时当作 1（只看这 5 位，
//!   因此 0x20/0x40/0x60 实际映射为 0x21/0x41/0x61）。
//! - 4000–5FFF：2 位高位寄存器，作为 ROM bank 的 bit5–6；mode 1 时还作用于 0000 区与 RAM bank。
//! - 6000–7FFF：banking mode（bit0）。
//!
//! bank 号按 ROM 实际 bank 数取模；RAM 最多 4×8 KiB，按实际大小取模。
//!
//! 多卡带（MBC1M，Pan Docs MBC1 一节末尾）：1 MiB 的卡带，bank 0x10 处另有一份带 Nintendo 标志的头部。
//! 判定：ROM 恰为 64 个 bank，且 bank 0x10 的标志区（0x0104–0x0133）与 bank 0 的完全相同、并且不是同一字节
//! 的重复。MBC1M 的高位寄存器作为 bank 号的 bit4–5，5 位寄存器的 bit4 不参与选 bank
//! （但 0→1 转换仍看完整的 5 位）；mode 1 时 0000 区为 bank `高位 << 4`。

use super::{load_ram, Cart, RAM_BANK, ROM_BANK};
use crate::state::{Snapshot, StateError, StateReader, StateWriter};

const KIND_MBC1: u8 = 1;

pub struct Mbc1 {
    rom: Vec<u8>,
    rom_banks: usize,
    ram: Vec<u8>,
    ram_enabled: bool,
    /// 5 位 ROM bank 寄存器（原值，未做 0→1 转换）。
    bank1: u8,
    /// 2 位高位寄存器。
    bank2: u8,
    /// banking mode（0 或 1）。
    mode: u8,
    /// MBC1M 接线（由 ROM 内容判定，不进存档）。
    multicart: bool,
    // 由寄存器推出的偏移，寄存器变化时重算。
    rom0_off: usize,
    romx_off: usize,
    ram_off: usize,
}

impl Mbc1 {
    /// `rom` 的长度必须是 `rom_banks` 个 16 KiB。
    pub fn new(rom: Vec<u8>, rom_banks: usize, ram_len: usize) -> Self {
        debug_assert_eq!(rom.len(), rom_banks * ROM_BANK);
        let mut m = Self {
            rom,
            rom_banks,
            ram: vec![0; ram_len],
            ram_enabled: false,
            bank1: 0,
            bank2: 0,
            mode: 0,
            multicart: false,
            rom0_off: 0,
            romx_off: 0,
            ram_off: 0,
        };
        m.multicart = is_multicart(&m.rom, rom_banks);
        m.remap();
        m
    }

    /// 是否按 MBC1M（多卡带）接线。
    pub fn is_multicart(&self) -> bool {
        self.multicart
    }

    fn remap(&mut self) {
        let low = if self.bank1 == 0 { 1 } else { self.bank1 as usize };
        let (low, high) = if self.multicart {
            (low & 0x0F, (self.bank2 as usize) << 4)
        } else {
            (low, (self.bank2 as usize) << 5)
        };
        let zero = if self.mode == 1 { high } else { 0 };
        self.rom0_off = (zero % self.rom_banks) * ROM_BANK;
        self.romx_off = ((high | low) % self.rom_banks) * ROM_BANK;
        let ram_bank = if self.mode == 1 { self.bank2 as usize } else { 0 };
        self.ram_off = ram_bank * RAM_BANK;
    }

    fn ram_index(&self, addr: u16) -> Option<usize> {
        if !self.ram_enabled || self.ram.is_empty() {
            return None;
        }
        Some((self.ram_off + (addr as usize & 0x1FFF)) % self.ram.len())
    }
}

/// 头部 Nintendo 标志所在区间。
const LOGO: std::ops::Range<usize> = 0x0104..0x0134;

/// MBC1M 判定：64 个 bank，bank 0x10 的标志区与 bank 0 相同，且标志区不是单一字节的重复。
fn is_multicart(rom: &[u8], rom_banks: usize) -> bool {
    if rom_banks != 64 {
        return false;
    }
    let logo0 = &rom[LOGO];
    let base = 0x10 * ROM_BANK;
    let logo16 = &rom[base + LOGO.start..base + LOGO.end];
    let uniform = logo0.iter().all(|&b| b == logo0[0]);
    !uniform && logo0 == logo16
}

impl Cart for Mbc1 {
    fn read_rom(&self, addr: u16) -> u8 {
        let a = addr as usize & 0x3FFF;
        if addr < 0x4000 {
            self.rom[self.rom0_off + a]
        } else {
            self.rom[self.romx_off + a]
        }
    }

    fn write_rom(&mut self, addr: u16, v: u8) {
        match addr & 0x7FFF {
            0x0000..=0x1FFF => self.ram_enabled = v & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.bank1 = v & 0x1F,
            0x4000..=0x5FFF => self.bank2 = v & 0x03,
            _ => self.mode = v & 0x01,
        }
        self.remap();
    }

    fn read_ram(&self, addr: u16) -> u8 {
        match self.ram_index(addr) {
            Some(i) => self.ram[i],
            None => 0xFF,
        }
    }

    fn write_ram(&mut self, addr: u16, v: u8) {
        if let Some(i) = self.ram_index(addr) {
            self.ram[i] = v;
        }
    }
}

impl Snapshot for Mbc1 {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CART");
        w.u8(KIND_MBC1);
        w.bool(self.ram_enabled);
        w.u8(self.bank1);
        w.u8(self.bank2);
        w.u8(self.mode);
        w.vec(&self.ram);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CART")?;
        if r.u8()? != KIND_MBC1 {
            return Err(StateError::Corrupt("cart kind"));
        }
        let ram_enabled = r.bool()?;
        let bank1 = r.u8()?;
        let bank2 = r.u8()?;
        let mode = r.u8()?;
        if bank1 > 0x1F || bank2 > 0x03 || mode > 1 {
            return Err(StateError::Corrupt("MBC1 registers"));
        }
        let ram = load_ram(r, self.ram.len())?;
        self.ram_enabled = ram_enabled;
        self.bank1 = bank1;
        self.bank2 = bank2;
        self.mode = mode;
        self.ram = ram;
        self.remap();
        Ok(())
    }
}
