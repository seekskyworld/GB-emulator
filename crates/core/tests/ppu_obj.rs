//! T42：精灵（OAM scan、10 个上限、8×8/8×16、翻转、调色板、优先级、OBJ-to-BG priority、LCDC.1）。
//! 用合成的 VRAM/OAM 直接断言前缓冲的像素。

use gb_core::ppu::Ppu;
use gb_core::{
    irq, Frame, Peripheral, Snapshot, StateReader, StateWriter, FRAME_T, SCREEN_H, SCREEN_W,
};

const LCDC: u16 = 0xFF40;
const LY: u16 = 0xFF44;
const BGP: u16 = 0xFF47;
const OBP0: u16 = 0xFF48;
const OBP1: u16 = 0xFF49;

const SHADES: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];
const WHITE: u8 = 0xFF;

/// LCD 开、BG 开、精灵开、tile data 0x8000、8×8。
const LCDC_8X8: u8 = 0x93;
/// 同上但 8×16。
const LCDC_8X16: u8 = 0x97;

fn g(c: u8) -> u8 {
    SHADES[c as usize]
}

fn put_tile(p: &mut Ppu, index: u8, color: impl Fn(usize, usize) -> u8) {
    let addr = 0x8000 + index as u16 * 16;
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

/// 写第 `i` 个 OAM 项：屏幕坐标 (x, y)（内部换算成 X+8、Y+16）。
fn obj(p: &mut Ppu, i: u16, x: i32, y: i32, tile: u8, attr: u8) {
    let base = 0xFE00 + i * 4;
    p.write_oam(base, (y + 16) as u8);
    p.write_oam(base + 1, (x + 8) as u8);
    p.write_oam(base + 2, tile);
    p.write_oam(base + 3, attr);
}

/// 新 PPU：OAM 全部移出屏幕（Y=0），BG map 全为 tile 0（全色号 0），恒等调色板。
fn setup() -> Ppu {
    let mut p = Ppu::new();
    for a in 0xFE00..0xFEA0 {
        p.write_oam(a, 0);
    }
    p.write_reg(BGP, 0xE4);
    p.write_reg(OBP0, 0xE4);
    p.write_reg(OBP1, 0x1B);
    p
}

fn run_to_vblank(p: &mut Ppu) {
    for _ in 0..2 * FRAME_T / 4 {
        if p.tick_mcycle() & irq::VBLANK != 0 {
            return;
        }
    }
    panic!("no VBlank within two frames");
}

fn run_to_line(p: &mut Ppu, ly: u8) {
    for _ in 0..2 * FRAME_T / 4 {
        p.tick_mcycle();
        if p.read_reg(LY) == ly {
            return;
        }
    }
    panic!("LY never reached {ly}");
}

fn draw_frame(p: &mut Ppu, lcdc: u8) -> Box<Frame> {
    p.write_reg(LCDC, lcdc & 0x7F);
    p.write_reg(LCDC, lcdc | 0x80);
    run_to_vblank(p);
    Box::new(*p.frame())
}

fn assert_frame(f: &Frame, want: impl Fn(usize, usize) -> u8) {
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            assert_eq!(f[y * SCREEN_W + x], want(x, y), "pixel (x={x}, y={y})");
        }
    }
}

fn inside(x: usize, y: usize, x0: i32, y0: i32, w: i32, h: i32) -> bool {
    let (x, y) = (x as i32, y as i32);
    x >= x0 && x < x0 + w && y >= y0 && y < y0 + h
}

