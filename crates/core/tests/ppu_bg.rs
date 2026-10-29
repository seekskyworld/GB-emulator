//! T41：BG/Window 渲染。用合成的 VRAM 直接断言前缓冲的像素。

use gb_core::ppu::Ppu;
use gb_core::{irq, Frame, Peripheral, FRAME_T, SCREEN_H, SCREEN_W};

const LCDC: u16 = 0xFF40;
const SCY: u16 = 0xFF42;
const SCX: u16 = 0xFF43;
const LY: u16 = 0xFF44;
const BGP: u16 = 0xFF47;
const WY: u16 = 0xFF4A;
const WX: u16 = 0xFF4B;

const SHADES: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
/// 恒等调色板：色号 n → 灰度 n。
const BGP_ID: u8 = 0xE4;

/// 色号按恒等调色板映射后的灰度。
fn g(c: u8) -> u8 {
    SHADES[c as usize]
}

/// 把 8×8 tile（`color(row, col)` 给出每个像素的色号）写到 VRAM 绝对地址 `addr`。
fn put_tile(p: &mut Ppu, addr: u16, color: impl Fn(usize, usize) -> u8) {
    for r in 0..8 {
        let (mut lo, mut hi) = (0u8, 0u8);
        for c in 0..8 {
            let v = color(r, c);
            lo |= (v & 1) << (7 - c);
            hi |= ((v >> 1) & 1) << (7 - c);
        }
        p.write_vram(addr + r as u16 * 2, lo);
        p.write_vram(addr + r as u16 * 2 + 1, hi);
    }
}

/// 把 32×32 tilemap（基址 0x9800 或 0x9C00）按 `tile(row, col)` 填满。
fn put_map(p: &mut Ppu, base: u16, tile: impl Fn(usize, usize) -> u8) {
    for r in 0..32 {
        for c in 0..32 {
            p.write_vram(base + (r * 32 + c) as u16, tile(r, c));
        }
    }
}

/// 推进到下一个 VBlank 中断（含）。
fn run_to_vblank(p: &mut Ppu) {
    for _ in 0..2 * FRAME_T / 4 {
        if p.tick_mcycle() & irq::VBLANK != 0 {
            return;
        }
    }
    panic!("no VBlank within two frames");
}

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

fn run(p: &mut Ppu, n: usize) {
    for _ in 0..n {
        p.tick_mcycle();
    }
}

/// 以 `lcdc`（bit7 会被置 1）从第 0 行开始画完整一帧，返回前缓冲的拷贝。
fn draw_frame(p: &mut Ppu, lcdc: u8) -> Box<Frame> {
    p.write_reg(LCDC, lcdc & 0x7F);
    p.write_reg(LCDC, lcdc | 0x80);
    run_to_vblank(p);
    Box::new(*p.frame())
}

fn px(f: &Frame, x: usize, y: usize) -> u8 {
    f[y * SCREEN_W + x]
}

/// 逐像素比较整帧，失败时报告第一个不同的位置。
fn assert_frame(f: &Frame, want: impl Fn(usize, usize) -> u8) {
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            assert_eq!(px(f, x, y), want(x, y), "pixel (x={x}, y={y})");
        }
    }
}

/// 两种带内部图案的 tile：用来同时检查 tile 选择与行内像素顺序。
fn pattern(t: usize, r: usize, c: usize) -> u8 {
    match t {
        0 => ((r + c) % 4) as u8,
        _ => (3 - (r * 3 + c) % 4) as u8,
    }
}

fn checkerboard(p: &mut Ppu) {
    put_tile(p, 0x8000, |r, c| pattern(0, r, c));
    put_tile(p, 0x8010, |r, c| pattern(1, r, c));
    put_map(p, 0x9800, |r, c| ((r + c) % 2) as u8);
}

fn expect_checker(x: usize, y: usize, scx: usize, scy: usize) -> u8 {
    let bx = (x + scx) & 255;
    let by = (y + scy) & 255;
    let t = (bx / 8 + by / 8) % 2;
    g(pattern(t, by % 8, bx % 8))
}

