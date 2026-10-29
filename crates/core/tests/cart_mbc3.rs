//! T50：MBC3 bank 映射、写 0 行为、RAM 使能与 RAM bank、RTC 寄存器存储、Snapshot 往返（合成 ROM）。

mod cart_util;

use cart_util::{bank_of, load, save, synth_rom};
use gb_core::cart::{self, Cart};

fn mbc3(banks: usize, kind: u8, ram_code: u8) -> Box<dyn Cart> {
    cart::from_rom(synth_rom(banks, kind, ram_code)).expect("MBC3 cart")
}

#[test]
fn all_mbc3_types_accepted() {
    for kind in 0x0F..=0x13u8 {
        let ram = if matches!(kind, 0x10 | 0x12 | 0x13) { 0x03 } else { 0x00 };
        assert!(cart::from_rom(synth_rom(8, kind, ram)).is_ok(), "type 0x{kind:02X}");
    }
}

#[test]
fn seven_bit_bank_register_and_zero_is_one() {
    let mut c = mbc3(128, 0x11, 0x00);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    for b in 1..128u8 {
        c.write_rom(0x2000, b);
        assert_eq!(bank_of(&*c, 0x4000), b, "bank {b}");
        assert_eq!(bank_of(&*c, 0x0000), 0, "0000 region fixed to bank 0");
    }
    // 与 MBC1 不同：0x20/0x40/0x60 可以直接访问
    for b in [0x20u8, 0x40, 0x60] {
        c.write_rom(0x3FFF, b);
        assert_eq!(bank_of(&*c, 0x4000), b);
    }
    c.write_rom(0x2000, 0x00);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    // 只有 7 位：0x80 → 0 → 1；0x85 → 5
    c.write_rom(0x2000, 0x80);
    assert_eq!(bank_of(&*c, 0x4000), 1);
    c.write_rom(0x2000, 0x85);
    assert_eq!(bank_of(&*c, 0x4000), 5);
}

#[test]
fn bank_wraps_modulo_rom_size() {
    // 宝可梦红的头部：类型 0x13、ROM 1 MiB（0x148=0x05，64 bank）、RAM 32 KiB（0x149=0x03）
    let rom = synth_rom(64, 0x13, 0x03);
    assert_eq!(rom[0x148], 0x05);
    let mut c = cart::from_rom(rom).unwrap();
    c.write_rom(0x2000, 0x3F);
    assert_eq!(bank_of(&*c, 0x4000), 0x3F);
    c.write_rom(0x2000, 0x41);
    assert_eq!(bank_of(&*c, 0x4000), 0x01);
    c.write_rom(0x2000, 0x40);
    assert_eq!(bank_of(&*c, 0x4000), 0x00, "0x40 is not 0, so no 0->1; wraps to bank 0");
}

#[test]
fn ram_enable_and_banks_0_to_3() {
    let mut c = mbc3(64, 0x13, 0x03);
    c.write_ram(0xA000, 0x55);
    assert_eq!(c.read_ram(0xA000), 0xFF, "disabled at power-up");
    c.write_rom(0x0000, 0x0A);
    assert_eq!(c.read_ram(0xA000), 0x00);
    for b in 0..4u8 {
        c.write_rom(0x4000, b);
        c.write_ram(0xA000, 0x10 + b);
        c.write_ram(0xBFFF, 0x20 + b);
    }
    for b in 0..4u8 {
        c.write_rom(0x5FFF, b);
        assert_eq!(c.read_ram(0xA000), 0x10 + b, "RAM bank {b}");
        assert_eq!(c.read_ram(0xBFFF), 0x20 + b);
    }
    c.write_rom(0x1FFF, 0x00);
    assert_eq!(c.read_ram(0xA000), 0xFF);
    c.write_ram(0xA000, 0xEE);
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x4000, 3);
    assert_eq!(c.read_ram(0xA000), 0x13, "write while disabled was ignored");
}

#[test]
fn rtc_registers_are_storage_on_timer_carts() {
    let mut c = mbc3(8, 0x10, 0x03);
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x4000, 0x00);
    c.write_ram(0xA000, 0x99);
    for reg in 0x08..=0x0Cu8 {
        c.write_rom(0x4000, reg);
        c.write_ram(0xA000, 0x30 + reg);
    }
    // 锁存写入不影响存储
    c.write_rom(0x6000, 0x00);
    c.write_rom(0x6000, 0x01);
    for reg in 0x08..=0x0Cu8 {
        c.write_rom(0x4000, reg);
        assert_eq!(c.read_ram(0xA000), 0x30 + reg, "RTC reg {reg:02X}");
        assert_eq!(c.read_ram(0xB000), 0x30 + reg, "RTC reg mapped over whole area");
    }
    c.write_rom(0x4000, 0x00);
    assert_eq!(c.read_ram(0xA000), 0x99, "RAM not clobbered by RTC writes");
    // 选择 0x0D 以上：读 0xFF
    c.write_rom(0x4000, 0x0D);
    assert_eq!(c.read_ram(0xA000), 0xFF);
    // RTC 也受使能控制
    c.write_rom(0x4000, 0x08);
    c.write_rom(0x0000, 0x00);
    assert_eq!(c.read_ram(0xA000), 0xFF);
}

#[test]
fn no_rtc_on_non_timer_carts() {
    let mut c = mbc3(8, 0x13, 0x03);
    c.write_rom(0x0000, 0x0A);
    c.write_rom(0x4000, 0x08);
    c.write_ram(0xA000, 0x12);
    assert_eq!(c.read_ram(0xA000), 0xFF);
}

#[test]
fn no_ram_type_reads_ff() {
    let mut c = mbc3(8, 0x11, 0x03);
    c.write_rom(0x0000, 0x0A);
    c.write_ram(0xA000, 0x12);
    assert_eq!(c.read_ram(0xA000), 0xFF);
}

#[test]
fn snapshot_roundtrip_restores_registers_rtc_and_ram() {
    let rom = synth_rom(64, 0x10, 0x03);
    let mut a = cart::from_rom(rom.clone()).unwrap();
    a.write_rom(0x0000, 0x0A);
    for b in 0..4u8 {
        a.write_rom(0x4000, b);
        a.write_ram(0xA100, 0x60 + b);
    }
    a.write_rom(0x4000, 0x0A);
    a.write_ram(0xA000, 0x17);
    a.write_rom(0x6000, 0x01);
    a.write_rom(0x4000, 0x02);
    a.write_rom(0x2000, 0x2A);
    let data = save(&*a);
    assert_eq!(&data[..4], b"CART");

    let mut b = cart::from_rom(rom).unwrap();
    load(&mut *b, &data).unwrap();
    assert_eq!(bank_of(&*b, 0x4000), 0x2A);
    assert_eq!(b.read_ram(0xA100), 0x62, "RAM bank and enable restored");
    b.write_rom(0x4000, 0x0A);
    assert_eq!(b.read_ram(0xA000), 0x17, "RTC register restored");
    b.write_rom(0x4000, 0x00);
    assert_eq!(b.read_ram(0xA100), 0x60);
    assert_eq!(save(&*a), data);

    let mut c = cart::from_rom(synth_rom(64, 0x10, 0x03)).unwrap();
    assert!(load(&mut *c, &data[..data.len() - 3]).is_err());
    let mut d = cart::from_rom(synth_rom(64, 0x10, 0x02)).unwrap();
    assert!(load(&mut *d, &data).is_err(), "RAM size mismatch");
    let mut e = cart::from_rom(synth_rom(64, 0x03, 0x03)).unwrap();
    assert!(load(&mut *e, &data).is_err(), "kind mismatch");
}