#[test]
fn only_the_first_ten_objects_on_a_line_are_drawn() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 3);
    // 第 20–27 行有 11 个互不重叠的精灵：OAM 序号 10 的那个不画。
    for i in 0..11 {
        obj(&mut p, i, 12 * i as i32, 20, 1, 0);
    }
    // 第 40–47 行：序号 11 的精灵 X=0（屏幕外）也占名额，序号 12..=21 中最后一个不画。
    obj(&mut p, 11, -8, 40, 1, 0);
    for i in 12..22 {
        obj(&mut p, i, 12 * (i as i32 - 12), 40, 1, 0);
    }
    // 第 60–67 行只有 1 个精灵，与上面序号 10 同一 X：可见（上限按行计）。
    obj(&mut p, 22, 120, 60, 1, 0);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        let hit = (0..10).any(|i| inside(x, y, 12 * i, 20, 8, 8))
            || (0..9).any(|i| inside(x, y, 12 * i, 40, 8, 8))
            || inside(x, y, 120, 60, 8, 8);
        if hit {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn offscreen_x_objects_still_count_toward_the_limit() {
    // 10 个 Y 命中但 X 都在屏幕外的精灵挤掉后面可见的那个。
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 3);
    for i in 0..10 {
        obj(&mut p, i, 160, 50, 1, 0); // X=168：屏幕外
    }
    obj(&mut p, 10, 80, 50, 1, 0);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert!(f.iter().all(|&v| v == WHITE));
    // 把一个移到别的行，名额空出来。
    obj(&mut p, 0, 160, 100, 1, 0);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 80, 50, 8, 8) {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn same_x_lower_oam_index_wins() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 1);
    put_tile(&mut p, 2, |_, _| 3);
    obj(&mut p, 5, 30, 30, 1, 0);
    obj(&mut p, 6, 30, 30, 2, 0);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 30, 30, 8, 8) {
            g(1)
        } else {
            WHITE
        }
    });
    // 交换两者的 OAM 位置后反过来。
    obj(&mut p, 5, 30, 30, 2, 0);
    obj(&mut p, 6, 30, 30, 1, 0);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 30, 30, 8, 8) {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn lower_x_wins_even_when_later_in_oam() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 1);
    put_tile(&mut p, 2, |_, _| 3);
    obj(&mut p, 0, 50, 30, 2, 0); // x 50..57，OAM 在前
    obj(&mut p, 1, 46, 30, 1, 0); // x 46..53，X 更小：重叠处在上
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 46, 30, 8, 8) {
            g(1)
        } else if inside(x, y, 50, 30, 8, 8) {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn transparent_pixels_show_lower_priority_objects_and_bg() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, c| if c < 4 { 0 } else { 2 }); // 左半透明
    put_tile(&mut p, 2, |_, _| 3);
    obj(&mut p, 0, 40, 40, 1, 0);
    obj(&mut p, 1, 40, 40, 2, 0);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 44, 40, 4, 8) {
            g(2)
        } else if inside(x, y, 40, 40, 4, 8) {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn obj_palettes_map_colors_and_color_0_is_transparent() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, c| (c % 4) as u8);
    // BG 改成纯色 1，以便看到透明。
    put_tile(&mut p, 0, |_, _| 1);
    obj(&mut p, 0, 0, 0, 1, 0x00); // OBP0 = E4（恒等）
    obj(&mut p, 1, 0, 16, 1, 0x10); // OBP1 = 1B（反转）
    p.write_reg(OBP0, 0xE7); // 低两位应被忽略：色号 0 仍透明
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        let c = (x % 4) as u8;
        if x < 8 && y < 8 && c != 0 {
            SHADES[((0xE7u8 >> (c * 2)) & 3) as usize]
        } else if x < 8 && (16..24).contains(&y) && c != 0 {
            SHADES[((0x1Bu8 >> (c * 2)) & 3) as usize]
        } else {
            g(1)
        }
    });
}

/// 每个像素都不同的 tile：色号 = (r*3 + c) % 4。
fn asym(r: usize, c: usize) -> u8 {
    ((r * 3 + c) % 4) as u8
}