#[test]
fn checkerboard_scrolled_by_scx3_scy5() {
    let mut p = Ppu::new();
    checkerboard(&mut p);
    p.write_reg(BGP, BGP_ID);
    p.write_reg(SCX, 3);
    p.write_reg(SCY, 5);
    let f = draw_frame(&mut p, 0x91);
    assert_frame(&f, |x, y| expect_checker(x, y, 3, 5));
}

#[test]
fn scrolling_wraps_around_the_256x256_map() {
    let mut p = Ppu::new();
    checkerboard(&mut p);
    // 让 map 的四个角可区分：第 31 列与第 31 行改用 tile 2（纯色 3）。
    put_tile(&mut p, 0x8020, |_, _| 3);
    for i in 0..32u16 {
        p.write_vram(0x9800 + 31 * 32 + i, 2);
        p.write_vram(0x9800 + i * 32 + 31, 2);
    }
    p.write_reg(BGP, BGP_ID);
    p.write_reg(SCX, 200);
    p.write_reg(SCY, 250);
    let f = draw_frame(&mut p, 0x91);
    assert_frame(&f, |x, y| {
        let bx = (x + 200) & 255;
        let by = (y + 250) & 255;
        if bx / 8 == 31 || by / 8 == 31 {
            g(3)
        } else {
            expect_checker(x, y, 200, 250)
        }
    });
}

#[test]
fn bgp_maps_to_exact_grey_levels() {
    let mut p = Ppu::new();
    // 每 8 列一个色号：第 c 列 tile 为纯色 c%4。
    for t in 0..4u16 {
        put_tile(&mut p, 0x8000 + t * 16, |_, _| t as u8);
    }
    put_map(&mut p, 0x9800, |_, c| (c % 4) as u8);
    for bgp in [0xE4u8, 0x1B, 0xFC, 0x00, 0xFF, 0x9C] {
        p.write_reg(BGP, bgp);
        let f = draw_frame(&mut p, 0x91);
        assert_frame(&f, |x, _| SHADES[((bgp >> ((x / 8 % 4) * 2)) & 3) as usize]);
    }
    let f = draw_frame(&mut p, 0x91);
    assert!(f.iter().all(|v| [0xFF, 0xAA, 0x55, 0x00].contains(v)));
}

#[test]
fn power_up_bgp_renders_color_0_white() {
    let mut p = Ppu::new();
    put_tile(&mut p, 0x8010, |_, _| 3);
    put_map(&mut p, 0x9800, |r, _| (r % 2) as u8);
    let f = draw_frame(&mut p, 0x91); // BGP=0xFC：色号 0 → 白，1..3 → 黑
    assert_frame(&f, |_, y| if (y / 8) % 2 == 0 { 0xFF } else { 0x00 });
}

/// BG 用 0x9800 的 tile 1（纯色 1），窗口用 0x9C00 的 tile 2（图案）。
fn bg_and_window(p: &mut Ppu) {
    put_tile(p, 0x8010, |_, _| 1);
    put_tile(p, 0x8020, |r, c| pattern(1, r, c));
    put_map(p, 0x9800, |_, _| 1);
    put_map(p, 0x9C00, |_, _| 2);
    p.write_reg(BGP, BGP_ID);
}

#[test]
fn window_covers_from_wx_minus_7_and_wy() {
    let mut p = Ppu::new();
    bg_and_window(&mut p);
    p.write_reg(WX, 7 + 20);
    p.write_reg(WY, 30);
    // 窗口 map 0x9C00（LCDC.6=1）、BG map 0x9800、窗口开启。
    let f = draw_frame(&mut p, 0x91 | 0x60);
    assert_frame(&f, |x, y| {
        if y >= 30 && x >= 20 {
            g(pattern(1, (y - 30) % 8, (x - 20) % 8))
        } else {
            g(1)
        }
    });
    // 窗口关闭（LCDC.5=0）时完全是 BG。
    let f = draw_frame(&mut p, 0x91 | 0x40);
    assert_frame(&f, |_, _| g(1));
    // 交换两张 map：BG 用 0x9C00（LCDC.3=1），窗口用 0x9800（LCDC.6=0）。
    let f = draw_frame(&mut p, 0x91 | 0x28);
    assert_frame(&f, |x, y| {
        if y >= 30 && x >= 20 {
            g(1)
        } else {
            g(pattern(1, y % 8, x % 8))
        }
    });
}

