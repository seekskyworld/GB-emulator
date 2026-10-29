//! T40：PPU 时序骨架（dot 计数、LY、mode、STAT 上升沿、VBlank、LCD 开关、双缓冲、存档）。
//! 只用公开接口：`Ppu` 与 `Peripheral`/`Snapshot`；1 次 `tick_mcycle` = 4 dot。

use gb_core::ppu::Ppu;
use gb_core::{irq, Peripheral, Snapshot, StateReader, StateWriter, FRAME_T};

const LCDC: u16 = 0xFF40;
const STAT: u16 = 0xFF41;
const SCY: u16 = 0xFF42;
const LY: u16 = 0xFF44;
const LYC: u16 = 0xFF45;
const BGP: u16 = 0xFF47;

/// 一帧的 M-cycle 数。
const FRAME_M: usize = FRAME_T as usize / 4;
/// 一行的 M-cycle 数。
const LINE_M: usize = 456 / 4;

/// 关再开 LCD：PPU 停在 LY=0 的行首（dot 0）。
fn restart(p: &mut Ppu) {
    let l = p.read_reg(LCDC);
    p.write_reg(LCDC, l & 0x7F);
    p.write_reg(LCDC, l | 0x80);
}

fn mode(p: &Ppu) -> u8 {
    p.read_reg(STAT) & 3
}

/// 推进 `n` 个 M-cycle，返回中断位的或。
fn run(p: &mut Ppu, n: usize) -> u8 {
    let mut req = 0;
    for _ in 0..n {
        req |= p.tick_mcycle();
    }
    req
}

/// 推进到下一个 VBlank 中断（含），返回用掉的 M-cycle 数。
fn run_to_vblank(p: &mut Ppu) -> usize {
    for n in 1..=2 * FRAME_M {
        if p.tick_mcycle() & irq::VBLANK != 0 {
            return n;
        }
    }
    panic!("no VBlank within two frames");
}

/// 在 dot = 4k（LCD 刚开启时 k=0）的位置，按 §3.5 的模型应读到的 (LY, mode)。
fn expected(k: usize) -> (u8, u8) {
    let dots = 4 * k;
    let line = (dots / 456) % 154;
    let dot = dots % 456;
    let first_line_after_on = dots < 456;
    let m = if line >= 144 {
        1
    } else if dot < 80 {
        if first_line_after_on {
            0
        } else {
            2
        }
    } else if dot < 252 {
        3
    } else {
        0
    };
    (line as u8, m)
}

#[test]
fn power_up_registers_follow_dmg_column() {
    let p = Ppu::new();
    assert_eq!(p.read_reg(LCDC), 0x91);
    assert_eq!(p.read_reg(STAT), 0x85);
    assert_eq!(p.read_reg(SCY), 0x00);
    assert_eq!(p.read_reg(0xFF43), 0x00);
    assert_eq!(p.read_reg(LY), 0x00);
    assert_eq!(p.read_reg(LYC), 0x00);
    assert_eq!(p.read_reg(BGP), 0xFC);
    assert_eq!(p.read_reg(0xFF4A), 0x00);
    assert_eq!(p.read_reg(0xFF4B), 0x00);
    assert!(p.frame().iter().all(|&v| v == 0xFF));
}

#[test]
fn power_up_state_reaches_line_0_within_one_line() {
    // 开机时处于最后一个 VBlank 行（mode 1、LY 读作 0）；不到一行之后进入第 0 行 mode 2。
    let mut p = Ppu::new();
    let mut n = 0;
    while mode(&p) != 2 {
        assert_eq!(mode(&p), 1);
        assert_eq!(p.read_reg(LY), 0);
        assert_eq!(p.tick_mcycle(), 0);
        n += 1;
        assert!(n < LINE_M, "still in VBlank after a full line");
    }
    assert_eq!(p.read_reg(LY), 0);
    // 之后第一次 VBlank 正好在 144 行之后。
    assert_eq!(run_to_vblank(&mut p), 144 * LINE_M);
}

#[test]
fn frame_is_70224_t_and_exactly_one_vblank_per_frame() {
    let mut p = Ppu::new();
    run_to_vblank(&mut p);
    for _ in 0..5 {
        assert_eq!(run_to_vblank(&mut p) * 4, FRAME_T as usize);
    }
    // 连续 10 帧的 tick 里恰好 10 次 VBlank。
    let mut count = 0;
    for _ in 0..10 * FRAME_M {
        if p.tick_mcycle() & irq::VBLANK != 0 {
            count += 1;
        }
    }
    assert_eq!(count, 10);
}