#[test]
fn x_and_y_flip() {
    let mut p = setup();
    put_tile(&mut p, 1, asym);
    obj(&mut p, 0, 10, 10, 1, 0x00);
    obj(&mut p, 1, 30, 10, 1, 0x20); // X 翻转
    obj(&mut p, 2, 50, 10, 1, 0x40); // Y 翻转
    obj(&mut p, 3, 70, 10, 1, 0x60); // 两者
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if !(10..18).contains(&y) {
            return WHITE;
        }
        let r = y - 10;
        let c = match x {
            10..=17 => asym(r, x - 10),
            30..=37 => asym(r, 7 - (x - 30)),
            50..=57 => asym(7 - r, x - 50),
            70..=77 => asym(7 - r, 7 - (x - 70)),
            _ => return WHITE,
        };
        if c == 0 {
            WHITE
        } else {
            g(c)
        }
    });
}

#[test]
fn objects_clip_at_screen_edges() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 3);
    obj(&mut p, 0, -5, -6, 1, 0); // 左上角只露出 3×2
    obj(&mut p, 1, 157, 141, 1, 0); // 右下角只露出 3×3
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if (x < 3 && y < 2) || (x >= 157 && y >= 141) {
            g(3)
        } else {
            WHITE
        }
    });
}

/// BG：左边 80 列为色号 0，右边 80 列为色号 2。
fn split_bg(p: &mut Ppu) {
    put_tile(p, 0, |_, _| 0);
    put_tile(p, 3, |_, _| 2);
    for r in 0..32u16 {
        for c in 0..32u16 {
            p.write_vram(0x9800 + r * 32 + c, if c < 10 { 0 } else { 3 });
        }
    }
}

#[test]
fn obj_to_bg_priority_only_yields_to_bg_colors_1_to_3() {
    let mut p = setup();
    split_bg(&mut p);
    put_tile(&mut p, 1, |_, _| 1);
    obj(&mut p, 0, 76, 20, 1, 0x80); // 跨过 x=80：左半在色号 0 上，右半在色号 2 上
    obj(&mut p, 1, 76, 40, 1, 0x00); // 无 priority：两边都在上
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        let bg = if x < 80 { g(0) } else { g(2) };
        if inside(x, y, 76, 20, 4, 8) || inside(x, y, 76, 40, 8, 8) {
            g(1)
        } else {
            bg
        }
    });
    // LCDC.0=0：BG 变成色号 0，priority 精灵也全部可见。
    let f = draw_frame(&mut p, LCDC_8X8 & !0x01);
    assert_frame(&f, |x, y| {
        if inside(x, y, 76, 20, 8, 8) || inside(x, y, 76, 40, 8, 8) {
            g(1)
        } else {
            g(0)
        }
    });
}

#[test]
fn obj_to_bg_priority_also_applies_over_the_window() {
    let mut p = setup();
    put_tile(&mut p, 3, |_, _| 2);
    for i in 0..0x400u16 {
        p.write_vram(0x9C00 + i, 3); // 窗口全为色号 2
    }
    put_tile(&mut p, 1, |_, _| 1);
    p.write_reg(0xFF4A, 0); // WY
    p.write_reg(0xFF4B, 7 + 80); // WX：窗口从 x=80 开始
    obj(&mut p, 0, 76, 20, 1, 0x80);
    let f = draw_frame(&mut p, LCDC_8X8 | 0x60);
    assert_frame(&f, |x, y| {
        if x >= 80 {
            g(2)
        } else if inside(x, y, 76, 20, 8, 8) {
            g(1)
        } else {
            g(0)
        }
    });
}

#[test]
fn higher_priority_object_behind_bg_masks_lower_ones() {
    // 优先级先在精灵之间决出（不看 priority 位），再拿胜者与 BG 比较（OAM.md "Drawing priority"）。
    let mut p = setup();
    split_bg(&mut p);
    put_tile(&mut p, 1, |_, _| 1);
    put_tile(&mut p, 2, |_, _| 3);
    obj(&mut p, 0, 100, 30, 1, 0x80); // X 小、在 BG 后面
    obj(&mut p, 1, 104, 30, 2, 0x00); // X 大、在 BG 前面
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 100, 30, 8, 8) {
            g(2) // 重叠区也被前一个精灵"挡住"而露出 BG
        } else if inside(x, y, 108, 30, 4, 8) {
            g(3)
        } else if x < 80 {
            g(0)
        } else {
            g(2)
        }
    });
}

