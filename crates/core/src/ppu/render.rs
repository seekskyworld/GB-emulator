//! 扫描线渲染（T41 BG/Window，T42 精灵）。
//!
//! `render_span(x0, x1)` 用此刻的寄存器、VRAM、OAM 把当前行的像素 `x0..x1` 画到后缓冲；
//! 何时调用见 `ppu.rs` 顶部的"渲染时机"。

use super::{Ppu, MAX_LINE_OBJS};
use crate::SCREEN_W;

/// 调色板值 0..=3 对应的灰度（色号 0 = 白 = 0xFF）。
const SHADES: [u8; 4] = [0xFF, 0xAA, 0x55, 0x00];

/// 精灵像素缓冲的编码：bit0–1 色号（0 = 无精灵像素），bit2 = OBP1，bit3 = OBJ-to-BG priority。
const OBJ_PAL1: u8 = 0x04;
const OBJ_BEHIND_BG: u8 = 0x08;

/// 用调色板寄存器把色号 0..=3 映射为灰度。
fn shade(palette: u8, color: u8) -> u8 {
    SHADES[((palette >> (color * 2)) & 3) as usize]
}

impl Ppu {
    /// OAM scan：按 OAM 顺序选出 Y 命中当前行的前 10 个精灵（不论 X 是否可见，也不论 LCDC.1），
    /// 再按 DMG 绘制优先级排序：X 小者在前，X 相同时 OAM 序号小者在前（`OAM.md`）。
    pub(super) fn oam_scan(&mut self) {
        let height = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let y = self.line as u16 + 16; // OAM 的 Y = 屏幕 y + 16
        let mut n = 0;
        for i in 0..40 {
            let top = self.oam[i * 4] as u16;
            if y >= top && y < top + height {
                self.objs[n] = i as u8;
                n += 1;
                if n == MAX_LINE_OBJS {
                    break;
                }
            }
        }
        // 稳定插入排序：X 相同的保持 OAM 顺序。
        for i in 1..n {
            let mut j = i;
            while j > 0 && self.obj_x(self.objs[j - 1]) > self.obj_x(self.objs[j]) {
                self.objs.swap(j - 1, j);
                j -= 1;
            }
        }
        self.obj_count = n as u8;
    }

    fn obj_x(&self, index: u8) -> u8 {
        self.oam[index as usize * 4 + 1]
    }

    /// 本行窗口开始的屏幕 x：LCDC.0=1、LCDC.5=1、本帧 Y 条件已满足、WX 在 0..=166 时为 `WX-7`
    /// （WX<7 时为 0），窗口从那里覆盖到行尾。
    pub(super) fn window_start(&self) -> Option<usize> {
        if self.lcdc & 0x21 == 0x21 && self.win_y_hit && self.wx <= 166 {
            Some((self.wx as usize).saturating_sub(7))
        } else {
            None
        }
    }

    /// mode 3 比最短的 172 dot 多出的长度（`Rendering.md` "Mode 3 length"）：
    /// SCX%8；本行出现窗口时 6；每个被绘制的精灵（X<168，含 X=0）6 dot，再加上它最左像素所在的
    /// BG/窗口 tile 若未被前面的精灵计过，则加"该 tile 中在它右边的像素数 − 2"（不小于 0；X=0 固定为 5）。
    /// 精灵按从左到右的顺序考虑（即 OAM scan 排好的绘制优先级顺序）。
    pub(super) fn mode3_penalty(&self) -> u16 {
        let fine = (self.scx & 7) as i32;
        let win = self.window_start();
        let mut extra = fine + if win.is_some() { 6 } else { 0 };
        if self.lcdc & 0x02 != 0 {
            // 已计过的 tile：bit 0..=21 为 BG 列，bit 32.. 为窗口列。
            let mut seen: u64 = 0;
            for &index in &self.objs[..self.obj_count as usize] {
                let x = self.obj_x(index) as i32;
                if x >= 168 {
                    continue;
                }
                let px = x - 8; // 最左像素的屏幕 x（可为负）
                let (key, right) = match win {
                    Some(ws) if px >= ws as i32 => {
                        let o = px - ws as i32;
                        (32 + o / 8, 7 - o % 8)
                    }
                    _ => {
                        let m = px + 8 + fine; // BG tile 边界在屏幕 x ≡ -SCX (mod 8)
                        (m / 8, 7 - m % 8)
                    }
                };
                if seen & (1u64 << key) == 0 {
                    seen |= 1u64 << key;
                    extra += if x == 0 { 5 } else { (right - 2).max(0) };
                }
                extra += 6;
            }
        }
        // mode 3 最长 289 dot。
        extra.min(289 - 172) as u16
    }

