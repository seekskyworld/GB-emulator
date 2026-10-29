//! 卡带单测共用：合成 ROM（每个 bank 的首字节与 0x1234 偏移处写入该 bank 号）。
#![allow(dead_code)]

use gb_core::cart::Cart;
use gb_core::{StateReader, StateWriter};

pub const BANK: usize = 0x4000;

/// `banks` 个 16 KiB bank；头部 0x147 = `kind`，0x148 按 bank 数，0x149 = `ram_code`。
pub fn synth_rom(banks: usize, kind: u8, ram_code: u8) -> Vec<u8> {
    let mut rom = vec![0u8; banks * BANK];
    for b in 0..banks {
        rom[b * BANK] = b as u8;
        rom[b * BANK + 0x1234] = b as u8 ^ 0xA5;
    }
    rom[0x147] = kind;
    rom[0x148] = (banks / 2).trailing_zeros() as u8;
    rom[0x149] = ram_code;
    rom
}

/// 读出某个区（0x0000 或 0x4000）当前映射的 bank 号，并校验 bank 内偏移也来自同一个 bank。
pub fn bank_of(c: &dyn Cart, base: u16) -> u8 {
    let b = c.read_rom(base);
    assert_eq!(c.read_rom(base + 0x1234), b ^ 0xA5, "bank-internal offset mismatch at {base:04X}");
    b
}

pub fn save(c: &dyn Cart) -> Vec<u8> {
    let mut w = StateWriter::new();
    c.save(&mut w);
    w.into_inner()
}

pub fn load(c: &mut dyn Cart, data: &[u8]) -> Result<(), gb_core::StateError> {
    let mut r = StateReader::new(data);
    c.load(&mut r)?;
    assert_eq!(r.remaining(), 0, "cart snapshot left trailing bytes");
    Ok(())
}