#[test]
fn ly_and_mode_follow_the_line_schedule() {
    let mut p = Ppu::new();
    restart(&mut p);
    assert_eq!((p.read_reg(LY), mode(&p)), expected(0));
    let mut seen = [false; 154];
    for k in 1..=3 * FRAME_M {
        p.tick_mcycle();
        let got = (p.read_reg(LY), mode(&p));
        assert_eq!(got, expected(k), "at M-cycle {k}");
        seen[got.0 as usize] = true;
        // STAT bit7 恒为 1；bit2 跟随 LY==LYC（LYC=0）。
        let stat = p.read_reg(STAT);
        assert_eq!(stat & 0x80, 0x80);
        assert_eq!(stat & 0x04 != 0, got.0 == 0);
    }
    assert!(
        seen.iter().all(|&s| s),
        "every LY value 0..=153 must appear"
    );
}

#[test]
fn mode_lengths_per_visible_line() {
    let mut p = Ppu::new();
    restart(&mut p);
    run(&mut p, LINE_M); // 跳过开启后的第 0 行
    for _ in 0..143 {
        let mut counts = [0usize; 4];
        for _ in 0..LINE_M {
            counts[mode(&p) as usize] += 1;
            p.tick_mcycle();
        }
        assert_eq!(counts, [204 / 4, 0, 80 / 4, 172 / 4]);
    }
}

#[test]
fn ly_is_read_only() {
    let mut p = Ppu::new();
    restart(&mut p);
    run(&mut p, 10 * LINE_M + 3);
    assert_eq!(p.read_reg(LY), 10);
    p.write_reg(LY, 0x42);
    assert_eq!(p.read_reg(LY), 10);
    p.write_reg(LY, 0x00);
    assert_eq!(p.read_reg(LY), 10);
}

#[test]
fn stat_write_only_changes_select_bits() {
    let mut p = Ppu::new();
    p.write_reg(STAT, 0xFF);
    assert_eq!(p.read_reg(STAT), 0xFD); // 0x80 | 0x78 | LYC=LY | mode 1
    p.write_reg(STAT, 0x00);
    assert_eq!(p.read_reg(STAT), 0x85);
}

/// 按 STAT 中断设置跑 `frames` 帧，返回每次 STAT 中断时的 (LY, STAT)。
fn stat_irqs(stat: u8, lyc: u8, frames: usize) -> Vec<(u8, u8)> {
    let mut p = Ppu::new();
    p.write_reg(LYC, lyc);
    restart(&mut p);
    p.write_reg(STAT, stat);
    p.take_irq();
    run_to_vblank(&mut p);
    let mut out = Vec::new();
    for _ in 0..frames * FRAME_M {
        if p.tick_mcycle() & irq::STAT != 0 {
            out.push((p.read_reg(LY), p.read_reg(STAT)));
        }
    }
    out
}

#[test]
fn lyc_interrupt_fires_once_per_frame_on_the_matching_line() {
    for lyc in [0u8, 1, 8, 0x40, 0x70, 143, 144, 150, 153] {
        let irqs = stat_irqs(0x40, lyc, 3);
        assert_eq!(irqs.len(), 3, "LYC={lyc}: {irqs:?}");
        for &(ly, stat) in &irqs {
            assert_eq!(ly, lyc, "LYC={lyc}");
            assert_eq!(stat & 0x04, 0x04, "LYC={lyc}: STAT.2 must be set");
        }
    }
    // LYC 超出 0..=153 时永不触发。
    assert!(stat_irqs(0x40, 200, 2).is_empty());
}

#[test]
fn lyc_interrupt_comes_at_line_start() {
    let mut p = Ppu::new();
    p.write_reg(LYC, 0x30);
    restart(&mut p);
    p.write_reg(STAT, 0x40);
    let mut k = 0;
    loop {
        k += 1;
        if p.tick_mcycle() & irq::STAT != 0 {
            break;
        }
    }
    assert_eq!(k, 0x30 * LINE_M, "must fire on the M-cycle LY becomes LYC");
    assert_eq!(mode(&p), 2);
}

