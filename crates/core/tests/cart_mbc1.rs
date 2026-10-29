//! T50：MBC1 bank 映射、写 0 行为、RAM 使能与 RAM bank、Snapshot 往返（合成 ROM）。

mod cart_util;

use cart_util::{bank_of, load, save, synth_rom};
use gb_core::cart::{self, Cart};

fn mbc1(banks: usize, kind: u8, ram_code: u8) -> Box<dyn Cart> {
    cart::from_rom(synth_rom(banks, kind, ram_code)).expect("MBC1 cart")
}

#[test]
fn power_up_maps_bank_0_and_1() {
    let c = mbc1(64, 0x01, 0x00);
    assert_eq!(bank_of(&*c, 0x0000), 0);
    assert_eq!(bank_of(&*c, 0x4000), 1);
}

#[test]
fn rom_bank_register_selects_bank() {
    let mut c = mbc1(32, 0x01, 0x00);
    for b in 1..32u8 {
        c.write_rom(0x2000, b);
        assert_eq!(bank_of(&*c, 0x4000), b, "bank {b}");
        assert_eq!(bank_of(&*c, 0x0000), 0);
    }
    // 整个 2000–3FFF 都是这个寄存器
    c.write_rom(0x3FFF, 0x07);
    assert_eq!(bank_of(&*c, 0x4000), 7);
}

#[test]
fn writing_zero_selects_one() {
    let mut c = mbc1(32, 0x01, 0x00);
    c.write_rom(0x2000, 0x05);
    c.write_rom(0x2000, 0x00);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    // 只看低 5 位：0xE0 的低 5 位为 0 → bank 1；0xE1 → bank 1
    c.write_rom(0x2000, 0xE0);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    c.write_rom(0x2000, 0xE3);
    assert_eq!(bank_of(&*c, 0x4000), 3);
}

#[test]
fn banks_20_40_60_map_to_21_41_61() {
    // 2 MiB（128 bank）：高位寄存器给出 bit5–6，低 5 位为 0 时当作 1。
    let mut c = mbc1(128, 0x01, 0x00);
    c.write_rom(0x2000, 0x00);
    for (hi, want) in [(1u8, 0x21u8), (2, 0x41), (3, 0x61)] {
        c.write_rom(0x4000, hi);
        assert_eq!(bank_of(&*c, 0x4000), want, "hi={hi}");
    }
    // 高位寄存器配合非零低位
    c.write_rom(0x4000, 2);
    c.write_rom(0x2000, 0x1F);
    assert_eq!(bank_of(&*c, 0x4000), 0x5F);
}

#[test]
fn mode1_applies_upper_bits_to_0000_region() {
    let mut c = mbc1(128, 0x01, 0x00);
    c.write_rom(0x4000, 3);
    assert_eq!(bank_of(&*c, 0x0000), 0, "mode 0: 0000 locked to bank 0");
    c.write_rom(0x6000, 0x01);
    assert_eq!(bank_of(&*c, 0x0000), 0x60);
    assert_eq!(bank_of(&*c, 0x4000), 0x61);
    c.write_rom(0x4000, 1);
    assert_eq!(bank_of(&*c, 0x0000), 0x20);
    c.write_rom(0x7FFF, 0x00);
    assert_eq!(bank_of(&*c, 0x0000), 0);
    assert_eq!(bank_of(&*c, 0x4000), 0x21);
}

#[test]
fn bank_number_wraps_modulo_rom_size() {
    // 4 bank（64 KiB，Blargg 合集的大小）
    let mut c = mbc1(4, 0x01, 0x00);
    c.write_rom(0x2000, 0x05);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    c.write_rom(0x2000, 0x07);
    assert_eq!(bank_of(&*c, 0x4000), 3);
    // 0x04 不是 0，不做 0→1 转换；取模后为 bank 0
    c.write_rom(0x2000, 0x04);
    assert_eq!(bank_of(&*c, 0x4000), 0);
    // 高位寄存器在小 ROM 上被取模掉
    c.write_rom(0x2000, 0x02);
    c.write_rom(0x4000, 0x03);
    assert_eq!(bank_of(&*c, 0x4000), 2);
    c.write_rom(0x6000, 0x01);
    assert_eq!(bank_of(&*c, 0x0000), 0);
}