#[test]
fn window_ignores_scroll_and_wx_edges() {
    let mut p = Ppu::new();
    bg_and_window(&mut p);
    p.write_reg(SCX, 13);
    p.write_reg(SCY, 77);
    p.write_reg(WY, 0);
    // WX=7：窗口盖满整屏，不受 SCX/SCY 影响。
    p.write_reg(WX, 7);
    let f = draw_frame(&mut p, 0xF1);
    assert_frame(&f, |x, y| g(pattern(1, y % 8, x % 8)));
    // WX=166：只剩最右一列。
    p.write_reg(WX, 166);
    let f = draw_frame(&mut p, 0xF1);
    assert_frame(&f, |x, y| {
        if x == 159 {
            g(pattern(1, y % 8, 0))
        } else {
            g(1)
        }
    });
    // WX=167：不可见。
    p.write_reg(WX, 167);
    let f = draw_frame(&mut p, 0xF1);
    assert_frame(&f, |_, _| g(1));
    // WX=3：窗口左边 4 列被裁掉。
    p.write_reg(WX, 3);
    let f = draw_frame(&mut p, 0xF1);
    assert_frame(&f, |x, y| g(pattern(1, y % 8, (x + 4) % 8)));
}

#[test]
fn wy_set_to_a_passed_line_mid_frame_shows_no_window() {
    let mut p = Ppu::new();
    bg_and_window(&mut p);
    p.write_reg(WX, 7);
    p.write_reg(WY, 200);
    p.write_reg(LCDC, 0x71);
    p.write_reg(LCDC, 0xF1);
    run_to_line(&mut p, 50);
    p.write_reg(WY, 30); // 第 30 行已经过去
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |_, _| g(1));
    // 下一帧从第 30 行开始出现。
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |x, y| {
        if y >= 30 {
            g(pattern(1, (y - 30) % 8, x % 8))
        } else {
            g(1)
        }
    });
}

#[test]
fn wy_condition_stays_latched_after_wy_changes() {
    let mut p = Ppu::new();
    bg_and_window(&mut p);
    p.write_reg(WX, 7);
    p.write_reg(WY, 20);
    p.write_reg(LCDC, 0x71);
    p.write_reg(LCDC, 0xF1);
    run_to_line(&mut p, 40);
    p.write_reg(WY, 100); // Y 条件已在第 20 行锁存，本帧保持
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |x, y| {
        if y >= 20 {
            g(pattern(1, (y - 20) % 8, x % 8))
        } else {
            g(1)
        }
    });
}

#[test]
fn window_line_counter_only_advances_on_drawn_lines() {
    let mut p = Ppu::new();
    put_tile(&mut p, 0x8010, |_, _| 1);
    put_map(&mut p, 0x9800, |_, _| 1);
    // 窗口 map 的每个 tile 行用不同的 tile：tile 0x10+n 的每一行像素都等于 (n + 行号) % 4。
    for n in 0..18u16 {
        put_tile(&mut p, 0x8000 + (0x10 + n) * 16, |r, _| {
            ((n as usize + r) % 4) as u8
        });
    }
    put_map(&mut p, 0x9C00, |r, _| 0x10 + r as u8);
    p.write_reg(BGP, BGP_ID);
    let win_row = |row: usize| g(((row / 8 + row % 8) % 4) as u8);
    p.write_reg(WX, 7);
    p.write_reg(WY, 10);
    p.write_reg(LCDC, 0x71);
    p.write_reg(LCDC, 0xF1);
    run_to_line(&mut p, 20);
    p.write_reg(WX, 200); // 第 20–39 行隐藏窗口
    run_to_line(&mut p, 40);
    p.write_reg(WX, 7);
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |_, y| match y {
        10..=19 => win_row(y - 10),
        40.. => win_row(y - 30), // 接着第 10 行窗口内容继续
        _ => g(1),
    });
    // 下一帧计数器清零，第 10–19 行仍是窗口第 0–9 行。
    run_to_vblank(&mut p);
    assert_eq!(px(p.frame(), 0, 10), win_row(0));
}