#[test]
fn stat_interrupt_is_rising_edge_of_the_ored_line() {
    // mode 0：每个可见行一次。
    let v = stat_irqs(0x08, 200, 2);
    assert_eq!(v.len(), 2 * 144);
    assert!(v.iter().all(|&(ly, s)| ly < 144 && s & 3 == 0));
    // mode 1：每帧一次，在 LY=144。
    let v = stat_irqs(0x10, 200, 2);
    assert_eq!(v, vec![(144, 0x91), (144, 0x91)]);
    // mode 2：每个可见行一次，外加 LY=144 的那一次（DMG）。
    let v = stat_irqs(0x20, 200, 2);
    assert_eq!(v.len(), 2 * 145);
    assert_eq!(v.iter().filter(|&&(ly, _)| ly == 144).count(), 2);
    // mode 0 + mode 2：mode 0 → 下一行 mode 2 时中断线一直为高，不再触发；
    // 只有每行进入 mode 0（144 次）与 VBlank 之后的第 0 行 mode 2（1 次）。
    let v = stat_irqs(0x28, 200, 2);
    assert_eq!(v.len(), 2 * 145);
    assert_eq!(v.iter().filter(|&&(_, s)| s & 3 == 2).count(), 2);
    // mode 1 + mode 2：LY=144 一次，第 0 行（从 mode 1 直接到 mode 2）不触发，第 1–143 行各一次。
    let v = stat_irqs(0x30, 200, 2);
    assert_eq!(v.len(), 2 * 144);
    assert!(!v.iter().any(|&(ly, _)| ly == 0));
    // 全部打开且 LYC 永不相等：只在 mode 3 拉低，每个可见行进入 mode 0 时一次。
    let v = stat_irqs(0x78, 200, 2);
    assert_eq!(v.len(), 2 * 144);
    // 全部打开且 LYC=LY 跨过 mode 3：那一行的 mode 0 不再触发。
    let v = stat_irqs(0x78, 0x50, 2);
    assert_eq!(v.len(), 2 * 143);
    assert!(!v.iter().any(|&(ly, _)| ly == 0x50));
}

#[test]
fn vblank_and_line_144_mode2_stat_fire_together() {
    let mut p = Ppu::new();
    restart(&mut p);
    p.write_reg(STAT, 0x20);
    run_to_vblank(&mut p);
    for _ in 0..3 {
        // 帧内最后一次可见行的 mode 2 中断之后，下一次 STAT 中断与 VBlank 在同一个 M-cycle。
        loop {
            let r = p.tick_mcycle();
            if r & irq::VBLANK != 0 {
                assert_eq!(r & irq::STAT, irq::STAT);
                assert_eq!(p.read_reg(LY), 144);
                break;
            }
        }
    }
}

#[test]
fn stat_write_that_raises_the_line_requests_an_interrupt() {
    let mut p = Ppu::new();
    restart(&mut p);
    run_to_vblank(&mut p);
    run(&mut p, 10);
    assert_eq!(mode(&p), 1);
    p.write_reg(STAT, 0x10); // 已在 mode 1：中断线 0→1
    assert_eq!(p.take_irq(), irq::STAT);
    assert_eq!(p.take_irq(), 0);
    p.write_reg(STAT, 0x10); // 线已经为高：不再触发
    assert_eq!(p.tick_mcycle() & irq::STAT, 0);
    // LYC 写成当前 LY：0→1，中断位留到下一次 tick 交出。
    p.write_reg(STAT, 0x40);
    p.write_reg(LYC, p.read_reg(LY));
    assert_eq!(p.tick_mcycle() & irq::STAT, irq::STAT);
}

#[test]
fn lcd_off_freezes_the_ppu() {
    let mut p = Ppu::new();
    restart(&mut p);
    run(&mut p, 20 * LINE_M + 30);
    assert_eq!(p.read_reg(LY), 20);
    p.write_reg(LCDC, 0x11);
    assert_eq!(p.read_reg(LY), 0);
    assert_eq!(mode(&p), 0);
    assert!(p.frame().iter().all(|&v| v == 0xFF));
    // 关闭期间 tick 不推进、不产生中断。
    p.write_reg(STAT, 0x78);
    assert_eq!(run(&mut p, 2 * FRAME_M), 0);
    assert_eq!(p.read_reg(LY), 0);
    assert_eq!(mode(&p), 0);
    // 重新开启：从 LY=0 的行首开始。
    p.write_reg(STAT, 0x00);
    p.write_reg(LCDC, 0x91);
    assert_eq!(run(&mut p, LINE_M - 1), 0);
    assert_eq!(p.read_reg(LY), 0);
    p.tick_mcycle();
    assert_eq!(p.read_reg(LY), 1);
    assert_eq!(run_to_vblank(&mut p), 143 * LINE_M);
}

#[test]
fn lcd_on_line_0_reports_mode_0_then_mode_3() {
    let mut p = Ppu::new();
    restart(&mut p);
    for k in 0..LINE_M {
        let want = if k < 20 {
            0
        } else if k < 63 {
            3
        } else {
            0
        };
        assert_eq!(mode(&p), want, "M-cycle {k}");
        p.tick_mcycle();
    }
    assert_eq!(mode(&p), 2);
    assert_eq!(p.read_reg(LY), 1);
}

