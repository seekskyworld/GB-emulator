//! OAM DMA 状态机（core 通道，D022；Pan Docs `OAM_DMA_Transfer.md`）。实际搬运由 Bus 驱动（PLAN §3.2）：
//! Bus 每个 M-cycle 先推进外设、再调用一次 `tick_mcycle`，返回 `Some((src, index))` 时从总线读 `src`
//! （不推进时钟、无副作用），再调用 `Ppu::dma_write_oam(index, v)`；之后才执行 CPU 这个 M-cycle 的访问。
//!
//! 冻结部分（D001）：下列公开方法的签名。
//!
//! 时序（M-cycle W 是 CPU 写 FF46 的那个周期）：
//! - W+1：启动延迟，不搬运；若此前有传输在进行，它在这个周期照常再搬 1 字节；
//! - W+2 … W+161：共 160 个 M-cycle，每个周期搬 1 字节，源 `XX00+i` → OAM 第 i 字节；
//! - `oam_blocked()` 恰好在搬运字节的那些 M-cycle 为真（CPU 在同一周期的 OAM 访问读到 0xFF、写入被忽略）。
//!
//! 这与 Pan Docs 的说明一致：传输占 160 个 M-cycle，"starts right after instruction"；
//! 紧凑例程（`ldh [c],a` 之后 159 个 M-cycle 的等待循环）末尾用 `ret z` 而不是 `ret`，
//! 正是为了让第一次读栈落在 W+162，避开 DMA 的最后一个 M-cycle W+161。
//! 传输进行中再写 FF46：按同样的启动延迟重新开始（从第 0 字节、新的源页）。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};

/// 一次传输的字节数（OAM 大小）。
pub const DMA_LEN: u8 = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dma {
    /// 最近一次写入 FF46 的值（读 FF46 返回它）。
    reg: u8,
    /// 进行中的传输：源页（源地址高字节）。
    page: u8,
    /// 进行中的传输：下一个要搬的字节序号；`DMA_LEN` 表示没有进行中的传输。
    index: u8,
    /// 写 FF46 之后、启动延迟结束之前的新传输源页。
    pending: Option<u8>,
    /// 最近一个 M-cycle 是否搬运了字节（= CPU 在该周期内对 OAM 的访问被封锁）。
    transferring: bool,
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

impl Dma {
    /// 开机状态：FF46 读 0xFF（Power_Up_Sequence 的 DMG 列），没有进行中的传输。
    pub fn new() -> Self {
        Self { reg: 0xFF, page: 0, index: DMA_LEN, pending: None, transferring: false }
    }

    /// 读 FF46：最近一次写入的值。
    pub fn read_reg(&self) -> u8 {
        self.reg
    }

    /// 写 FF46：经过 1 个 M-cycle 的启动延迟后开始（或重新开始）一次 160 字节的传输，源地址为 `v << 8`。
    pub fn write_reg(&mut self, v: u8) {
        self.reg = v;
        self.pending = Some(v);
    }

    /// 推进 1 个 M-cycle；本周期需要搬运一个字节时返回 `Some((源地址, OAM 序号 0..160))`。
    pub fn tick_mcycle(&mut self) -> Option<(u16, u8)> {
        let out = if self.index < DMA_LEN {
            let i = self.index;
            self.index += 1;
            Some((u16::from(self.page) << 8 | u16::from(i), i))
        } else {
            None
        };
        // 启动延迟在这个周期结束：从下一个 M-cycle 起搬新传输的第 0 字节（覆盖进行中的旧传输）。
        if let Some(page) = self.pending.take() {
            self.page = page;
            self.index = 0;
        }
        self.transferring = out.is_some();
        out
    }

    /// 最近一个 M-cycle 是否在搬运字节：此时 CPU 访问 OAM 读到 0xFF、写入被忽略。
    pub fn oam_blocked(&self) -> bool {
        self.transferring
    }
}

impl Snapshot for Dma {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"DMA ");
        w.u8(self.reg);
        w.u8(self.page);
        w.u8(self.index);
        w.bool(self.pending.is_some());
        w.u8(self.pending.unwrap_or(0));
        w.bool(self.transferring);
    }

    /// 全部读完并校验后才写回；失败时状态不变。
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"DMA ")?;
        let reg = r.u8()?;
        let page = r.u8()?;
        let index = r.u8()?;
        let has_pending = r.bool()?;
        let pending_page = r.u8()?;
        let transferring = r.bool()?;
        if index > DMA_LEN {
            return Err(StateError::Corrupt("DMA index"));
        }
        *self = Dma { reg, page, index, pending: has_pending.then_some(pending_page), transferring };
        Ok(())
    }
}
