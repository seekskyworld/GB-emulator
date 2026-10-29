//! 扫描线渲染（T41 BG/Window，T42 精灵）。
//!
//! `render_span(x0, x1)` 用此刻的寄存器、VRAM、OAM 把当前行的像素 `x0..x1` 画到后缓冲；
//! 何时调用见 `ppu.rs` 顶部的"渲染时机"。

use super::Ppu;
use crate::SCREEN_W;

/// 调色板值 0..=3 对应的灰度（色号 0 = 白 = 0xFF）。
const SHADES: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];

/// 用调色板寄存器把色号 0..=3 映射为灰度。
fn shade(palette: u8, color: u8) -> u8 {
    SHADES[((palette >> (color * 2)) & 3) as usize]
}

impl Ppu {
    /// OAM scan：T42 实现。
    pub(super) fn oam_scan(&mut self) {
        self.obj_count = 0;
    }

    /// 把当前行的像素 `x0..x1` 画到后缓冲。
    pub(super) fn render_span(&mut self, x0: usize, x1: usize) {
        let mut bg = [0u8; SCREEN_W];
        self.bg_window_span(&mut bg, x0, x1);
        let row = self.line as usize * SCREEN_W;
        let bgp = self.bgp;
        for (dst, &c) in self.back[row + x0..row + x1].iter_mut().zip(&bg[x0..x1]) {
            *dst = shade(bgp, c);
        }
    }

    /// BG 与窗口的色号（LCDC.0=0 时全为 0，窗口也不画）。
    fn bg_window_span(&mut self, out: &mut [u8; SCREEN_W], x0: usize, x1: usize) {
        let lcdc = self.lcdc;
        if lcdc & 0x01 == 0 {
            return;
        }
        // 窗口：LCDC.5=1、本帧 Y 条件已满足、WX 在 0..=166 时，从 x = WX-7 开始覆盖到行尾。
        let win_start = if lcdc & 0x20 != 0 && self.win_y_hit && self.wx <= 166 {
            (self.wx as usize).saturating_sub(7)
        } else {
            SCREEN_W
        };
        let bg_end = x1.min(win_start.max(x0));
        if x0 < bg_end {
            let map = if lcdc & 0x08 != 0 { 0x1C00 } else { 0x1800 };
            let y = self.line.wrapping_add(self.scy);
            self.fetch_row(out, x0, bg_end, map, y, self.scx as usize);
        }
        let win_x0 = x0.max(win_start);
        if win_x0 < x1 {
            let map = if lcdc & 0x40 != 0 { 0x1C00 } else { 0x1800 };
            // 窗口第 0 列在屏幕 x = WX-7：列号 = x + 7 - WX（WX ≤ 166，不会越过 255）。
            let xoff = (7 + 256 - self.wx as usize) & 0xFF;
            self.fetch_row(out, win_x0, x1, map, self.win_line, xoff);
            self.win_drawn = true;
        }
    }

    /// 从 tilemap `map`（VRAM 偏移 0x1800/0x1C00）取第 `y` 个像素行，屏幕 x 对应 map 列 `(x + xoff) & 255`，
    /// 把色号写入 `out[xa..xb]`。tile data 寻址由 LCDC.4 决定。
    fn fetch_row(
        &self,
        out: &mut [u8; SCREEN_W],
        xa: usize,
        xb: usize,
        map: usize,
        y: u8,
        xoff: usize,
    ) {
        let map_row = map + (y as usize / 8) * 32;
        let fine_y = (y as usize & 7) * 2;
        let unsigned = self.lcdc & 0x10 != 0;
        let mut x = xa;
        while x < xb {
            let mx = (x + xoff) & 0xFF;
            let tile = self.vram[map_row + mx / 8];
            let base = if unsigned {
                tile as usize * 16
            } else {
                // "$8800 方式"：以 0x9000 为基址的有符号序号。
                (0x1000 + (tile as i8 as isize) * 16) as usize
            };
            let lo = self.vram[base + fine_y];
            let hi = self.vram[base + fine_y + 1];
            let mut bit = 7 - (mx & 7);
            loop {
                out[x] = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
                x += 1;
                if bit == 0 || x >= xb {
                    break;
                }
                bit -= 1;
            }
        }
    }
}
