//! T16：串口（PLAN §3.7）。

use gb_core::bus::Bus;
use gb_core::cart;
use gb_core::irq;
use gb_core::serial::Serial;
use gb_core::{CpuBus, Peripheral, Snapshot, StateReader, StateWriter};

const SB: u16 = 0xFF01;
const SC: u16 = 0xFF02;

fn ticks(s: &mut Serial, n: u32) -> u8 {
    (0..n).fold(0, |acc, _| acc | s.tick_mcycle())
}

#[test]
fn power_up_values() {
    let s = Serial::new();
    assert_eq!(s.read_reg(SB), 0x00);
    assert_eq!(s.read_reg(SC), 0x7E);
}

#[test]
fn sc_unused_bits_read_one() {
    let mut s = Serial::new();
    for (v, expect) in [(0x00u8, 0x7Eu8), (0x01, 0x7F), (0x80, 0xFE), (0x81, 0xFF), (0x7E, 0x7E), (0xFF, 0xFF)] {
        s.write_reg(SC, v);
        assert_eq!(s.read_reg(SC), expect, "SC write {v:02X}");
    }
}

#[test]
fn internal_clock_transfer_outputs_immediately_and_completes_after_4096_t() {
    let mut s = Serial::new();
    s.write_reg(SB, 0x41);
    s.write_reg(SC, 0x81);
    assert_eq!(s.take_output(), vec![0x41], "SB appended when the transfer starts");
    assert!(s.take_output().is_empty());
    assert_eq!(ticks(&mut s, 1023), 0, "not complete before 4096 T");
    assert_eq!(s.read_reg(SC) & 0x80, 0x80);
    assert_eq!(s.tick_mcycle(), irq::SERIAL, "complete at exactly 4096 T");
    assert_eq!(s.read_reg(SC), 0x7F, "SC.7 cleared, clock select kept");
    assert_eq!(s.read_reg(SB), 0xFF, "disconnected link shifts in 1s");
    assert_eq!(ticks(&mut s, 5000), 0, "only one interrupt per transfer");
}

#[test]
fn sb_shifts_in_ones_during_transfer() {
    let mut s = Serial::new();
    s.write_reg(SB, 0x00);
    s.write_reg(SC, 0x81);
    for bit in 1..=8u32 {
        ticks(&mut s, 128); // 512 T 一位
        assert_eq!(s.read_reg(SB), ((1u16 << bit) - 1) as u8, "after {bit} bits");
    }
}

#[test]
fn external_clock_transfer_never_completes() {
    let mut s = Serial::new();
    s.write_reg(SB, 0x55);
    s.write_reg(SC, 0x80);
    assert!(s.take_output().is_empty(), "external clock does not send");
    assert_eq!(ticks(&mut s, 100_000), 0);
    assert_eq!(s.read_reg(SC), 0xFE, "SC.7 stays set");
    assert_eq!(s.read_reg(SB), 0x55);
}

#[test]
fn each_start_appends_sb_even_without_waiting() {
    // 软件可以不等 SC.7 清零就写下一个字符：每次启动都记录当时的 SB。
    let mut s = Serial::new();
    for &c in b"Hi!\n" {
        s.write_reg(SB, c);
        s.write_reg(SC, 0x81);
        ticks(&mut s, 10);
    }
    assert_eq!(s.take_output(), b"Hi!\n".to_vec());
    // 重新启动后计时从头开始：最后一次启动后满 4096 T 才完成。
    assert_eq!(ticks(&mut s, 1013), 0);
    assert_eq!(s.tick_mcycle(), irq::SERIAL);
}

#[test]
fn clearing_sc7_aborts_transfer() {
    let mut s = Serial::new();
    s.write_reg(SB, 0x12);
    s.write_reg(SC, 0x81);
    ticks(&mut s, 100);
    s.write_reg(SC, 0x01);
    assert_eq!(ticks(&mut s, 5000), 0);
    assert_eq!(s.read_reg(SC), 0x7F);
}

#[test]
fn snapshot_roundtrip_mid_transfer() {
    let mut a = Serial::new();
    a.write_reg(SB, 0x3C);
    a.write_reg(SC, 0x81);
    ticks(&mut a, 300);
    let mut w = StateWriter::new();
    a.save(&mut w);
    let data = w.into_inner();
    let mut b = Serial::new();
    let mut r = StateReader::new(&data);
    b.load(&mut r).unwrap();
    assert_eq!(r.remaining(), 0);
    assert_eq!(b.read_reg(SB), a.read_reg(SB));
    assert_eq!(b.read_reg(SC), a.read_reg(SC));
    assert!(b.take_output().is_empty(), "output buffer is host-side, not saved");
    assert_eq!(ticks(&mut b, 723), 0);
    assert_eq!(b.tick_mcycle(), irq::SERIAL);
    // 损坏的数据被拒绝。
    let mut bad = data.clone();
    bad[5] = 0x02; // SC 非法位
    assert!(Serial::new().load(&mut StateReader::new(&bad)).is_err());
}

fn bus() -> Bus {
    let mut rom = vec![0u8; 0x8000];
    rom[0x147] = 0x00;
    Bus::new(cart::from_rom(rom).unwrap())
}

#[test]
fn bus_serial_transfer_requests_interrupt_after_4096_t() {
    let mut b = bus();
    b.write(0xFF0F, 0x00);
    b.write(SB, b'P');
    b.write(SC, 0x81);
    let start = b.mcycles();
    assert_eq!(b.take_serial(), vec![b'P']);
    while b.peek(0xFF0F) & irq::SERIAL == 0 {
        b.idle();
        assert!(b.mcycles() - start <= 1024, "serial transfer did not complete in 4096 T");
    }
    assert_eq!((b.mcycles() - start) * 4, 4096);
    assert_eq!(b.read(SC) & 0x80, 0);
    assert_eq!(b.read(SB), 0xFF);
    b.write(0xFFFF, irq::SERIAL);
    assert_eq!(b.pending(), irq::SERIAL);
}

#[test]
fn bus_external_clock_never_interrupts() {
    let mut b = bus();
    b.write(0xFF0F, 0x00);
    b.write(SC, 0x80);
    for _ in 0..20_000 {
        b.idle();
    }
    assert_eq!(b.peek(0xFF0F) & irq::SERIAL, 0);
    assert_eq!(b.peek(SC), 0xFE);
    assert!(b.take_serial().is_empty());
}