#[test]
fn ram_disabled_by_default_and_needs_0a() {
    let mut c = mbc1(4, 0x03, 0x03);
    c.write_ram(0xA000, 0x11);
    assert_eq!(c.read_ram(0xA000), 0xFF, "disabled at power-up");
    c.write_rom(0x0000, 0x0A);
    assert_eq!(c.read_ram(0xA000), 0x00, "write while disabled was ignored");
    c.write_ram(0xA000, 0x11);
    c.write_ram(0xBFFF, 0x22);
    assert_eq!(c.read_ram(0xA000), 0x11);
    assert_eq!(c.read_ram(0xBFFF), 0x22);
    // 任何低 4 位不是 A 的值都会禁用
    c.write_rom(0x1FFF, 0x0B);
    assert_eq!(c.read_ram(0xA000), 0xFF);
    c.write_ram(0xA000, 0x99);
    // 低 4 位为 A 的其它值也能使能
    c.write_rom(0x1000, 0xFA);
    assert_eq!(c.read_ram(0xA000), 0x11);
    c.write_rom(0x0000, 0x00);
    assert_eq!(c.read_ram(0xA000), 0xFF);
}

#[test]
fn ram_banks_switch_only_in_mode1() {
    let mut c = mbc1(4, 0x03, 0x03); // 32 KiB RAM
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x6000, 0x01);
    for b in 0..4u8 {
        c.write_rom(0x4000, b);
        c.write_ram(0xA000, 0x40 + b);
        c.write_ram(0xB234, 0x80 + b);
    }
    for b in 0..4u8 {
        c.write_rom(0x4000, b);
        assert_eq!(c.read_ram(0xA000), 0x40 + b, "RAM bank {b}");
        assert_eq!(c.read_ram(0xB234), 0x80 + b);
    }
    // mode 0：RAM 锁定在 bank 0
    c.write_rom(0x4000, 3);
    c.write_rom(0x6000, 0x00);
    assert_eq!(c.read_ram(0xA000), 0x40);
}

#[test]
fn ram_8k_ignores_bank_register() {
    let mut c = mbc1(4, 0x02, 0x02);
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x6000, 0x01);
    c.write_ram(0xA000, 0x77);
    c.write_rom(0x4000, 2);
    assert_eq!(c.read_ram(0xA000), 0x77, "single 8 KiB bank wraps");
}

#[test]
fn no_ram_type_reads_ff_even_when_enabled() {
    let mut c = mbc1(4, 0x01, 0x03);
    c.write_rom(0x0000, 0x0A);
    c.write_ram(0xA000, 0x12);
    assert_eq!(c.read_ram(0xA000), 0xFF);
}

#[test]
fn snapshot_roundtrip_restores_registers_and_ram() {
    let rom = synth_rom(128, 0x03, 0x03);
    let mut a = cart::from_rom(rom.clone()).unwrap();
    a.write_rom(0x0000, 0x0A);
    a.write_rom(0x6000, 0x01);
    for b in 0..4u8 {
        a.write_rom(0x4000, b);
        a.write_ram(0xA000 + b as u16, 0x30 + b);
    }
    a.write_rom(0x4000, 2);
    a.write_rom(0x2000, 0x13);
    let data = save(&*a);
    assert_eq!(&data[..4], b"CART");

    let mut b = cart::from_rom(rom).unwrap();
    load(&mut *b, &data).unwrap();
    assert_eq!(bank_of(&*b, 0x4000), 0x53);
    assert_eq!(bank_of(&*b, 0x0000), 0x40);
    assert_eq!(b.read_ram(0xA002), 0x32);
    b.write_rom(0x4000, 1);
    assert_eq!(b.read_ram(0xA001), 0x31);
    assert_eq!(save(&*a), data, "save is deterministic");

    // 截断与错位的存档报错
    let mut c = cart::from_rom(synth_rom(128, 0x03, 0x03)).unwrap();
    assert!(load(&mut *c, &data[..data.len() - 1]).is_err());
    assert!(load(&mut *c, b"XXXX").is_err());
    // RAM 大小不同的卡带拒绝读档
    let mut d = cart::from_rom(synth_rom(128, 0x03, 0x02)).unwrap();
    assert!(load(&mut *d, &data).is_err());
    // 不同种类的卡带拒绝读档
    let mut e = cart::from_rom(synth_rom(128, 0x13, 0x03)).unwrap();
    assert!(load(&mut *e, &data).is_err());
}
