//! T50b：MBC5 9 位 bank（写 0 就是 bank 0）、RAM 使能与 16 个 RAM bank、震动位、Snapshot 往返。

mod cart_util;

use cart_util::{bank_of, load, save, synth_rom, BANK};
use gb_core::cart::{self, Cart};

/// 超过 256 个 bank 时，bank 号的低 8 位写在首字节，第 9 位写在 0x2000 偏移处（便于检查 bit8）。
fn big_rom(banks: usize, kind: u8, ram_code: u8) -> Vec<u8> {
    let mut rom = synth_rom(banks, kind, ram_code);
    for b in 0..banks {
        rom[b * BANK + 0x2000] = (b >> 8) as u8;
    }
    rom
}

fn bank9(c: &dyn Cart) -> usize {
    bank_of(c, 0x4000) as usize | (c.read_rom(0x6000) as usize) << 8
}

#[test]
fn power_up_bank_1() {
    let c = cart::from_rom(synth_rom(8, 0x19, 0x00)).unwrap();
    assert_eq!(bank_of(&*c, 0x0000), 0);
    assert_eq!(bank_of(&*c, 0x4000), 1);
}

#[test]
fn writing_zero_selects_bank_zero() {
    let mut c = cart::from_rom(synth_rom(8, 0x19, 0x00)).unwrap();
    c.write_rom(0x2000, 0x00);
    assert_eq!(bank_of(&*c, 0x4000), 0, "MBC5: 0 is really bank 0");
    c.write_rom(0x2FFF, 0x05);
    assert_eq!(bank_of(&*c, 0x4000), 5);
}

#[test]
fn nine_bit_bank_number() {
    let mut c = cart::from_rom(big_rom(512, 0x19, 0x00)).unwrap(); // 8 MiB
    for b in [0usize, 1, 0x20, 0x40, 0x60, 0x7F, 0x80, 0xFF, 0x100, 0x101, 0x155, 0x1FF] {
        c.write_rom(0x2000, b as u8);
        c.write_rom(0x3000, (b >> 8) as u8);
        assert_eq!(bank9(&*c), b, "bank {b:#x}");
        assert_eq!(bank_of(&*c, 0x0000), 0);
    }
    // 低 8 位与第 9 位是两个独立的寄存器
    c.write_rom(0x2000, 0x22);
    c.write_rom(0x3FFF, 0x01);
    assert_eq!(bank9(&*c), 0x122);
    c.write_rom(0x2000, 0x33);
    assert_eq!(bank9(&*c), 0x133, "high bit kept when low byte changes");
    c.write_rom(0x3000, 0xFE); // 只用 bit0
    assert_eq!(bank9(&*c), 0x033);
}

#[test]
fn bank_wraps_modulo_rom_size() {
    let mut c = cart::from_rom(synth_rom(32, 0x19, 0x00)).unwrap();
    c.write_rom(0x2000, 0x25);
    assert_eq!(bank_of(&*c, 0x4000), 5);
    c.write_rom(0x3000, 0x01);
    assert_eq!(bank_of(&*c, 0x4000), 5, "bit 8 wraps away on a 512 KiB ROM");
}

#[test]
fn ram_enable_and_16_banks() {
    let mut c = cart::from_rom(synth_rom(8, 0x1B, 0x04)).unwrap(); // 128 KiB RAM
    c.write_ram(0xA000, 0x11);
    assert_eq!(c.read_ram(0xA000), 0xFF, "disabled at power-up");
    c.write_rom(0x0000, 0x0A);
    for b in 0..16u8 {
        c.write_rom(0x4000, b);
        c.write_ram(0xA000, 0x80 + b);
        c.write_ram(0xBFFF, 0x40 + b);
    }
    for b in 0..16u8 {
        c.write_rom(0x5FFF, b);
        assert_eq!(c.read_ram(0xA000), 0x80 + b, "RAM bank {b}");
        assert_eq!(c.read_ram(0xBFFF), 0x40 + b);
    }
    c.write_rom(0x1FFF, 0x00);
    assert_eq!(c.read_ram(0xA000), 0xFF);
    c.write_rom(0x0000, 0x1A); // 低 4 位为 A 即使能
    assert_eq!(c.read_ram(0xA000), 0x8F);
}

#[test]
fn ram_bank_wraps_modulo_ram_size() {
    let mut c = cart::from_rom(synth_rom(8, 0x1A, 0x03)).unwrap(); // 32 KiB
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x4000, 0x01);
    c.write_ram(0xA000, 0x77);
    c.write_rom(0x4000, 0x05);
    assert_eq!(c.read_ram(0xA000), 0x77);
}

#[test]
fn rumble_bit_does_not_select_ram() {
    let mut c = cart::from_rom(synth_rom(8, 0x1E, 0x04)).unwrap();
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x4000, 0x02);
    c.write_ram(0xA000, 0x22);
    c.write_rom(0x4000, 0x0A); // bit3 = 震动，RAM bank 仍是 2
    assert_eq!(c.read_ram(0xA000), 0x22);
}

#[test]
fn no_ram_types_read_ff() {
    for kind in [0x19u8, 0x1C] {
        let mut c = cart::from_rom(synth_rom(8, kind, 0x03)).unwrap();
        c.write_rom(0x0000, 0x0A);
        c.write_ram(0xA000, 0x12);
        assert_eq!(c.read_ram(0xA000), 0xFF, "type 0x{kind:02X}");
    }
}

#[test]
fn snapshot_roundtrip() {
    let rom = big_rom(512, 0x1B, 0x04);
    let mut a = cart::from_rom(rom.clone()).unwrap();
    a.write_rom(0x0000, 0x0A);
    for b in 0..16u8 {
        a.write_rom(0x4000, b);
        a.write_ram(0xA123, b ^ 0x5A);
    }
    a.write_rom(0x4000, 0x09);
    a.write_rom(0x2000, 0x34);
    a.write_rom(0x3000, 0x01);
    let data = save(&*a);
    assert_eq!(&data[..4], b"CART");
    let mut b = cart::from_rom(rom).unwrap();
    load(&mut *b, &data).unwrap();
    assert_eq!(bank9(&*b), 0x134);
    assert_eq!(b.read_ram(0xA123), 0x09 ^ 0x5A);
    b.write_rom(0x4000, 0x03);
    assert_eq!(b.read_ram(0xA123), 0x03 ^ 0x5A);
    assert!(load(&mut *b, &data[..data.len() - 2]).is_err());
    let mut small = cart::from_rom(big_rom(512, 0x1B, 0x03)).unwrap();
    assert!(load(&mut *small, &data).is_err(), "RAM size mismatch");
}
