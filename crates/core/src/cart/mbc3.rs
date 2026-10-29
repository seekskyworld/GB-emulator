//! MBC3（类型 0x0F–0x13；platform 通道，PLAN §3.6，Pan Docs MBC3.md）。
//!
//! - 0000–1FFF：RAM 与 RTC 使能，写入值低 4 位为 0xA 时使能，其余值禁用。
//! - 2000–3FFF：7 位 ROM bank 寄存器，0 当作 1（0x20/0x40/0x60 可直接访问）。
//! - 4000–5FFF：00–07 选 RAM bank（按实际 RAM bank 数取模，DMG 卡带最多 4 个），
//!   08–0C 选 RTC 寄存器（S/M/H/DL/DH）。其他值：A000–BFFF 读 0xFF、写忽略。
//! - 6000–7FFF：锁存寄存器。RTC 不走时，寄存器只是可读写存储，锁存值也只保存不生效。
//!
//! 只有带 TIMER 的类型（0x0F、0x10）有 RTC；其余类型选中 08–0C 时读 0xFF。

use super::{load_ram, Cart, RAM_BANK, ROM_BANK};
use crate::state::{Snapshot, StateError, StateReader, StateWriter};

const KIND_MBC3: u8 = 3;

pub struct Mbc3 {
    rom: Vec<u8>,
    rom_banks: usize,
    ram: Vec<u8>,
    has_rtc: bool,
    ram_enabled: bool,
    /// 7 位 ROM bank 寄存器（原值，未做 0→1 转换）。
    rom_bank: u8,
    /// 4000–5FFF 写入的原值（RAM bank 或 RTC 寄存器选择）。
    select: u8,
    /// RTC 寄存器 08–0C：秒、分、时、日低 8 位、日高位/停止/进位。
    rtc: [u8; 5],
    /// 最近一次写入 6000–7FFF 的值。
    latch: u8,
    romx_off: usize,
}

impl Mbc3 {
    /// `rom` 的长度必须是 `rom_banks` 个 16 KiB。
    pub fn new(rom: Vec<u8>, rom_banks: usize, ram_len: usize, has_rtc: bool) -> Self {
        debug_assert_eq!(rom.len(), rom_banks * ROM_BANK);
        let mut m = Self {
            rom,
            rom_banks,
            ram: vec![0; ram_len],
            has_rtc,
            ram_enabled: false,
            rom_bank: 0,
            select: 0,
            rtc: [0; 5],
            latch: 0xFF,
            romx_off: 0,
        };
        m.remap();
        m
    }

    fn remap(&mut self) {
        let bank = if self.rom_bank == 0 { 1 } else { self.rom_bank as usize };
        self.romx_off = (bank % self.rom_banks) * ROM_BANK;
    }

    fn ram_index(&self, addr: u16) -> Option<usize> {
        if self.ram.is_empty() {
            return None;
        }
        let banks = (self.ram.len() / RAM_BANK).max(1);
        let bank = self.select as usize % banks;
        Some((bank * RAM_BANK + (addr as usize & 0x1FFF)) % self.ram.len())
    }
}

impl Cart for Mbc3 {
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
            0x2000..=0x3FFF => {
                self.rom_bank = v & 0x7F;
                self.remap();
            }
            0x4000..=0x5FFF => self.select = v,
            _ => self.latch = v,
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        match self.select {
            0x00..=0x07 => self.ram_index(addr).map_or(0xFF, |i| self.ram[i]),
            0x08..=0x0C if self.has_rtc => self.rtc[(self.select - 0x08) as usize],
            _ => 0xFF,
        }
    }

    fn write_ram(&mut self, addr: u16, v: u8) {
        if !self.ram_enabled {
            return;
        }
        match self.select {
            0x00..=0x07 => {
                if let Some(i) = self.ram_index(addr) {
                    self.ram[i] = v;
                }
            }
            0x08..=0x0C if self.has_rtc => self.rtc[(self.select - 0x08) as usize] = v,
            _ => {}
        }
    }
}

impl Snapshot for Mbc3 {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CART");
        w.u8(KIND_MBC3);
        w.bool(self.ram_enabled);
        w.u8(self.rom_bank);
        w.u8(self.select);
        w.bytes(&self.rtc);
        w.u8(self.latch);
        w.vec(&self.ram);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CART")?;
        if r.u8()? != KIND_MBC3 {
            return Err(StateError::Corrupt("cart kind"));
        }
        let ram_enabled = r.bool()?;
        let rom_bank = r.u8()?;
        if rom_bank > 0x7F {
            return Err(StateError::Corrupt("MBC3 ROM bank"));
        }
        let select = r.u8()?;
        let mut rtc = [0u8; 5];
        r.bytes_into(&mut rtc)?;
        let latch = r.u8()?;
        let ram = load_ram(r, self.ram.len())?;
        self.ram_enabled = ram_enabled;
        self.rom_bank = rom_bank;
        self.select = select;
        self.rtc = rtc;
        self.latch = latch;
        self.ram = ram;
        self.remap();
        Ok(())
    }
}