#[test]
fn window_disabled_by_lcdc5_does_not_advance_counter() {
    let mut p = Ppu::new();
    put_tile(&mut p, 0x8010, |_, _| 1);
    put_map(&mut p, 0x9800, |_, _| 1);
    for n in 0..18u16 {
        put_tile(&mut p, 0x8000 + (0x10 + n) * 16, |r, _| {
            ((n as usize + r) % 4) as u8
        });
    }
    put_map(&mut p, 0x9C00, |r, _| 0x10 + r as u8);
    p.write_reg(BGP, BGP_ID);
    let win_row = |row: usize| g(((row / 8 + row % 8) % 4) as u8);
    p.write_reg(WX, 7);
    p.write_reg(WY, 0);
    p.write_reg(LCDC, 0x51);
    p.write_reg(LCDC, 0xD1); // 窗口关闭，但 Y 条件在第 0 行已满足
    run_to_line(&mut p, 60);
    p.write_reg(LCDC, 0xF1);
    run_to_vblank(&mut p);
    assert_frame(
        p.frame(),
        |_, y| if y >= 60 { win_row(y - 60) } else { g(1) },
    );
}

#[test]
fn signed_and_unsigned_tile_addressing() {
    let mut p = Ppu::new();
    // 每个可能被选中的位置放不同的纯色：
    put_tile(&mut p, 0x8000, |_, _| 1); // 无符号 0x00
    put_tile(&mut p, 0x9000, |_, _| 2); // 有符号 0x00
    put_tile(&mut p, 0x8800, |_, _| 3); // 0x80（两种方式相同）
    put_tile(&mut p, 0x87F0, |_, _| 1); // 无符号 0x7F
    put_tile(&mut p, 0x97F0, |r, c| ((r + c) % 4) as u8); // 有符号 0x7F
    put_tile(&mut p, 0x8FF0, |r, _| (r % 4) as u8); // 0xFF（两种方式相同）
    let tiles = [0x00u8, 0x80, 0x7F, 0xFF];
    put_map(&mut p, 0x9800, |_, c| tiles[c % 4]);
    p.write_reg(BGP, BGP_ID);
    let signed = draw_frame(&mut p, 0x81); // LCDC.4=0
    assert_frame(&signed, |x, y| match tiles[x / 8 % 4] {
        0x00 => g(2),
        0x80 => g(3),
        0x7F => g(((y + x) % 8 % 4) as u8),
        _ => g((y % 8 % 4) as u8),
    });
    let unsigned = draw_frame(&mut p, 0x91); // LCDC.4=1
    assert_frame(&unsigned, |x, y| match tiles[x / 8 % 4] {
        0x00 => g(1),
        0x80 => g(3),
        0x7F => g(1),
        _ => g((y % 8 % 4) as u8),
    });
}

#[test]
fn lcdc0_off_shows_bg_color_0_for_bg_and_window() {
    let mut p = Ppu::new();
    bg_and_window(&mut p);
    p.write_reg(WX, 7);
    p.write_reg(WY, 0);
    p.write_reg(BGP, 0x1B); // 色号 0 → 调色板值 3（黑），确认用的是 BGP 的第 0 项
    let f = draw_frame(&mut p, 0xF0);
    assert_frame(&f, |_, _| 0x00);
}

