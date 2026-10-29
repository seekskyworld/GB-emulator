//! MBC2（类型 0x05–0x06；platform 通道，PLAN §3.6，Pan Docs MBC2.md）。
//!
//! - 0000–3FFF：地址 bit8 = 0 时为 RAM 使能（写入值低 4 位为 0xA 时使能）；
//!   bit8 = 1 时为 4 位 ROM bank 寄存器（0 当作 1），bank 号按 ROM 实际 bank 数取模。
//! - 4000–7FFF 的写入无效。
//! - 内置 512×4 bit RAM：A000–BFFF 只用地址低 9 位（按 0x1FF 镜像）；读出时高 4 位为 1。

use super::{Cart, ROM_BANK};
use crate::state::{Snapshot, StateError, StateReader, StateWriter};

const KIND_MBC2: u8 = 2;
/// 内置 RAM 的半字节数。
const RAM_NIBBLES: usize = 512;

pub struct Mbc2 {
    rom: Vec<u8>,
    rom_banks: usize,
    /// 每个元素只用低 4 位。
    ram: [u8; RAM_NIBBLES],
    ram_enabled: bool,
    /// 4 位 ROM bank 寄存器（原值，未做 0→1 转换）。
    rom_bank: u8,
    romx_off: usize,
}

impl Mbc2 {
    /// `rom` 的长度必须是 `rom_banks` 个 16 KiB。
    pub fn new(rom: Vec<u8>, rom_banks: usize) -> Self {
        debug_assert_eq!(rom.len(), rom_banks * ROM_BANK);
        let mut m =
            Self { rom, rom_banks, ram: [0; RAM_NIBBLES], ram_enabled: false, rom_bank: 1, romx_off: 0 };
        m.remap();
        m
    }

    fn remap(&mut self) {
        let bank = if self.rom_bank == 0 { 1 } else { self.rom_bank as usize };
        self.romx_off = (bank % self.rom_banks) * ROM_BANK;
    }
}

impl Cart for Mbc2 {
    fn read_rom(&self, addr: u16) -> u8 {
        let a = addr as usize & 0x3FFF;
        if addr < 0x4000 {
            self.rom[a]
        } else {
            self.rom[self.romx_off + a]
        }
    }

    fn write_rom(&mut self, addr: u16, v: u8) {
        if addr >= 0x4000 {
            return;
        }
        if addr & 0x0100 == 0 {
            self.ram_enabled = v & 0x0F == 0x0A;
        } else {
            self.rom_bank = v & 0x0F;
            self.remap();
        }
    }

    fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        0xF0 | self.ram[addr as usize & 0x1FF]
    }

    fn write_ram(&mut self, addr: u16, v: u8) {
        if self.ram_enabled {
            self.ram[addr as usize & 0x1FF] = v & 0x0F;
        }
    }
}

impl Snapshot for Mbc2 {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"CART");
        w.u8(KIND_MBC2);
        w.bool(self.ram_enabled);
        w.u8(self.rom_bank);
        w.vec(&self.ram);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"CART")?;
        if r.u8()? != KIND_MBC2 {
            return Err(StateError::Corrupt("cart kind"));
        }
        let ram_enabled = r.bool()?;
        let rom_bank = r.u8()?;
        if rom_bank > 0x0F {
            return Err(StateError::Corrupt("MBC2 ROM bank"));
        }
        let ram = r.vec()?;
        if ram.len() != RAM_NIBBLES || ram.iter().any(|&n| n > 0x0F) {
            return Err(StateError::Corrupt("MBC2 RAM"));
        }
        self.ram_enabled = ram_enabled;
        self.rom_bank = rom_bank;
        self.ram.copy_from_slice(&ram);
        self.remap();
        Ok(())
    }
}