    /// 把当前行的像素 `x0..x1` 画到后缓冲。
    pub(super) fn render_span(&mut self, x0: usize, x1: usize) {
        let mut bg = [0u8; SCREEN_W];
        self.bg_window_span(&mut bg, x0, x1);
        let mut obj = [0u8; SCREEN_W];
        if self.lcdc & 0x02 != 0 {
            self.obj_span(&mut obj, x0, x1);
        }
        let row = self.line as usize * SCREEN_W;
        let (bgp, obp0, obp1) = (self.bgp, self.obp0, self.obp1);
        let dst = &mut self.back[row + x0..row + x1];
        for ((d, &c), &o) in dst.iter_mut().zip(&bg[x0..x1]).zip(&obj[x0..x1]) {
            // 精灵像素只在 OBJ-to-BG priority 置位且 BG/窗口色号为 1–3 时退到后面。
            *d = if o != 0 && (o & OBJ_BEHIND_BG == 0 || c == 0) {
                shade(if o & OBJ_PAL1 != 0 { obp1 } else { obp0 }, o & 3)
            } else {
                shade(bgp, c)
            };
        }
    }

    /// 本行选中的精灵在 `x0..x1` 内的像素：按绘制优先级依次填入，每个位置只取优先级最高的
    /// 不透明像素（之后才与 BG 比较 priority，`OAM.md` "Drawing priority"）。
    fn obj_span(&self, out: &mut [u8; SCREEN_W], x0: usize, x1: usize) {
        let tall = self.lcdc & 0x04 != 0;
        let height: i32 = if tall { 16 } else { 8 };
        for &index in &self.objs[..self.obj_count as usize] {
            let i = index as usize * 4;
            let (top, ox, tile, attr) = (
                self.oam[i],
                self.oam[i + 1],
                self.oam[i + 2],
                self.oam[i + 3],
            );
            let mut row = self.line as i32 + 16 - top as i32;
            if !(0..height).contains(&row) {
                continue; // mode 3 中途改了 LCDC.2
            }
            if attr & 0x40 != 0 {
                row = height - 1 - row; // Y 翻转作用于整个 8×16
            }
            // 8×16 忽略 tile 号 bit0：上半为 NN&$FE，下半为 NN|$01（row 8–15 自然落到下一个 tile）。
            let tile = if tall { tile & 0xFE } else { tile } as usize;
            let addr = tile * 16 + row as usize * 2;
            let (lo, hi) = (self.vram[addr], self.vram[addr + 1]);
            let left = ox as i32 - 8;
            let flags = ((attr >> 2) & OBJ_PAL1) | ((attr >> 4) & OBJ_BEHIND_BG);
            let sx0 = left.max(x0 as i32);
            let sx1 = (left + 8).min(x1 as i32);
            for sx in sx0..sx1 {
                let slot = &mut out[sx as usize];
                if *slot != 0 {
                    continue;
                }
                let col = sx - left;
                let bit = if attr & 0x20 != 0 { col } else { 7 - col };
                let c = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
                if c != 0 {
                    *slot = c | flags;
                }
            }
        }
    }

    /// BG 与窗口的色号（LCDC.0=0 时全为 0，窗口也不画）。
    fn bg_window_span(&mut self, out: &mut [u8; SCREEN_W], x0: usize, x1: usize) {
        let lcdc = self.lcdc;
        if lcdc & 0x01 == 0 {
            return;
        }
        let win_start = self.window_start().unwrap_or(SCREEN_W);
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