/// 8×16 用的两个 tile：tile 4 的第 r 行色号 r%4，tile 5 的第 r 行色号 (r+2)%4（各行每列相同）。
fn tall_tiles(p: &mut Ppu) {
    put_tile(p, 4, |r, _| (r % 4) as u8);
    put_tile(p, 5, |r, _| ((r + 2) % 4) as u8);
}

/// 8×16 精灵第 `row`（0..16）行的色号（未翻转）。
fn tall_row(row: usize) -> u8 {
    if row < 8 {
        (row % 4) as u8
    } else {
        ((row - 8 + 2) % 4) as u8
    }
}

fn shade_or_white(c: u8) -> u8 {
    if c == 0 {
        WHITE
    } else {
        g(c)
    }
}

#[test]
fn tall_objects_ignore_tile_bit0_and_flip_as_a_whole() {
    let mut p = setup();
    tall_tiles(&mut p);
    obj(&mut p, 0, 10, 40, 4, 0x00);
    obj(&mut p, 1, 30, 40, 5, 0x00); // bit0 被忽略：与上一个相同
    obj(&mut p, 2, 50, 40, 5, 0x40); // Y 翻转：上下 tile 互换且各自翻转
    let f = draw_frame(&mut p, LCDC_8X16);
    assert_frame(&f, |x, y| {
        if !(40..56).contains(&y) {
            return WHITE;
        }
        let row = y - 40;
        match x {
            10..=17 | 30..=37 => shade_or_white(tall_row(row)),
            50..=57 => shade_or_white(tall_row(15 - row)),
            _ => WHITE,
        }
    });
    // 8×8 模式下同样的 OAM 只画上面 8 行，tile 5 就是 tile 5。
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if !(40..48).contains(&y) {
            return WHITE;
        }
        let r = y - 40;
        match x {
            10..=17 => shade_or_white((r % 4) as u8),
            30..=37 => shade_or_white(((r + 2) % 4) as u8),
            50..=57 => shade_or_white(((7 - r + 2) % 4) as u8),
            _ => WHITE,
        }
    });
}

