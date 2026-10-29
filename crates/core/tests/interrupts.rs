//! T14（IF/IE 侧）：IF/IE 寄存器、外设请求汇入 IF、`CpuBus::pending/ack`（PLAN §3.2、§3.3）。

use gb_core::bus::Bus;
use gb_core::buttons;
use gb_core::cart;
use gb_core::interrupts::Interrupts;
use gb_core::irq;
use gb_core::{CpuBus, Snapshot, StateReader, StateWriter};

const IF: u16 = 0xFF0F;
const IE: u16 = 0xFFFF;

fn bus() -> Bus {
    let mut rom = vec![0u8; 0x8000];
    rom[0x147] = 0x00;
    Bus::new(cart::from_rom(rom).unwrap())
}

#[test]
fn power_up_if_e1_ie_00() {
    let b = bus();
    assert_eq!(b.peek(IF), 0xE1);
    assert_eq!(b.peek(IE), 0x00);
    assert_eq!(b.pending(), 0);
}

#[test]
fn if_reads_upper_three_bits_as_one() {
    let mut b = bus();
    for v in 0..=0xFFu8 {
        b.write(IF, v);
        assert_eq!(b.read(IF), 0xE0 | (v & 0x1F), "IF write {v:02X}");
    }
}

#[test]
fn ie_keeps_all_bits_but_pending_masks_to_five() {
    let mut b = bus();
    b.write(IE, 0xFF);
    assert_eq!(b.read(IE), 0xFF);
    b.write(IF, 0xFF);
    assert_eq!(b.pending(), 0x1F);
    b.write(IE, 0xE0);
    assert_eq!(b.pending(), 0);
}

#[test]
fn pending_is_ie_and_if() {
    let mut b = bus();
    b.write(IF, irq::VBLANK | irq::TIMER | irq::JOYPAD);
    b.write(IE, irq::TIMER | irq::SERIAL | irq::JOYPAD);
    assert_eq!(b.pending(), irq::TIMER | irq::JOYPAD);
}

#[test]
fn ack_clears_only_that_bit() {
    let mut b = bus();
    b.write(IE, 0x1F);
    b.write(IF, 0x1F);
    for bit in 0..5u8 {
        b.ack(bit);
        let left = 0x1F & !((2u8 << bit) - 1);
        assert_eq!(b.pending(), left, "after ack({bit})");
        assert_eq!(b.peek(IF), 0xE0 | left);
    }
}

#[test]
fn peripheral_requests_accumulate_in_if_while_idle() {
    // HALT 期间 CPU 只调用 idle：外设照常推进，请求照常进 IF，pending 随之出现。
    let mut b = bus();
    b.write(IF, 0x00);
    b.write(IE, irq::TIMER | irq::SERIAL | irq::JOYPAD);
    b.write(0xFF06, 0x00); // TMA
    b.write(0xFF07, 0x00);
    b.write(0xFF04, 0x00); // 周期 0：计数器清零
    b.write(0xFF05, 0xF0); // 周期 1：TIMA 再加 16 次溢出
    b.write(0xFF07, 0x05); // 周期 2：每 16 T；首次加 1 在周期 4，溢出在周期 64，中断在周期 65
    b.write(0xFF01, 0x00); // 周期 3
    b.write(0xFF02, 0x81); // 周期 4：串口 4096 T 后完成
    b.write(0xFF00, 0x10); // 周期 5：选功能键
    let mut first_timer = None;
    for n in 1..=1100u32 {
        b.idle(); // 周期 5 + n
        if first_timer.is_none() && b.pending() & irq::TIMER != 0 {
            first_timer = Some(n);
        }
    }
    assert_eq!(first_timer, Some(60), "timer IRQ on M-cycle 65 after the DIV reset");
    assert_eq!(b.pending(), irq::TIMER | irq::SERIAL);
    // 请求是"或"进去的：已置位的位不会被后来的请求清掉。
    b.set_buttons(buttons::START);
    b.idle();
    assert_eq!(b.pending(), irq::TIMER | irq::SERIAL | irq::JOYPAD);
    assert_eq!(b.peek(IF), 0xE0 | irq::TIMER | irq::SERIAL | irq::JOYPAD);
    // CPU 写 IF 可以丢弃请求。
    b.write(IF, irq::SERIAL);
    assert_eq!(b.pending(), irq::SERIAL);
}

#[test]
fn requests_set_if_even_when_ie_disabled() {
    let mut b = bus();
    b.write(IF, 0x00);
    b.write(IE, 0x00);
    b.write(0xFF02, 0x81);
    for _ in 0..1024 {
        b.idle();
    }
    assert_eq!(b.peek(IF), 0xE0 | irq::SERIAL);
    assert_eq!(b.pending(), 0);
    b.write(IE, irq::SERIAL);
    assert_eq!(b.pending(), irq::SERIAL);
}

#[test]
fn interrupts_snapshot_roundtrip() {
    let mut a = Interrupts::new();
    a.write_if(0x0A);
    a.write_ie(0x1C);
    let mut w = StateWriter::new();
    a.save(&mut w);
    let data = w.into_inner();
    let mut b = Interrupts::new();
    let mut r = StateReader::new(&data);
    b.load(&mut r).unwrap();
    assert_eq!(r.remaining(), 0);
    assert_eq!(b.read_if(), 0xEA);
    assert_eq!(b.read_ie(), 0x1C);
    assert_eq!(b.pending(), 0x08);
    // 标签错位或非法 IF 位被拒绝。
    assert!(Interrupts::new().load(&mut StateReader::new(b"XXXX\x00\x00")).is_err());
    assert!(Interrupts::new().load(&mut StateReader::new(b"INTR\xE0\x00")).is_err());
    assert!(Interrupts::new().load(&mut StateReader::new(b"INTR\x01")).is_err());
}
