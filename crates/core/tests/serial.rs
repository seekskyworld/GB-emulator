//! T16 / W4：串口（PLAN §3.7；内部时钟取自系统计数器 bit 8 的下降沿，见 `serial.rs`）。

use gb_core::bus::Bus;
use gb_core::cart;
use gb_core::irq;
use gb_core::serial::Serial;
use gb_core::{CpuBus, Peripheral, Snapshot, StateReader, StateWriter};

const SB: u16 = 0xFF01;
const SC: u16 = 0xFF02;

/// 串口加一个每 M-cycle 加 4 的系统计数器，接线方式与 Bus 相同。
struct Rig {
    s: Serial,
    counter: u16,
}

impl Rig {
    /// 系统计数器从 `counter` 开始（此刻之前的值视为相同，不产生边沿）。
    fn at(counter: u16) -> Self {
        let mut s = Serial::new();
        s.set_system_counter(counter);
        s.tick_mcycle();
        Rig { s, counter }
    }

    fn tick(&mut self) -> u8 {
        self.counter = self.counter.wrapping_add(4);
        self.s.set_system_counter(self.counter);
        self.s.tick_mcycle()
    }

    fn ticks(&mut self, n: u32) -> u8 {
        (0..n).fold(0, |acc, _| acc | self.tick())
    }

    /// 推进直到串口中断，返回经过的 M-cycle 数（上限 `limit`）。
    fn until_irq(&mut self, limit: u32) -> Option<u32> {
        (1..=limit).find(|_| self.tick() & irq::SERIAL != 0)
    }