#[test]
fn tall_objects_are_selected_over_16_lines() {
    // 8×16 时 OAM scan 用 16 行高度：Y 命中下半的精灵也占名额。
    let mut p = setup();
    put_tile(&mut p, 2, |_, _| 3);
    put_tile(&mut p, 3, |_, _| 3);
    for i in 0..10 {
        obj(&mut p, i, 160, 52, 2, 0); // 屏幕外 X，第 52–67 行
    }
    obj(&mut p, 10, 40, 60, 2, 0); // 第 60–75 行
    let f = draw_frame(&mut p, LCDC_8X16);
    assert_frame(&f, |x, y| {
        if inside(x, y, 40, 68, 8, 8) {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn lcdc1_off_hides_objects() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 3);
    obj(&mut p, 0, 20, 10, 1, 0);
    obj(&mut p, 1, 20, 60, 1, 0);
    let f = draw_frame(&mut p, LCDC_8X8 & !0x02);
    assert!(f.iter().all(|&v| v == WHITE));
    // 帧中途在 mode 2 关掉 LCDC.1：上面的可见，下面的不可见。
    p.write_reg(LCDC, LCDC_8X8 & 0x7F);
    p.write_reg(LCDC, LCDC_8X8);
    run_to_line(&mut p, 40);
    p.write_reg(LCDC, LCDC_8X8 & !0x02);
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |x, y| {
        if inside(x, y, 20, 10, 8, 8) {
            g(3)
        } else {
            WHITE
        }
    });
}

#[test]
fn obj_size_change_in_mode_2_applies_to_that_line() {
    // acid2 在 mode 2 由 LYC 中断切换 LCDC.2：OAM scan 与取 tile 都必须看到新值。
    let mut p = setup();
    tall_tiles(&mut p);
    obj(&mut p, 0, 40, 88, 5, 0x40); // Y 翻转的 8×16，第 88–103 行
    p.write_reg(LCDC, LCDC_8X8 & 0x7F);
    p.write_reg(LCDC, LCDC_8X8);
    run_to_line(&mut p, 88);
    for _ in 0..13 {
        p.tick_mcycle(); // dot 52
    }
    p.write_reg(LCDC, LCDC_8X16);
    run_to_line(&mut p, 104);
    p.write_reg(LCDC, LCDC_8X8);
    run_to_vblank(&mut p);
    assert_frame(p.frame(), |x, y| {
        if inside(x, y, 40, 88, 8, 16) {
            shade_or_white(tall_row(15 - (y - 88)))
        } else {
            WHITE
        }
    });
}

#[test]
fn dma_written_oam_is_used_for_rendering() {
    let mut p = setup();
    put_tile(&mut p, 1, |_, _| 2);
    let bytes = [16 + 70u8, 8 + 33, 1, 0];
    for (i, &b) in bytes.iter().enumerate() {
        p.dma_write_oam(i as u8, b);
    }
    assert_eq!(p.read_oam(0xFE00), 86);
    assert_eq!(p.read_oam(0xFE01), 41);
    let f = draw_frame(&mut p, LCDC_8X8);
    assert_frame(&f, |x, y| {
        if inside(x, y, 33, 70, 8, 8) {
            g(2)
        } else {
            WHITE
        }
    });
}

fn save(p: &Ppu) -> Vec<u8> {
    let mut w = StateWriter::new();
    p.save(&mut w);
    w.into_inner()
}

#[test]
fn snapshot_roundtrip_with_vram_oam_buffers_and_line_state() {
    let mut a = setup();
    tall_tiles(&mut a);
    put_tile(&mut a, 1, asym);
    for i in 0..20u16 {
        a.write_vram(0x9800 + i * 33, 4 + (i % 2) as u8);
    }
    for i in 0..30 {
        obj(
            &mut a,
            i,
            (i as i32 * 37) % 168 - 8,
            (i as i32 * 23) % 150 - 8,
            1 + (i % 5) as u8,
            (i as u8) << 4,
        );
    }
    a.write_reg(0xFF41, 0x48);
    a.write_reg(0xFF45, 0x33);
    a.write_reg(0xFF4A, 25);
    a.write_reg(0xFF4B, 60);
    a.write_reg(LCDC, LCDC_8X8 | 0x20);
    run_to_vblank(&mut a);
    run_to_line(&mut a, 70);
    for _ in 0..40 {
        a.tick_mcycle(); // 第 70 行 mode 3 中（dot 160）
    }
    a.write_reg(BGP, 0x1B); // 让本行已经提交了一部分像素
    let data = save(&a);
    let mut b = Ppu::new();
    b.load(&mut StateReader::new(&data)).unwrap();
    assert_eq!(save(&b), data);
    // 两边做同样的操作，逐 M-cycle 比较中断，两帧后比较画面与完整状态。
    for k in 0..2 * FRAME_T as usize / 4 {
        if k == 500 {
            a.write_reg(0xFF43, 9);
            b.write_reg(0xFF43, 9);
        }
        assert_eq!(a.tick_mcycle(), b.tick_mcycle(), "M-cycle {k}");
    }
    assert_eq!(a.frame()[..], b.frame()[..]);
    assert!(a.frame().iter().any(|&v| v != WHITE));
    assert_eq!(save(&a), save(&b));
}
