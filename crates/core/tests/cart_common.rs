//! T50：卡带类型选择、ROM only、未知类型与头部错误（合成 ROM，不依赖 refs）。

mod cart_util;

use cart_util::{bank_of, synth_rom};
use gb_core::cart::{self, CartError};

#[test]
fn rom_only_maps_32k_and_ignores_writes() {
    let rom = synth_rom(2, 0x00, 0x00);
    let mut c = cart::from_rom(rom.clone()).unwrap();
    assert_eq!(bank_of(&*c, 0x0000), 0);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    c.write_rom(0x2000, 0x05);
    c.write_rom(0x0000, 0x0A);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    assert_eq!(c.read_rom(0x7FFF), rom[0x7FFF]);
    // 没有 RAM：读 0xFF、写忽略
    c.write_ram(0xA000, 0x12);
    assert_eq!(c.read_ram(0xA000), 0xFF);
}

#[test]
fn rom_plus_ram_type_08() {
    let mut c = cart::from_rom(synth_rom(2, 0x08, 0x02)).unwrap();
    c.write_ram(0xA123, 0x5A);
    assert_eq!(c.read_ram(0xA123), 0x5A);
    assert_eq!(c.read_ram(0xBFFF), 0x00);
}

#[test]
fn unsupported_types_are_errors_not_panics() {
    // MBC2、MBC5 在 T50b 才实现；其余为未知或不支持的类型。
    for kind in [0x05u8, 0x06, 0x0B, 0x0D, 0x19, 0x1B, 0x1E, 0x20, 0x22, 0xFC, 0xFE, 0xFF] {
        let r = cart::from_rom(synth_rom(4, kind, 0x00));
        assert_eq!(r.err(), Some(CartError::UnsupportedType(kind)), "type 0x{kind:02X}");
    }
}

#[test]
fn bad_header_sizes_are_errors() {
    assert_eq!(cart::from_rom(vec![0; 0x100]).err(), Some(CartError::BadSize(0x100)));
    assert_eq!(cart::from_rom(Vec::new()).err(), Some(CartError::BadSize(0)));
    let r = cart::from_rom(synth_rom(4, 0x03, 0x07));
    assert_eq!(r.err(), Some(CartError::UnsupportedRamSize(0x07)));
}

#[test]
fn short_rom_is_padded_with_ff() {
    let mut rom = vec![0u8; 0x200];
    rom[0x147] = 0x00;
    let c = cart::from_rom(rom).unwrap();
    assert_eq!(c.read_rom(0x01FF), 0x00);
    assert_eq!(c.read_rom(0x0200), 0xFF);
    assert_eq!(c.read_rom(0x7FFF), 0xFF);
}

#[test]
fn machine_accepts_64k_mbc1() {
    // Blargg 合集是 64 KiB 的 MBC1，T02 的临时实现会拒绝它。
    assert!(gb_core::Machine::new(synth_rom(4, 0x01, 0x00)).is_ok());
}
