//! OAM DMA 状态（ppu 通道，T42）。实际搬运由 Bus 驱动（PLAN §3.2）：
//! Bus 每个 M-cycle 调用一次 `tick_mcycle`，返回 `Some((src, index))` 时从总线读 `src`
//! （不推进时钟、无副作用），再调用 `Ppu::dma_write_oam(index, v)`。
//!
//! 冻结部分（D001）：下列公开方法的签名。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};

#[derive(Debug, Clone, Default)]
pub struct Dma {
    /// 最近一次写入 FF46 的值（读 FF46 返回它）。
    reg: u8,
}

impl Dma {
    pub fn new() -> Self {
        Self { reg: 0xFF }
    }

    /// 读 FF46。
    pub fn read_reg(&self) -> u8 {
        self.reg
    }

    /// 写 FF46：启动（或重启）一次 160 字节的传输，源地址为 `v << 8`。
    pub fn write_reg(&mut self, v: u8) {
        // T02 空壳：T42 实现启动延迟、进度与重启。
        self.reg = v;
    }

    /// 推进 1 个 M-cycle；本周期需要搬运一个字节时返回 `Some((源地址, OAM 序号 0..160))`。
    pub fn tick_mcycle(&mut self) -> Option<(u16, u8)> {
        None
    }

    /// 传输进行中（CPU 访问 OAM 读到 0xFF、写入被忽略）。
    pub fn oam_blocked(&self) -> bool {
        false
    }
}

impl Snapshot for Dma {
    fn save(&self, _w: &mut StateWriter) {
        todo!("ppu: Snapshot for Dma")
    }
    fn load(&mut self, _r: &mut StateReader) -> Result<(), StateError> {
        todo!("ppu: Snapshot for Dma")
    }
}
