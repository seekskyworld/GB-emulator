//! MBC5（类型 0x19–0x1E；platform 通道，PLAN §3.6，Pan Docs MBC5.md）。
//!
//! - 0000–1FFF：RAM 使能，写入值低 4 位为 0xA 时使能，其余值禁用。
//! - 2000–2FFF：ROM bank 低 8 位；3000–3FFF：ROM bank 第 9 位（bit0）。**写 0 就是 bank 0**。
//! - 4000–5FFF：RAM bank（0x00–0x0F）；带震动的类型（0x1C–0x1E）bit3 接震动马达，不参与 RAM 选择。
//! - bank 号按 ROM / RAM 实际 bank 数取模；RAM 最多 16×8 KiB。

use super::{load_ram, Cart, RAM_BANK, ROM_BANK};
use crate::state::{Snapshot, StateError, StateReader, StateWriter};

const KIND_MBC5: u8 = 5;

pub struct Mbc5 {
    rom: Vec<u8>,
    rom_banks: usize,
    ram: Vec<u8>,
    rumble: bool,
    ram_enabled: bool,
    /// 9 位 ROM bank 寄存器。
    rom_bank: u16,
    /// 4000–5FFF 写入值的低 4 位。
    ram_bank: u8,
    romx_off: usize,
}

impl Mbc5 {
    /// `rom` 的长度必须是 `rom_banks` 个 16 KiB。
    pub fn new(rom: Vec<u8>, rom_banks: usize, ram_len: usize, rumble: bool) -> Self {
        debug_assert_eq!(rom.len(), rom_banks * ROM_BANK);
        let mut m = Self {
            rom,
            rom_banks,
            ram: vec![0; ram_len],
            rumble,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            romx_off: 0,
        };
        m.remap();
        m
    }

    fn remap(&mut self) {
        self.romx_off = (self.rom_bank as usize % self.rom_banks) * ROM_BANK;
    }

    fn ram_index(&self, addr: u16) -> Option<usize> {
        if !self.ram_enabled || self.ram.is_empty() {
            return None;
        }
        let sel = if self.rumble { self.ram_bank & 0x07 } else { self.ram_bank } as usize;
        let banks = (self.ram.len() / RAM_BANK).max(1);
        Some(((sel % banks) * RAM_BANK + (addr as usize & 0x1FFF)) % self.ram.len())
    }
}

impl Cart for Mbc5 {
    fn read_rom(&self, addr: u16) -> u8 {
        let a = addr as usize & 0x3FFF;
        if addr < 0x4000 {
            self.rom[a]
        } else {
            self.rom[self.romx_off + a]
        }
    }

    fn write_rom(&mut self, addr: u16, v: u8) {
        match addr & 0x7FFF {
            0x0000..=0x1FFF => self.ram_enabled = v & 0x0F == 0x0A,
            0x2000..=0x2FFF => {
                self.rom_bank = (self.rom_bank & 0x100) | v as u16;
                self.remap();
            }
            0x3000..=0x3FFF => {
                self.rom_bank = (self.rom_bank & 0xFF) | ((v as u16 & 1) << 8);
                self.remap();
            }
            0x4000..=0x5FFF => self.ram_bank = v & 0x0F,
            _ => {}
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        self.ram_index(addr).map_or(0xFF, |i| self.ram[i])
    }

    fn write_ram(&mut self, addr: u16, v: u8) {
        if let Some(i) = self.ram_index(addr) {
            self.ram[i] = v;
        }
    }
}

impl Snapshot for Mbc5 {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CART");
        w.u8(KIND_MBC5);
        w.bool(self.ram_enabled);
        w.u16(self.rom_bank);
        w.u8(self.ram_bank);
        w.vec(&self.ram);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CART")?;
        if r.u8()? != KIND_MBC5 {
            return Err(StateError::Corrupt("cart kind"));
        }
        let ram_enabled = r.bool()?;
        let rom_bank = r.u16()?;
        let ram_bank = r.u8()?;
        if rom_bank > 0x1FF || ram_bank > 0x0F {
            return Err(StateError::Corrupt("MBC5 registers"));
        }
        let ram = load_ram(r, self.ram.len())?;
        self.ram_enabled = ram_enabled;
        self.rom_bank = rom_bank;
        self.ram_bank = ram_bank;
        self.ram = ram;
        self.remap();
        Ok(())
    }
}