    fn start(&mut self, sb: u8) {
        self.s.write_reg(SB, sb);
        self.s.write_reg(SC, 0x81);
    }
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
fn internal_clock_transfer_outputs_immediately_and_completes_after_8_clock_edges() {
    // 计数器从 0 开始：bit 8 在 512、1024、… 处下降，传输恰好 4096 T（1024 个 M-cycle）。
    let mut r = Rig::at(0);
    r.start(0x41);
    assert_eq!(r.s.take_output(), vec![0x41], "SB appended when the transfer starts");
    assert!(r.s.take_output().is_empty());
    assert_eq!(r.ticks(1023), 0, "not complete before the 8th edge");
    assert_eq!(r.s.read_reg(SC) & 0x80, 0x80);
    assert_eq!(r.tick(), irq::SERIAL, "complete on the 8th falling edge of bit 8");
    assert_eq!(r.s.read_reg(SC), 0x7F, "SC.7 cleared, clock select kept");
    assert_eq!(r.s.read_reg(SB), 0xFF, "disconnected link shifts in 1s");
    assert_eq!(r.ticks(5000), 0, "only one interrupt per transfer");
}

#[test]
fn transfer_length_follows_clock_phase() {
    // 开始时 bit 8 即将下降：第 1 位只等 1 个 M-cycle，总共 1 + 7×128 个 M-cycle。
    let mut r = Rig::at(0x01FC);
    r.start(0x00);
    assert_eq!(r.until_irq(2000), Some(1 + 7 * 128));
    // 开始时 bit 8 刚下降：等满 8 个周期。
    let mut r = Rig::at(0x0200);
    r.start(0x00);
    assert_eq!(r.until_irq(2000), Some(8 * 128));
    // 开始时 bit 8 为 1、离下降还有 64 个 M-cycle。
    let mut r = Rig::at(0x0100);
    r.start(0x00);
    assert_eq!(r.until_irq(2000), Some(64 + 7 * 128));
}

#[test]
fn sb_shifts_in_ones_on_each_clock_edge() {
    let mut r = Rig::at(0);
    r.start(0x00);
    for bit in 1..=8u32 {
        r.ticks(127);
        assert_eq!(r.s.read_reg(SB), ((1u16 << (bit - 1)) - 1) as u8, "just before edge {bit}");
        r.tick();
        assert_eq!(r.s.read_reg(SB), ((1u16 << bit) - 1) as u8, "after edge {bit}");
    }
}

#[test]
fn div_reset_with_clock_bit_set_counts_as_an_edge() {
    let mut r = Rig::at(0);
    r.start(0x00);
    r.ticks(100); // 计数器 400：bit 8 = 1
    assert_eq!(r.s.read_reg(SB), 0x00);
    r.counter = 0u16.wrapping_sub(4); // 下一次 tick 后为 0：等同写 DIV
    r.tick();
    assert_eq!(r.s.read_reg(SB), 0x01, "bit 8 went 1 -> 0");
}

#[test]
fn external_clock_transfer_never_completes() {
    let mut r = Rig::at(0);
    r.s.write_reg(SB, 0x55);
    r.s.write_reg(SC, 0x80);
    assert!(r.s.take_output().is_empty(), "external clock does not send");
    assert_eq!(r.ticks(100_000), 0);
    assert_eq!(r.s.read_reg(SC), 0xFE, "SC.7 stays set");
    assert_eq!(r.s.read_reg(SB), 0x55);
}

#[test]
fn each_start_appends_sb_even_without_waiting() {
    // 软件可以不等 SC.7 清零就写下一个字符：每次启动都记录当时的 SB，并重新数 8 个时钟。
    let mut r = Rig::at(0);
    for &c in b"Hi!\n" {
        r.start(c);
        r.ticks(10);
    }
    assert_eq!(r.s.take_output(), b"Hi!\n".to_vec());
    // 最后一次启动时计数器为 120（bit 8 = 0），之后的第 8 个下降沿在 4096。
    assert_eq!(r.until_irq(2000), Some(1024 - 40));
}

#[test]
fn clearing_sc7_aborts_transfer() {
    let mut r = Rig::at(0);
    r.start(0x12);
    r.ticks(100);
    r.s.write_reg(SC, 0x01);
    assert_eq!(r.ticks(5000), 0);
    assert_eq!(r.s.read_reg(SC), 0x7F);
}

#[test]
fn snapshot_roundtrip_mid_transfer() {
    let mut a = Rig::at(0);
    a.start(0x3C);
    a.ticks(300);
    let mut w = StateWriter::new();
    a.s.save(&mut w);
    let data = w.into_inner();
    let mut b = Rig { s: Serial::new(), counter: a.counter };
    let mut rd = StateReader::new(&data);
    b.s.load(&mut rd).unwrap();
    assert_eq!(rd.remaining(), 0);
    assert_eq!(b.s.read_reg(SB), a.s.read_reg(SB));
    assert_eq!(b.s.read_reg(SC), a.s.read_reg(SC));
    assert!(b.s.take_output().is_empty(), "output buffer is host-side, not saved");
    a.s.take_output();
    assert_eq!(a.s, b.s);
    assert_eq!(b.until_irq(2000), Some(1024 - 300));
    // 损坏的数据被拒绝。
    let mut bad = data.clone();
    bad[5] = 0x02; // SC 非法位
    assert!(Serial::new().load(&mut StateReader::new(&bad)).is_err());
    let mut bad = data.clone();
    bad[6] = 9; // 剩余位数超过 8
    assert!(Serial::new().load(&mut StateReader::new(&bad)).is_err());
}

fn bus() -> Bus {
    let mut rom = vec![0u8; 0x8000];
    rom[0x147] = 0x00;
    Bus::new(cart::from_rom(rom).unwrap())
}

/// 从当前时刻起推进，直到 IF 出现串口中断；返回经过的 M-cycle 数。
fn bus_until_serial_irq(b: &mut Bus, limit: u64) -> u64 {
    let start = b.mcycles();
    while b.peek(0xFF0F) & irq::SERIAL == 0 {
        b.idle();
        assert!(b.mcycles() - start <= limit, "serial transfer did not complete");
    }
    b.mcycles() - start
}

#[test]
fn bus_serial_clock_is_aligned_to_div() {
    let mut b = bus();
    b.write(0xFF0F, 0x00);
    b.write(0xFF04, 0x00); // 本 M-cycle 计数器清零
    b.write(SB, b'P'); // 计数器 4
    b.write(SC, 0x81); // 计数器 8
    assert_eq!(b.take_serial(), vec![b'P']);
    // 第 8 个下降沿在计数器 4096 处，即 DIV 清零后第 1024 个 M-cycle、写 SC 后第 1022 个。
    assert_eq!(bus_until_serial_irq(&mut b, 1100), 1022);
    assert_eq!(b.read(SC) & 0x80, 0);
    assert_eq!(b.read(SB), 0xFF);
    b.write(0xFFFF, irq::SERIAL);
    assert_eq!(b.pending(), irq::SERIAL);
}

#[test]
fn bus_serial_clock_phase_after_power_up() {
    // 开机计数器 0xABC8；在第 26 个 M-cycle 写 SC（计数器 0xAC30），bit 8 的下降沿在 0xAE00、0xB000……
    // 第 8 个在 0xAE00 + 7×512，即第 26 + 116 + 7×128 = 1038 个 M-cycle。
    let mut b = bus();
    for _ in 0..25 {
        b.idle();
    }
    b.write(SC, 0x81);
    assert_eq!(b.mcycles(), 26);
    assert_eq!(26 + bus_until_serial_irq(&mut b, 1100), 1038);
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