#[test]
fn lyc_flag_is_kept_while_lcd_is_off() {
    // 对应 Mooneye stat_lyc_onoff 的四轮。
    let mut p = Ppu::new();
    p.write_reg(STAT, 0x40);
    p.write_reg(LYC, 0x90);
    restart(&mut p);
    run_to_vblank(&mut p);
    p.take_irq();
    // 第 1 轮：关闭时标志为真，保持；改 LYC 无效；开启后 LY=0≠1，标志清零，不触发。
    p.write_reg(LCDC, 0x11);
    assert_eq!(p.read_reg(STAT), 0xC4);
    p.write_reg(LYC, 0x01);
    assert_eq!(p.read_reg(STAT), 0xC4);
    p.write_reg(LCDC, 0x91);
    assert_eq!(p.read_reg(STAT), 0xC0);
    assert_eq!(p.take_irq(), 0);
    // 第 2 轮：关闭时为真，开启时 LY=0=LYC 仍为真：中断线不变，不触发。
    p.write_reg(LYC, 0x90);
    run_to_vblank(&mut p);
    p.take_irq();
    p.tick_mcycle();
    p.write_reg(LCDC, 0x11);
    p.write_reg(LYC, 0x00);
    assert_eq!(p.read_reg(STAT), 0xC4);
    p.write_reg(LCDC, 0x91);
    assert_eq!(p.read_reg(STAT), 0xC4);
    assert_eq!(p.take_irq(), 0);
    // 第 3 轮：关闭时为假，开启时比较 0 与 1，仍为假。
    run_to_vblank(&mut p);
    p.tick_mcycle();
    p.take_irq();
    p.write_reg(LYC, 0x00);
    p.write_reg(LCDC, 0x11);
    p.write_reg(LYC, 0x01);
    assert_eq!(p.read_reg(STAT), 0xC0);
    p.write_reg(LCDC, 0x91);
    assert_eq!(p.read_reg(STAT), 0xC0);
    assert_eq!(p.take_irq(), 0);
    // 第 4 轮：关闭时为假，LYC=0，开启后变真：触发 STAT 中断。
    run_to_vblank(&mut p);
    p.tick_mcycle();
    p.take_irq();
    p.write_reg(LYC, 0x00);
    p.write_reg(LCDC, 0x11);
    assert_eq!(p.read_reg(STAT), 0xC0);
    p.write_reg(LCDC, 0x91);
    assert_eq!(p.read_reg(STAT), 0xC4);
    assert_eq!(p.tick_mcycle() & irq::STAT, irq::STAT);
}

/// 两个 PPU 的全部可观察寄存器。
fn regs(p: &Ppu) -> Vec<u8> {
    (0xFF40..=0xFF4B)
        .filter(|&a| a != 0xFF46)
        .map(|a| p.read_reg(a))
        .collect()
}

fn save(p: &Ppu) -> Vec<u8> {
    let mut w = StateWriter::new();
    p.save(&mut w);
    w.into_inner()
}

#[test]
fn snapshot_roundtrip_mid_line_continues_identically() {
    let mut a = Ppu::new();
    a.write_reg(STAT, 0x68);
    a.write_reg(LYC, 0x21);
    restart(&mut a);
    run(&mut a, 3 * FRAME_M + 77 * LINE_M + 33); // 某帧第 77 行的 mode 3 中
    let data = save(&a);
    let mut b = Ppu::new();
    b.load(&mut StateReader::new(&data)).unwrap();
    assert_eq!(save(&b), data);
    assert_eq!(regs(&a), regs(&b));
    for k in 0..2 * FRAME_M {
        assert_eq!(a.tick_mcycle(), b.tick_mcycle(), "M-cycle {k}");
        assert_eq!(regs(&a), regs(&b));
    }
    assert_eq!(save(&a), save(&b));
}

#[test]
fn snapshot_rejects_bad_data_and_keeps_state() {
    let a = Ppu::new();
    let data = save(&a);
    let mut b = Ppu::new();
    restart(&mut b);
    run(&mut b, 1000);
    let before = save(&b);
    // 截断
    assert!(b
        .load(&mut StateReader::new(&data[..data.len() - 1]))
        .is_err());
    assert_eq!(save(&b), before);
    // 标签错位
    let mut bad = data.clone();
    bad[0] ^= 0xFF;
    assert!(b.load(&mut StateReader::new(&bad)).is_err());
    assert_eq!(save(&b), before);
}