#[test]
fn register_writes_in_mode_2_apply_to_the_same_line() {
    let mut p = Ppu::new();
    // 第 c 列 tile 为纯色 c%4：SCX 每 8 像素换一种颜色。
    for t in 0..4u16 {
        put_tile(&mut p, 0x8000 + t * 16, |_, _| t as u8);
    }
    put_map(&mut p, 0x9800, |_, c| (c % 4) as u8);
    p.write_reg(BGP, BGP_ID);
    p.write_reg(LCDC, 0x11);
    p.write_reg(LCDC, 0x91);
    run_to_line(&mut p, 60);
    run(&mut p, 19); // dot 76，仍在 mode 2
    assert_eq!(p.read_reg(0xFF41) & 3, 2);
    p.write_reg(SCX, 8);
    run_to_line(&mut p, 70);
    run(&mut p, 64); // dot 256，mode 0
    assert_eq!(p.read_reg(0xFF41) & 3, 0);
    p.write_reg(SCX, 16);
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |x, y| {
        let scx = match y {
            0..=59 => 0,
            60..=70 => 8,
            _ => 16,
        };
        g((((x + scx) / 8) % 4) as u8)
    });
}

#[test]
fn mid_mode_3_write_splits_the_line_at_the_output_position() {
    let mut p = Ppu::new();
    put_tile(&mut p, 0x8000, |_, _| 1);
    put_map(&mut p, 0x9800, |_, _| 0);
    p.write_reg(BGP, BGP_ID);
    p.write_reg(LCDC, 0x11);
    p.write_reg(LCDC, 0x91);
    run_to_line(&mut p, 80);
    run(&mut p, 33); // dot 132：像素 0..40 已输出（第 x 个像素在 dot 92+x）
    assert_eq!(p.read_reg(0xFF41) & 3, 3);
    p.write_reg(BGP, 0xE7); // 色号 1 → 1，色号 0 → 3
    p.write_reg(BGP, 0xE8); // 同一 dot 内再写一次：色号 1 → 2
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |x, y| match y {
        0..=79 => g(1),
        80 if x < 40 => g(1),
        _ => g(2),
    });
}

#[test]
fn vram_written_during_mode_2_shows_on_that_line() {
    let mut p = Ppu::new();
    put_map(&mut p, 0x9800, |_, _| 0);
    p.write_reg(BGP, BGP_ID);
    p.write_reg(LCDC, 0x11);
    p.write_reg(LCDC, 0x91);
    run_to_line(&mut p, 16);
    run(&mut p, 5);
    put_tile(&mut p, 0x8000, |_, _| 3);
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |_, y| if y >= 16 { g(3) } else { g(0) });
}

#[test]
fn lcd_off_whitens_the_front_buffer_once() {
    let mut p = Ppu::new();
    put_tile(&mut p, 0x8000, |_, _| 3);
    put_map(&mut p, 0x9800, |_, _| 0);
    p.write_reg(BGP, BGP_ID);
    let f = draw_frame(&mut p, 0x91);
    assert!(f.iter().all(|&v| v == 0x00));
    run(&mut p, 3000);
    p.write_reg(LCDC, 0x11);
    assert!(p.frame().iter().all(|&v| v == 0xFF));
    // 关闭期间画面保持全白，tick 不改变它。
    run(&mut p, 2 * FRAME_T as usize / 4);
    assert!(p.frame().iter().all(|&v| v == 0xFF));
    // 重新开启后第一次 VBlank 交出完整的新帧。
    p.write_reg(LCDC, 0x91);
    run_to_vblank(&mut p);
    assert!(p.frame().iter().all(|&v| v == 0x00));
}

#[test]
fn front_buffer_only_changes_at_vblank() {
    let mut p = Ppu::new();
    put_tile(&mut p, 0x8000, |_, _| 3);
    put_map(&mut p, 0x9800, |_, _| 0);
    p.write_reg(BGP, BGP_ID);
    let f = draw_frame(&mut p, 0x91);
    assert!(f.iter().all(|&v| v == 0x00));
    // 下一帧画到一半时改调色板：前缓冲不变，直到 VBlank 才换成新帧。
    run_to_line(&mut p, 72);
    p.write_reg(BGP, 0x00);
    run_to_line(&mut p, 143);
    assert!(p.frame().iter().all(|&v| v == 0x00));
    run_to_vblank(&mut p);
    let f = p.frame();
    assert!(f[..72 * SCREEN_W].iter().all(|&v| v == 0x00));
    assert!(f[72 * SCREEN_W..].iter().all(|&v| v == 0xFF));
}
