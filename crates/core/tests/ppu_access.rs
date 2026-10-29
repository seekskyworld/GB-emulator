//! W4：PPU 精确时序——mode 3 可变长度、VRAM/OAM 访问封锁、行首 LYC 位、调试读不受封锁。
//! 期望值按 Pan Docs `Rendering.md` "Mode 3 length" 的算法与 `Accessing_VRAM_and_OAM.md`，
//! 并与 Mooneye acceptance/ppu 测试源码里记录的硬件结果对齐（见 `ppu.rs` 顶部的时序表）。

use gb_core::ppu::Ppu;
use gb_core::{irq, Peripheral, FRAME_T};

const LCDC: u16 = 0xFF40;
const STAT: u16 = 0xFF41;
const SCX: u16 = 0xFF43;
const LY: u16 = 0xFF44;
const LYC: u16 = 0xFF45;
const WY: u16 = 0xFF4A;
const WX: u16 = 0xFF4B;

/// 推进到 LY 变为 `ly` 的那个 M-cycle（该行 dot 0）。
fn run_to_line(p: &mut Ppu, ly: u8) {
    for _ in 0..2 * FRAME_T / 4 {
        p.tick_mcycle();
        if p.read_reg(LY) == ly {
            return;
        }
    }
    panic!("LY never reached {ly}");
}

/// LCD 关闭时配置好（`setup`），再开启；返回第 0x42 行里 (STAT 首次读到 mode 0 的 dot, mode 0 STAT 中断的 dot)。
fn mode0_timing(setup: impl FnOnce(&mut Ppu)) -> (usize, usize) {
    let mut p = Ppu::new();
    p.write_reg(LCDC, 0x00);
    for a in 0xFE00..0xFEA0 {
        p.write_oam(a, 0);
    }
    setup(&mut p);
    p.write_reg(LCDC, p.read_reg(LCDC) | 0x80);
    p.write_reg(STAT, 0x08);
    run_to_line(&mut p, 0x42);
    p.take_irq();
    let (mut vis, mut int) = (None, None);
    for k in 1..=114 {
        let r = p.tick_mcycle();
        if int.is_none() && r & irq::STAT != 0 {
            int = Some(4 * k);
        }
        if vis.is_none() && p.read_reg(STAT) & 3 == 0 {
            vis = Some(4 * k);
        }
    }
    (vis.unwrap(), int.unwrap())
}

/// 在第 0x42 行放精灵（OAM Y = 0x42 + 16），X 由参数给出。
fn objs(xs: &[u8]) -> impl FnOnce(&mut Ppu) + '_ {
    move |p: &mut Ppu| {
        p.write_reg(LCDC, 0x93 & 0x7F);
        for (i, &x) in xs.iter().enumerate() {
            let a = 0xFE00 + 4 * i as u16;
            p.write_oam(a, 0x52);
            p.write_oam(a + 1, x);
        }
    }
}

/// 期望：STAT 在 ceil4(253+p) 读到 mode 0，mode 0 中断在 ceil4(256+p)。
fn want(p: usize) -> (usize, usize) {
    let c4 = |v: usize| v.div_ceil(4) * 4;
    (c4(253 + p), c4(256 + p))
}

#[test]
fn mode3_without_penalties_ends_at_256() {
    assert_eq!(mode0_timing(|p| p.write_reg(LCDC, 0x11)), want(0));
    assert_eq!(want(0), (256, 256));
}

#[test]
fn scx_fine_scroll_lengthens_mode3() {
    for scx in 0..16u8 {
        let got = mode0_timing(|p| {
            p.write_reg(LCDC, 0x11);
            p.write_reg(SCX, scx);
        });
        assert_eq!(got, want((scx % 8) as usize), "SCX={scx}");
    }
    // 只看中断：SCX 0 → 256，1–4 → 260，5–7 → 264。
    assert_eq!(want(1).1, 260);
    assert_eq!(want(4).1, 260);
    assert_eq!(want(5).1, 264);
}

