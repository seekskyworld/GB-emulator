//! T51：Joypad（PLAN §3.7）。

use gb_core::bus::Bus;
use gb_core::buttons::*;
use gb_core::cart;
use gb_core::irq;
use gb_core::joypad::Joypad;
use gb_core::{CpuBus, Peripheral, Snapshot, StateReader, StateWriter};

const P1: u16 = 0xFF00;
/// 只选方向键（bit4=0）。
const SEL_DPAD: u8 = 0x20;
/// 只选功能键（bit5=0）。
const SEL_BUTTONS: u8 = 0x10;
const SEL_BOTH: u8 = 0x00;
const SEL_NONE: u8 = 0x30;

fn pad(select: u8, mask: u8) -> Joypad {
    let mut j = Joypad::new();
    j.write_reg(P1, select);
    j.set_buttons(mask);
    j
}

#[test]
fn power_up_value() {
    assert_eq!(Joypad::new().read_reg(P1), 0xCF);
}

#[test]
fn dpad_group() {
    assert_eq!(pad(SEL_DPAD, 0).read_reg(P1), 0xEF);
    assert_eq!(pad(SEL_DPAD, RIGHT).read_reg(P1), 0xEE);
    assert_eq!(pad(SEL_DPAD, LEFT).read_reg(P1), 0xED);
    assert_eq!(pad(SEL_DPAD, UP).read_reg(P1), 0xEB);
    assert_eq!(pad(SEL_DPAD, DOWN).read_reg(P1), 0xE7);
    assert_eq!(pad(SEL_DPAD, UP | RIGHT).read_reg(P1), 0xEA);
    // 功能键不影响方向键组。
    assert_eq!(pad(SEL_DPAD, A | B | SELECT | START).read_reg(P1), 0xEF);
}

#[test]
fn button_group() {
    assert_eq!(pad(SEL_BUTTONS, 0).read_reg(P1), 0xDF);
    assert_eq!(pad(SEL_BUTTONS, A).read_reg(P1), 0xDE);
    assert_eq!(pad(SEL_BUTTONS, B).read_reg(P1), 0xDD);
    assert_eq!(pad(SEL_BUTTONS, SELECT).read_reg(P1), 0xDB);
    assert_eq!(pad(SEL_BUTTONS, START).read_reg(P1), 0xD7);
    assert_eq!(pad(SEL_BUTTONS, START | A).read_reg(P1), 0xD6);
    assert_eq!(pad(SEL_BUTTONS, RIGHT | LEFT | UP | DOWN).read_reg(P1), 0xDF);
}

#[test]
fn both_groups_selected_combine() {
    assert_eq!(pad(SEL_BOTH, 0).read_reg(P1), 0xCF);
    assert_eq!(pad(SEL_BOTH, A).read_reg(P1), 0xCE);
    assert_eq!(pad(SEL_BOTH, RIGHT).read_reg(P1), 0xCE);
    assert_eq!(pad(SEL_BOTH, B | UP).read_reg(P1), 0xC9);
    assert_eq!(pad(SEL_BOTH, 0xFF).read_reg(P1), 0xC0);
}

#[test]
fn no_group_selected_reads_f() {
    assert_eq!(pad(SEL_NONE, 0).read_reg(P1), 0xFF);
    assert_eq!(pad(SEL_NONE, 0xFF).read_reg(P1), 0xFF);
}

#[test]
fn only_select_bits_are_writable() {
    let mut j = Joypad::new();
    j.write_reg(P1, 0x0F); // 低 4 位只读、高 2 位恒 1
    assert_eq!(j.read_reg(P1), 0xCF);
    j.write_reg(P1, 0xF0);
    assert_eq!(j.read_reg(P1), 0xFF);
}

#[test]
fn interrupt_on_press_in_selected_group() {
    let mut j = pad(SEL_BUTTONS, 0);
    assert_eq!(j.tick_mcycle(), 0);
    j.set_buttons(START);
    assert_eq!(j.tick_mcycle(), irq::JOYPAD, "falling edge on selected group");
    assert_eq!(j.tick_mcycle(), 0, "held key does not re-trigger");
    j.set_buttons(START | A);
    assert_eq!(j.tick_mcycle(), irq::JOYPAD, "another line falls");
    j.set_buttons(0);
    assert_eq!(j.tick_mcycle(), 0, "release (0->1) does not trigger");
}

#[test]
fn no_interrupt_for_unselected_group() {
    let mut j = pad(SEL_BUTTONS, 0);
    j.tick_mcycle();
    j.set_buttons(RIGHT | DOWN);
    assert_eq!(j.tick_mcycle(), 0, "d-pad not selected");
    let mut j = pad(SEL_DPAD, 0);
    j.tick_mcycle();
    j.set_buttons(A | START);
    assert_eq!(j.tick_mcycle(), 0, "buttons not selected");
    j.set_buttons(A | START | LEFT);
    assert_eq!(j.tick_mcycle(), irq::JOYPAD, "d-pad selected");
    let mut j = pad(SEL_NONE, 0);
    j.tick_mcycle();
    j.set_buttons(0xFF);
    assert_eq!(j.tick_mcycle(), 0, "nothing selected");
}

#[test]
fn selecting_group_with_key_held_is_a_falling_edge() {
    // 选择线变化也会让低 4 位出现 1→0：按住 A 时从"不选"切到"选功能键"触发一次。
    let mut j = pad(SEL_NONE, A);
    assert_eq!(j.tick_mcycle(), 0);
    j.write_reg(P1, SEL_BUTTONS);
    assert_eq!(j.tick_mcycle(), irq::JOYPAD);
    assert_eq!(j.tick_mcycle(), 0);
}

#[test]
fn snapshot_roundtrip() {
    let mut a = pad(SEL_DPAD, UP | A);
    a.tick_mcycle();
    let mut w = StateWriter::new();
    a.save(&mut w);
    let data = w.into_inner();
    let mut b = Joypad::new();
    let mut r = StateReader::new(&data);
    b.load(&mut r).unwrap();
    assert_eq!(r.remaining(), 0);
    assert_eq!(a, b);
    assert_eq!(b.read_reg(P1), 0xEB);
    let mut bad = data.clone();
    bad[4] = 0x01;
    assert!(Joypad::new().load(&mut StateReader::new(&bad)).is_err());
}

#[test]
fn bus_set_buttons_reaches_p1_and_if() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x147] = 0x00;
    let mut b = Bus::new(cart::from_rom(rom).unwrap());
    b.write(0xFF0F, 0x00);
    b.write(P1, SEL_BUTTONS);
    b.idle();
    b.set_buttons(B);
    assert_eq!(b.peek(P1), 0xDD);
    assert_eq!(b.peek(0xFF0F) & irq::JOYPAD, 0, "requested on the next M-cycle");
    b.idle();
    assert_eq!(b.peek(0xFF0F) & irq::JOYPAD, irq::JOYPAD);
    b.write(0xFFFF, irq::JOYPAD);
    assert_eq!(b.pending(), irq::JOYPAD);
}
