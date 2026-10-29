//! T50b：MBC2 bank 映射（地址 bit8 选寄存器）、写 0、内置 512×4 bit RAM 与 0x1FF 镜像、Snapshot 往返。

mod cart_util;

use cart_util::{bank_of, load, save, synth_rom};
use gb_core::cart::{self, Cart};

fn mbc2(banks: usize) -> Box<dyn Cart> {
    cart::from_rom(synth_rom(banks, 0x06, 0x00)).expect("MBC2 cart")
}

#[test]
fn power_up_bank_1() {
    let c = mbc2(16);
    assert_eq!(bank_of(&*c, 0x0000), 0);
    assert_eq!(bank_of(&*c, 0x4000), 1);
}

#[test]
fn bank_register_needs_address_bit8() {
    let mut c = mbc2(16);
    for b in 1..16u8 {
        c.write_rom(0x2100, b);
        assert_eq!(bank_of(&*c, 0x4000), b);
    }
    // bit8 = 0 的地址写的是 RAM 使能，不改 bank
    c.write_rom(0x2000, 0x03);
    c.write_rom(0x0000, 0x05);
    c.write_rom(0x3E00, 0x07);
    assert_eq!(bank_of(&*c, 0x4000), 15);
    // 0000–3FFF 内任何 bit8 = 1 的地址都行
    c.write_rom(0x0100, 0x03);
    assert_eq!(bank_of(&*c, 0x4000), 3);
    c.write_rom(0x3FFF, 0x05);
    assert_eq!(bank_of(&*c, 0x4000), 5);
    // 4000–7FFF 的写入无效
    c.write_rom(0x4100, 0x09);
    c.write_rom(0x7FFF, 0x09);
    assert_eq!(bank_of(&*c, 0x4000), 5);
    assert_eq!(bank_of(&*c, 0x0000), 0);
}

#[test]
fn zero_is_one_and_only_4_bits() {
    let mut c = mbc2(16);
    c.write_rom(0x2100, 0x07);
    c.write_rom(0x2100, 0x00);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    c.write_rom(0x2100, 0x10); // 低 4 位为 0 → 1
    assert_eq!(bank_of(&*c, 0x4000), 1);
    c.write_rom(0x2100, 0xF4);
    assert_eq!(bank_of(&*c, 0x4000), 4);
}

#[test]
fn bank_wraps_modulo_rom_size() {
    let mut c = mbc2(8); // 128 KiB
    c.write_rom(0x2100, 0x0B);
    assert_eq!(bank_of(&*c, 0x4000), 3);
    c.write_rom(0x2100, 0x08); // 8 不是 0，不转换；取模为 bank 0
    assert_eq!(bank_of(&*c, 0x4000), 0);
}

#[test]
fn ram_enable_needs_bit8_clear_and_0a() {
    let mut c = mbc2(4);
    c.write_ram(0xA000, 0x05);
    assert_eq!(c.read_ram(0xA000), 0xFF, "disabled at power-up");
    c.write_rom(0x0100, 0x0A); // bit8 = 1：这是 bank 寄存器，不是使能
    assert_eq!(c.read_ram(0xA000), 0xFF);
    c.write_rom(0x0000, 0x0A);
    assert_eq!(c.read_ram(0xA000), 0xF0);
    c.write_rom(0x3E00, 0x1B);
    assert_eq!(c.read_ram(0xA000), 0xFF);
    c.write_rom(0x1200, 0x3A);
    assert_eq!(c.read_ram(0xA000), 0xF0, "write while disabled was ignored");
}

#[test]
fn ram_is_4_bit_with_upper_nibble_set() {
    let mut c = mbc2(4);
    c.write_rom(0x0000, 0x0A);
    c.write_ram(0xA000, 0xAB);
    assert_eq!(c.read_ram(0xA000), 0xFB);
    c.write_ram(0xA1FF, 0x00);
    assert_eq!(c.read_ram(0xA1FF), 0xF0);
    c.write_ram(0xA001, 0x5C);
    assert_eq!(c.read_ram(0xA001), 0xFC);
}

#[test]
fn ram_mirrors_every_0x200() {
    let mut c = mbc2(4);
    c.write_rom(0x0000, 0x0A);
    for i in 0..0x200u16 {
        c.write_ram(0xA000 + i, (i as u8) & 0x0F);
    }
    for mirror in 0..16u16 {
        for i in [0u16, 1, 0x0FF, 0x123, 0x1FF] {
            let a = 0xA000 + mirror * 0x200 + i;
            assert_eq!(c.read_ram(a), 0xF0 | (i as u8 & 0x0F), "{a:04X}");
        }
    }
    // 写镜像地址改的是同一个单元
    c.write_ram(0xBE05, 0x09);
    assert_eq!(c.read_ram(0xA005), 0xF9);
    assert_eq!(c.read_ram(0xB205), 0xF9);
}

#[test]
fn header_ram_size_is_ignored() {
    // MBC2 的 RAM 是内置的；头部 0x149 不论写什么都不影响（也不报错）
    let mut c = cart::from_rom(synth_rom(4, 0x05, 0x07)).unwrap();
    c.write_rom(0x0000, 0x0A);
    c.write_ram(0xA010, 0x03);
    assert_eq!(c.read_ram(0xA010), 0xF3);
}

#[test]
fn snapshot_roundtrip() {
    let rom = synth_rom(16, 0x06, 0x00);
    let mut a = cart::from_rom(rom.clone()).unwrap();
    a.write_rom(0x0000, 0x0A);
    a.write_rom(0x2100, 0x0D);
    for i in 0..0x200u16 {
        a.write_ram(0xA000 + i, (i * 7) as u8);
    }
    let data = save(&*a);
    assert_eq!(&data[..4], b"CART");
    let mut b = cart::from_rom(rom).unwrap();
    load(&mut *b, &data).unwrap();
    assert_eq!(bank_of(&*b, 0x4000), 0x0D);
    for i in 0..0x200u16 {
        assert_eq!(b.read_ram(0xA000 + i), 0xF0 | ((i * 7) as u8 & 0x0F));
    }
    assert!(load(&mut *b, &data[..data.len() - 1]).is_err());
    let mut other = cart::from_rom(synth_rom(16, 0x01, 0x00)).unwrap();
    assert!(load(&mut *other, &data).is_err(), "kind mismatch");
}
