//! T15：定时器（PLAN §3.4；Pan Docs Timer_and_Divider_Registers、Timer_Obscure_Behaviour）。
//!
//! 第一部分直接驱动 `Timer`（`Peripheral` 接口，每次 `tick_mcycle` = 4 T）；
//! 第二部分经 `Bus` 按指令的 M-cycle 序列访问寄存器，验证 D002（先推进外设再访问）下的相对时序。

use gb_core::bus::Bus;
use gb_core::cart;
use gb_core::irq;
use gb_core::timer::Timer;
use gb_core::{CpuBus, Peripheral, Snapshot, StateReader, StateWriter};

const DIV: u16 = 0xFF04;
const TIMA: u16 = 0xFF05;
const TMA: u16 = 0xFF06;
const TAC: u16 = 0xFF07;

/// 计数器清零、TIMA/TMA=0、再写入 `tac`（先关定时器，避免设置过程中的下降沿）。
fn timer_with(tac: u8) -> Timer {
    let mut t = Timer::new();
    t.write_reg(TAC, 0x00);
    t.write_reg(DIV, 0);
    t.write_reg(TIMA, 0);
    t.write_reg(TMA, 0);
    t.write_reg(TAC, tac);
    t
}

/// 推进 n 个 M-cycle，返回期间请求的中断位之或。
fn ticks(t: &mut Timer, n: u32) -> u8 {
    (0..n).fold(0, |acc, _| acc | t.tick_mcycle())
}

#[test]
fn power_up_div_and_phase() {
    let mut t = Timer::new();
    assert_eq!(t.read_reg(DIV), 0xAB);
    assert_eq!(t.read_reg(TIMA), 0x00);
    assert_eq!(t.read_reg(TMA), 0x00);
    assert_eq!(t.read_reg(TAC), 0xF8);
    // 开机后第 14 个 M-cycle DIV 恰好变为 0xAC。
    ticks(&mut t, 13);
    assert_eq!(t.read_reg(DIV), 0xAB);
    ticks(&mut t, 1);
    assert_eq!(t.read_reg(DIV), 0xAC);
}

#[test]
fn div_increments_every_256_t_and_write_resets() {
    let mut t = timer_with(0x00);
    for n in 1..=5u8 {
        ticks(&mut t, 63);
        assert_eq!(t.read_reg(DIV), n - 1);
        ticks(&mut t, 1);
        assert_eq!(t.read_reg(DIV), n);
    }
    t.write_reg(DIV, 0x77); // 写任何值都清零
    assert_eq!(t.read_reg(DIV), 0);
    assert_eq!(t.counter(), 0);
}

#[test]
fn div_wraps_without_affecting_disabled_tima() {
    let mut t = timer_with(0x03); // 关闭（bit2=0），选 bit7
    ticks(&mut t, 0x4000); // 65536 T：计数器回绕一圈
    assert_eq!(t.counter(), 0);
    assert_eq!(t.read_reg(TIMA), 0);
}

#[test]
fn tima_rates_for_each_tac() {
    // TAC 四档：00→1024 T、01→16 T、10→64 T、11→256 T（以 M-cycle 计为 256/4/16/64）。
    for (tac, period_t) in [(0x04u8, 1024u32), (0x05, 16), (0x06, 64), (0x07, 256)] {
        let period = period_t / 4;
        let mut t = timer_with(tac);
        ticks(&mut t, period - 1);
        assert_eq!(t.read_reg(TIMA), 0, "TAC {tac:02X}: before first period");
        ticks(&mut t, 1);
        assert_eq!(t.read_reg(TIMA), 1, "TAC {tac:02X}: after {period_t} T");
        ticks(&mut t, period * 9);
        assert_eq!(t.read_reg(TIMA), 10, "TAC {tac:02X}: after 10 periods");
        ticks(&mut t, period - 1);
        assert_eq!(t.read_reg(TIMA), 10, "TAC {tac:02X}: no early increment");
    }
}

#[test]
fn disabled_timer_does_not_count() {
    for tac in 0..4u8 {
        let mut t = timer_with(tac);
        assert_eq!(ticks(&mut t, 4096), 0);
        assert_eq!(t.read_reg(TIMA), 0, "TAC {tac:02X}");
    }
}

