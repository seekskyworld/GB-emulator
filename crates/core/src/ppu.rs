//! PPU（ppu 通道，T40–T42）：逐扫描线渲染加双缓冲（PLAN §3.5）。
//!
//! 冻结部分（D001）：`Ppu::new`、VRAM/OAM 访问方法、`dma_write_oam`、`frame`、`Peripheral` 实现
//! （`tick_mcycle` 返回 VBlank/STAT 中断位；`read_reg`/`write_reg` 处理 FF40–FF45、FF47–FF4B；
//! FF46 归 `dma.rs`）、`Snapshot`。VRAM 与 OAM 数组由本模块持有，Bus 只做转发。
//!
//! T02 空壳：存储与访问已可用，`tick_mcycle` 不推进时序（返回 0），由 T40 替换。
//! 在此之前整机仍可运行（Blargg 的控制台在 LY 不变时会超时后继续）。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::{Frame, Peripheral, FRAME_PIXELS};

pub struct Ppu {
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    /// 最近一个完整帧（进入 LY=144 时由后缓冲交换而来）。
    front: Box<Frame>,
    /// 正在绘制的帧。
    back: Box<Frame>,
    /// FF40–FF4B 的寄存器存储（T02 空壳用；T40 起可替换为具名字段）。
    regs: [u8; 12],
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    /// DMG 开机状态（Pan Docs Power_Up_Sequence 的硬件寄存器表，DMG 列），前缓冲为全白。
    pub fn new() -> Self {
        let mut regs = [0u8; 12];
        regs[0x0] = 0x91; // LCDC
        regs[0x1] = 0x85; // STAT
        regs[0x7] = 0xFC; // BGP
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            front: Box::new([0xFF; FRAME_PIXELS]),
            back: Box::new([0xFF; FRAME_PIXELS]),
            regs,
        }
    }

    /// CPU 读 VRAM，`addr` 为绝对地址 0x8000..=0x9FFF。无副作用（`peek` 也走这里）。
    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[(addr & 0x1FFF) as usize]
    }

    /// CPU 写 VRAM，`addr` 为绝对地址 0x8000..=0x9FFF。
    pub fn write_vram(&mut self, addr: u16, v: u8) {
        self.vram[(addr & 0x1FFF) as usize] = v;
    }

    /// CPU 读 OAM，`addr` 为绝对地址 0xFE00..=0xFE9F。无副作用。
    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr as usize - 0xFE00) % 0xA0]
    }

    /// CPU 写 OAM，`addr` 为绝对地址 0xFE00..=0xFE9F。
    pub fn write_oam(&mut self, addr: u16, v: u8) {
        self.oam[(addr as usize - 0xFE00) % 0xA0] = v;
    }

    /// OAM DMA 写入第 `index`（0..160）个字节；由 Bus 驱动，不受 PPU 模式封锁影响。
    pub fn dma_write_oam(&mut self, index: u8, v: u8) {
        self.oam[index as usize % 0xA0] = v;
    }

    /// 最近一个完整帧（前缓冲），值为 0xFF/0xAA/0x55/0x00。
    pub fn frame(&self) -> &Frame {
        &self.front
    }
}

impl Peripheral for Ppu {
    fn tick_mcycle(&mut self) -> u8 {
        // T02 空壳：T40 实现 dot 计数、LY、mode、STAT、VBlank 与双缓冲交换。
        let _ = &mut self.back;
        0
    }

    fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF40..=0xFF4B => self.regs[(addr - 0xFF40) as usize],
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, addr: u16, v: u8) {
        if let 0xFF40..=0xFF4B = addr {
            self.regs[(addr - 0xFF40) as usize] = v;
        }
    }
}

impl Snapshot for Ppu {
    fn save(&self, _w: &mut StateWriter) {
        todo!("ppu: Snapshot for Ppu")
    }
    fn load(&mut self, _r: &mut StateReader) -> Result<(), StateError> {
        todo!("ppu: Snapshot for Ppu")
    }
}