#[test]
fn object_penalties_follow_the_pan_docs_algorithm() {
    // (精灵 X 列表, 额外 dot)
    let cases: &[(&[u8], usize)] = &[
        (&[0], 11),                                     // X=0 固定 11
        (&[1], 10),                                     // 6 个像素在右边：4 + 6
        (&[4], 7),                                      // 3 个：1 + 6
        (&[7], 6),                                      // 0 个：0 + 6
        (&[8], 11),                                     // 屏幕 x=0，所在 tile 7 个在右边
        (&[167], 6),                                    // 最右一列
        (&[168], 0),                                    // X≥168 不画，不计
        (&[0, 0], 17),                                  // 同一 tile 只计一次：11 + 6
        (&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 65),          // 11 + 9×6
        (&[0, 8], 22),                                  // 两个 tile
        (&[2, 10], 18),                                 // 9 + 9
        (&[72, 64, 56, 48, 40, 32, 24, 16, 8, 0], 110), // 顺序无关
    ];
    for &(xs, extra) in cases {
        assert_eq!(mode0_timing(objs(xs)), want(extra), "objects at {xs:?}");
    }
}

#[test]
fn object_penalties_need_lcdc1_and_track_scx_alignment() {
    // LCDC.1=0：精灵不画，也不延长 mode 3。
    let got = mode0_timing(|p| {
        objs(&[0, 8, 16])(p);
        p.write_reg(LCDC, 0x11);
    });
    assert_eq!(got, want(0));
    // SCX=3：BG tile 边界移到屏幕 x=5，X=8（屏幕 x=0）所在 tile 右边还有 4 个像素：3 + (4−2) + 6。
    let got = mode0_timing(|p| {
        objs(&[8])(p);
        p.write_reg(SCX, 3);
    });
    assert_eq!(got, want(3 + 2 + 6));
}

#[test]
fn window_adds_six_dots() {
    let got = mode0_timing(|p| {
        p.write_reg(LCDC, 0x31);
        p.write_reg(WY, 0);
        p.write_reg(WX, 7);
    });
    assert_eq!(got, want(6));
    // WX=167：窗口不出现，不加。
    let got = mode0_timing(|p| {
        p.write_reg(LCDC, 0x31);
        p.write_reg(WY, 0);
        p.write_reg(WX, 167);
    });
    assert_eq!(got, want(0));
}

/// 在一行的每个 M-cycle 读写一次 VRAM/OAM，返回 (OAM 读, OAM 写, VRAM 读, VRAM 写) 是否被封锁。
fn lock_profile(p: &mut Ppu, len: usize) -> Vec<(bool, bool, bool, bool)> {
    let mut out = Vec::new();
    for k in 0..len {
        let oam_r = p.read_oam(0xFE9F) == 0xFF;
        let vram_r = p.read_vram(0x9FFF) == 0xFF;
        p.write_oam(0xFE00 + k as u16, 0xA5);
        p.write_vram(0x8800 + k as u16, 0xA5);
        out.push((oam_r, false, vram_r, false));
        p.tick_mcycle();
    }
    for (k, e) in out.iter_mut().enumerate() {
        e.1 = p.peek_oam(0xFE00 + k as u16) != 0xA5;
        e.3 = p.peek_vram(0x8800 + k as u16) != 0xA5;
    }
    out
}

fn fresh_with_lcd_off() -> Ppu {
    let mut p = Ppu::new();
    p.write_reg(LCDC, 0x11);
    for a in 0xFE00..0xFEA0 {
        p.write_oam(a, 0);
    }
    for a in 0x8000..0xA000 {
        p.write_vram(a, 0);
    }
    p
}

#[test]
fn access_locks_on_a_normal_line() {
    let mut p = fresh_with_lcd_off();
    p.write_reg(LCDC, 0x91);
    run_to_line(&mut p, 10);
    for (k, &(oam_r, oam_w, vram_r, vram_w)) in lock_profile(&mut p, 114).iter().enumerate() {
        let d = 4 * k;
        assert_eq!(oam_r, d < 256, "OAM read at dot {d}");
        assert_eq!(
            oam_w,
            (4..80).contains(&d) || (84..256).contains(&d),
            "OAM write at dot {d}"
        );
        assert_eq!(vram_r, (80..256).contains(&d), "VRAM read at dot {d}");
        assert_eq!(vram_w, (84..256).contains(&d), "VRAM write at dot {d}");
    }
}