#[test]
fn div_write_triggers_extra_increment_when_selected_bit_set() {
    for (tac, bit) in [(0x04u8, 9u32), (0x05, 3), (0x06, 5), (0x07, 7)] {
        // 所选位为 1 时写 DIV：下降沿 → 额外加 1。
        let mut t = timer_with(tac);
        ticks(&mut t, (1 << bit) / 4); // 计数器 = 1<<bit，所选位刚变 1
        assert_eq!(t.read_reg(TIMA), 0);
        t.write_reg(DIV, 0);
        assert_eq!(t.read_reg(TIMA), 1, "TAC {tac:02X}: DIV write with bit {bit} set");
        // 所选位为 0 时写 DIV：不加。
        ticks(&mut t, 1);
        t.write_reg(DIV, 0);
        assert_eq!(t.read_reg(TIMA), 1, "TAC {tac:02X}: DIV write with bit {bit} clear");
    }
}

#[test]
fn div_write_with_timer_disabled_never_increments() {
    let mut t = timer_with(0x01); // bit3 选中但关闭
    ticks(&mut t, 2); // 计数器 = 8，bit3=1
    t.write_reg(DIV, 0);
    assert_eq!(t.read_reg(TIMA), 0);
}

#[test]
fn tac_writes_can_increment() {
    // Pan Docs 例子：系统计数器（M-cycle）= $3FF0、TAC=$FC 时，写 $05 或 $06 立即加 1，写 $04 或 $07 不加。
    // 本实现的计数器以 T 计，即 $3FF0 << 2。
    for (new_tac, expect) in [(0x05u8, 1u8), (0x06, 1), (0x04, 0), (0x07, 0)] {
        let mut t = timer_with(0x00);
        ticks(&mut t, 0x3FF0);
        assert_eq!(t.counter(), 0xFFC0);
        t.write_reg(TAC, 0xFC);
        assert_eq!(t.read_reg(TIMA), 0);
        t.write_reg(TAC, new_tac);
        assert_eq!(t.read_reg(TIMA), expect, "TAC $FC -> ${new_tac:02X}");
    }
    // DMG：所选位为 1 时关闭定时器也会加 1。
    let mut t = timer_with(0x05);
    ticks(&mut t, 2); // bit3 = 1
    t.write_reg(TAC, 0x01);
    assert_eq!(t.read_reg(TIMA), 1);
    // 所选位为 0 时关闭不加；从 0 位切到 1 位也不加。
    let mut t = timer_with(0x05);
    ticks(&mut t, 1); // 计数器 = 4：bit3 = 0
    t.write_reg(TAC, 0x01);
    t.write_reg(TAC, 0x05);
    assert_eq!(t.read_reg(TIMA), 0);
}

#[test]
fn tac_reads_upper_bits_as_one() {
    let mut t = Timer::new();
    t.write_reg(TAC, 0x00);
    assert_eq!(t.read_reg(TAC), 0xF8);
    t.write_reg(TAC, 0xFF);
    assert_eq!(t.read_reg(TAC), 0xFF);
    t.write_reg(TAC, 0x05);
    assert_eq!(t.read_reg(TAC), 0xFD);
}

/// TMA=0x23、TIMA=0xFE、TAC=0x05：TIMA 每 4 个 M-cycle 加 1；返回已推进到"周期 A 之前一刻"的定时器。
/// 第 8 个 M-cycle 溢出（周期 A），第 9 个装入 TMA 并请求中断（周期 B）。
fn about_to_overflow() -> Timer {
    let mut t = timer_with(0x05);
    t.write_reg(TMA, 0x23);
    t.write_reg(TIMA, 0xFE);
    assert_eq!(ticks(&mut t, 4), 0);
    assert_eq!(t.read_reg(TIMA), 0xFF);
    assert_eq!(ticks(&mut t, 3), 0);
    assert_eq!(t.read_reg(TIMA), 0xFF);
    t
}

#[test]
fn overflow_reads_zero_for_one_mcycle_then_reloads_with_irq() {
    let mut t = about_to_overflow();
    assert_eq!(t.tick_mcycle(), 0, "cycle A: no interrupt yet");
    assert_eq!(t.read_reg(TIMA), 0x00, "cycle A: TIMA reads 0");
    assert_eq!(t.tick_mcycle(), irq::TIMER, "cycle B: interrupt requested");
    assert_eq!(t.read_reg(TIMA), 0x23, "cycle B: TMA loaded");
    assert_eq!(ticks(&mut t, 2), 0);
    assert_eq!(t.read_reg(TIMA), 0x23);
    assert_eq!(ticks(&mut t, 1), 0);
    assert_eq!(t.read_reg(TIMA), 0x24);
}

#[test]
fn tima_write_in_cycle_a_cancels_reload_and_irq() {
    let mut t = about_to_overflow();
    t.tick_mcycle(); // 周期 A
    t.write_reg(TIMA, 0x55);
    assert_eq!(t.tick_mcycle(), 0, "no interrupt after cancelled overflow");
    assert_eq!(t.read_reg(TIMA), 0x55);
    assert_eq!(ticks(&mut t, 16), 0);
}

