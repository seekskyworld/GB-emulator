//! 扫描线渲染（T41 BG/Window，T42 精灵）。T40 阶段只占位：不选精灵、不画像素。

use super::Ppu;

impl Ppu {
    /// OAM scan：T42 实现。
    pub(super) fn oam_scan(&mut self) {
        self.obj_count = 0;
    }

    /// 把当前行的像素 `x0..x1` 画到后缓冲：T41 实现。
    pub(super) fn render_span(&mut self, _x0: usize, _x1: usize) {}
}