#[test]
fn access_locks_on_the_first_line_after_lcd_on() {
    let mut p = fresh_with_lcd_off();
    p.write_reg(LCDC, 0x91);
    for (k, &(oam_r, oam_w, vram_r, vram_w)) in lock_profile(&mut p, 113).iter().enumerate() {
        let d = 4 * k;
        let locked = (80..252).contains(&d);
        assert_eq!(
            (oam_r, oam_w, vram_r, vram_w),
            (locked, locked, locked, locked),
            "dot {d}"
        );
    }
    assert_eq!(p.read_reg(LY), 1);
}

#[test]
fn locks_scale_with_mode3_length_and_vanish_when_lcd_is_off() {
    let mut p = fresh_with_lcd_off();
    p.write_reg(SCX, 5); // p = 5：STAT 与解锁都在 ceil4(258) = 260
    p.write_reg(LCDC, 0x91);
    run_to_line(&mut p, 20);
    let prof = lock_profile(&mut p, 114);
    assert!(prof[64].0 && prof[64].2, "still locked at dot 256");
    assert!(!prof[65].0 && !prof[65].2, "unlocked at dot 260");
    // LCD 关闭：随时可访问。
    run_to_line(&mut p, 30);
    for _ in 0..30 {
        p.tick_mcycle();
    }
    assert_eq!(p.read_vram(0x9FFF), 0xFF); // mode 3 中
    p.write_reg(LCDC, 0x11);
    assert_eq!(p.read_vram(0x9FFF), 0x00);
    assert_eq!(p.read_oam(0xFE9F), 0x00);
}

#[test]
fn peek_ignores_locks() {
    let mut p = fresh_with_lcd_off();
    p.write_vram(0x9000, 0x3C);
    p.write_oam(0xFE10, 0x7E);
    p.write_reg(LCDC, 0x91);
    run_to_line(&mut p, 50);
    for _ in 0..40 {
        p.tick_mcycle(); // dot 160：mode 3
    }
    assert_eq!(p.read_reg(STAT) & 3, 3);
    assert_eq!(p.read_vram(0x9000), 0xFF);
    assert_eq!(p.read_oam(0xFE10), 0xFF);
    assert_eq!(p.peek_vram(0x9000), 0x3C);
    assert_eq!(p.peek_oam(0xFE10), 0x7E);
    // 被封锁的写入不生效。
    p.write_vram(0x9000, 0x00);
    p.write_oam(0xFE10, 0x00);
    assert_eq!(p.peek_vram(0x9000), 0x3C);
    assert_eq!(p.peek_oam(0xFE10), 0x7E);
}

#[test]
fn lyc_flag_reads_0_at_line_start_then_compares_at_dot_4() {
    let mut p = Ppu::new();
    p.write_reg(LYC, 0x30);
    p.write_reg(LCDC, 0x11);
    p.write_reg(LCDC, 0x91);
    p.write_reg(STAT, 0x40);
    p.take_irq();
    loop {
        let r = p.tick_mcycle();
        if p.read_reg(LY) == 0x30 {
            assert_eq!(r & irq::STAT, 0, "no LYC interrupt at dot 0");
            break;
        }
    }
    assert_eq!(p.read_reg(STAT) & 0x04, 0, "STAT.2 reads 0 at dot 0");
    assert_eq!(
        p.tick_mcycle() & irq::STAT,
        irq::STAT,
        "LYC interrupt at dot 4"
    );
    assert_eq!(p.read_reg(STAT) & 0x07, 0x06, "mode 2 and LYC=LY at dot 4");
    // 到下一行 dot 0：LY=0x31，STAT.2 立刻读 0。
    run_to_line(&mut p, 0x31);
    assert_eq!(p.read_reg(STAT) & 0x07, 0x00);
}