#[test]
fn tima_write_in_cycle_b_is_ignored() {
    let mut t = about_to_overflow();
    t.tick_mcycle(); // 周期 A
    assert_eq!(t.tick_mcycle(), irq::TIMER); // 周期 B
    t.write_reg(TIMA, 0x55);
    assert_eq!(t.read_reg(TIMA), 0x23);
    // 下一个周期起写 TIMA 恢复正常。
    t.tick_mcycle();
    t.write_reg(TIMA, 0x56);
    assert_eq!(t.read_reg(TIMA), 0x56);
}

#[test]
fn tma_write_in_cycle_b_is_copied_to_tima() {
    let mut t = about_to_overflow();
    t.tick_mcycle(); // 周期 A
    t.tick_mcycle(); // 周期 B
    t.write_reg(TMA, 0x7F);
    assert_eq!(t.read_reg(TIMA), 0x7F);
    assert_eq!(t.read_reg(TMA), 0x7F);
    // 周期 B 之后写 TMA 不再影响 TIMA。
    t.tick_mcycle();
    t.write_reg(TMA, 0x10);
    assert_eq!(t.read_reg(TIMA), 0x7F);
}

#[test]
fn tma_write_in_cycle_a_is_used_for_reload() {
    let mut t = about_to_overflow();
    t.tick_mcycle(); // 周期 A
    t.write_reg(TMA, 0x42);
    assert_eq!(t.tick_mcycle(), irq::TIMER);
    assert_eq!(t.read_reg(TIMA), 0x42);
}

#[test]
fn div_and_tac_writes_in_cycle_a_do_not_prevent_reload() {
    let mut t = about_to_overflow();
    t.tick_mcycle(); // 周期 A
    t.write_reg(DIV, 0);
    t.write_reg(TAC, 0x05);
    assert_eq!(t.tick_mcycle(), irq::TIMER);
    assert_eq!(t.read_reg(TIMA), 0x23);
}

#[test]
fn tma_ff_interrupts_every_increment() {
    let mut t = timer_with(0x05);
    t.write_reg(TMA, 0xFF);
    t.write_reg(TIMA, 0xFF);
    let mut irqs = 0;
    // 第 4k 个 M-cycle 溢出，第 4k+1 个请求中断：401 个 M-cycle 内恰好 100 次（每 16 T 一次）。
    for _ in 0..401 {
        if t.tick_mcycle() & irq::TIMER != 0 {
            irqs += 1;
        }
    }
    assert_eq!(irqs, 100);
}

#[test]
fn overflow_from_div_write_reloads_next_mcycle() {
    let mut t = timer_with(0x05);
    t.write_reg(TMA, 0x80);
    t.write_reg(TIMA, 0xFF);
    ticks(&mut t, 2); // bit3 = 1
    t.write_reg(DIV, 0); // 下降沿 → 溢出
    assert_eq!(t.read_reg(TIMA), 0x00);
    assert_eq!(t.tick_mcycle(), irq::TIMER);
    assert_eq!(t.read_reg(TIMA), 0x80);
}

#[test]
fn snapshot_roundtrip_mid_reload() {
    let mut a = about_to_overflow();
    a.tick_mcycle(); // 停在周期 A
    let mut w = StateWriter::new();
    a.save(&mut w);
    let data = w.into_inner();
    let mut b = Timer::new();
    let mut r = StateReader::new(&data);
    b.load(&mut r).unwrap();
    assert_eq!(r.remaining(), 0);
    assert_eq!(a, b);
    // 恢复后的行为与原件一致。
    assert_eq!(a.tick_mcycle(), b.tick_mcycle());
    assert_eq!(b.read_reg(TIMA), 0x23);
    // 损坏的数据被拒绝。
    let mut bad = data.clone();
    *bad.last_mut().unwrap() = 9;
    assert!(Timer::new().load(&mut StateReader::new(&bad)).is_err());
}

// ---- 经 Bus 的时序（D002）：每条指令按 M-cycle 序列调用 read/write/idle ----

fn bus() -> Bus {
    let mut rom = vec![0u8; 0x8000];
    rom[0x147] = 0x00;
    Bus::new(cart::from_rom(rom).unwrap())
}

/// n 个 NOP（每个 1 次取指）。
fn nops(b: &mut Bus, n: u32) {
    for _ in 0..n {
        b.read(0x0000);
    }
}

/// `LDH (a8),A`：取指、取操作数、写。
fn ldh_write(b: &mut Bus, addr: u16, v: u8) {
    b.read(0x0000);
    b.read(0x0001);
    b.write(addr, v);
}

/// `LDH A,(a8)`：取指、取操作数、读。
fn ldh_read(b: &mut Bus, addr: u16) -> u8 {
    b.read(0x0000);
    b.read(0x0001);
    b.read(addr)
}

/// `PUSH AF`：取指、内部周期、写高字节、写低字节。
fn push(b: &mut Bus) {
    b.read(0x0000);
    b.idle();
    b.write(0xDFFF, 0);
    b.write(0xDFFE, 0);
}

#[test]
fn bus_boot_div_phase() {
    // PC=0x0100：NOP（1）+ JP a16（4），再 6 个 NOP，然后 LDH A,(DIV)：读发生在 DIV 刚加 1 之后。
    let mut b = bus();
    nops(&mut b, 5);
    nops(&mut b, 6);
    let mut seen = vec![ldh_read(&mut b, DIV)];
    // 每段 PUSH AF + n 个 NOP + LDH：57 个 NOP 恰好 64 个 M-cycle，相位不变；56/58 前后错开一个 M-cycle。
    for n in [57, 56, 57, 57, 58] {
        push(&mut b);
        nops(&mut b, n);
        seen.push(ldh_read(&mut b, DIV));
    }
    assert_eq!(seen, vec![0xAC, 0xAD, 0xAD, 0xAE, 0xAF, 0xB1]);
}

/// 设置 TAC=%110（每 64 T）、TIMA=TMA=0xFE，以 DIV 写入对齐相位（写 DIV 后 TIMA 在第 16、32 个 M-cycle 加 1）。
fn bus_aligned_timer(b: &mut Bus) {
    ldh_write(b, 0xFFFF, 0);
    ldh_write(b, 0xFF0F, 0);
    ldh_write(b, DIV, 0);
    ldh_write(b, TIMA, 0xFE);
    ldh_write(b, TMA, 0xFE);
    ldh_write(b, TAC, 0x06);
    ldh_write(b, DIV, 0);
    ldh_write(b, TIMA, 0xFE);
    ldh_write(b, DIV, 0);
}

#[test]
fn bus_tima_reload_window() {
    // 写 DIV 之后 n 个 NOP 再读 TIMA：读在第 n+3 个 M-cycle。
    for (n, expect) in [(28u32, 0xFFu8), (29, 0x00), (30, 0xFE), (60, 0xFF), (61, 0x00), (62, 0xFE)] {
        let mut b = bus();
        bus_aligned_timer(&mut b);
        nops(&mut b, n);
        assert_eq!(ldh_read(&mut b, TIMA), expect, "{n} NOPs");
    }
}

#[test]
fn bus_tima_write_while_reloading() {
    // LD A,H（1）+ n 个 NOP，然后 LDH (TIMA),A 写 0x7F，再读 TIMA。
    for (n, expect) in [(27u32, 0x80u8), (28, 0x7F), (29, 0xFE), (30, 0x7F)] {
        let mut b = bus();
        bus_aligned_timer(&mut b);
        nops(&mut b, 1 + n);
        ldh_write(&mut b, TIMA, 0x7F);
        assert_eq!(ldh_read(&mut b, TIMA), expect, "{n} NOPs");
    }
}

#[test]
fn bus_tma_write_while_reloading() {
    for (n, expect) in [(28u32, 0x7Fu8), (29, 0x7F), (30, 0xFE), (31, 0xFE)] {
        let mut b = bus();
        bus_aligned_timer(&mut b);
        nops(&mut b, 1 + n);
        ldh_write(&mut b, TMA, 0x7F);
        assert_eq!(ldh_read(&mut b, TIMA), expect, "{n} NOPs");
    }
}

#[test]
fn bus_timer_interrupt_reaches_if_one_cycle_after_overflow() {
    let mut b = bus();
    bus_aligned_timer(&mut b);
    // 第 32 个 M-cycle 溢出（周期 A），第 33 个置 IF bit2（周期 B）。
    nops(&mut b, 31);
    assert_eq!(b.peek(0xFF0F) & 0x04, 0);
    nops(&mut b, 1);
    assert_eq!(b.peek(TIMA), 0x00);
    assert_eq!(b.peek(0xFF0F) & 0x04, 0, "cycle A: IF not yet set");
    nops(&mut b, 1);
    assert_eq!(b.peek(TIMA), 0xFE);
    assert_eq!(b.peek(0xFF0F) & 0x04, 0x04, "cycle B: IF set");
    b.write(0xFFFF, 0x04);
    assert_eq!(b.pending(), 0x04);
}

#[test]
fn bus_stop_resets_div() {
    let mut b = bus();
    nops(&mut b, 100);
    assert_ne!(b.peek(DIV), 0);
    b.stop();
    assert_eq!(b.peek(DIV), 0);
}
