//! 串口（sys 通道，T16；PLAN §3.7；Pan Docs Serial_Data_Transfer_(Link_Cable)）。
//!
//! - 写 SC 时若 bit7=1 且 bit0=1（内部时钟），立即把 SB 追加到输出缓冲区（`take_output` 取走），
//!   并开始一次 8 位传输：每 512 T 移出最高位、移入 1（未连接链接线时输入恒为 1），
//!   4096 T 后 SB=0xFF、清 SC.7、请求串口中断（IF bit3）。
//! - 外部时钟（SC.0=0）的传输永不完成（没有对端提供时钟）。
//! - 读 SC 时未使用的位（bit6–1）为 1；读 SB 返回当前移位寄存器。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::{irq, Peripheral};

/// 每移 1 位的 M-cycle 数（8192 Hz：512 T）。
const MCYCLES_PER_BIT: u16 = 512 / 4;
/// 一次完整传输的 M-cycle 数（8 位，4096 T）。
const TRANSFER_MCYCLES: u16 = 8 * MCYCLES_PER_BIT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serial {
    sb: u8,
    /// SC 中存在的位：bit7（传输使能）与 bit0（时钟选择）。
    sc: u8,
    /// 内部时钟传输剩余的 M-cycle 数；0 = 没有进行中的内部时钟传输。
    remaining: u16,
    /// 已发出的字节（不进存档）。
    out: Vec<u8>,
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl Serial {
    /// DMG 开机：SB=00、SC=7E（读出值）。
    pub fn new() -> Self {
        Self { sb: 0x00, sc: 0x00, remaining: 0, out: Vec::new() }
    }

    /// 取走自上次调用以来发出的字节。
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }

    /// 是否有进行中的内部时钟传输。
    pub fn transferring(&self) -> bool {
        self.remaining != 0
    }
}

impl Peripheral for Serial {
    fn tick_mcycle(&mut self) -> u8 {
        if self.remaining == 0 {
            return 0;
        }
        self.remaining -= 1;
        if self.remaining.is_multiple_of(MCYCLES_PER_BIT) {
            self.sb = (self.sb << 1) | 1;
        }
        if self.remaining == 0 {
            self.sc &= 0x7F;
            return irq::SERIAL;
        }
        0
    }

    fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF01 => self.sb,
            0xFF02 => 0x7E | self.sc,
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, addr: u16, v: u8) {
        match addr {
            0xFF01 => self.sb = v,
            0xFF02 => {
                self.sc = v & 0x81;
                if self.sc == 0x81 {
                    self.out.push(self.sb);
                    self.remaining = TRANSFER_MCYCLES;
                } else {
                    // 清 bit7 中止传输；外部时钟传输等待永远不会到来的时钟。
                    self.remaining = 0;
                }
            }
            _ => {}
        }
    }
}

impl Snapshot for Serial {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"SERI");
        w.u8(self.sb);
        w.u8(self.sc);
        w.u16(self.remaining);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"SERI")?;
        let sb = r.u8()?;
        let sc = r.u8()?;
        let remaining = r.u16()?;
        if sc & !0x81 != 0 || remaining > TRANSFER_MCYCLES || (remaining != 0 && sc != 0x81) {
            return Err(StateError::Corrupt("serial: SC/transfer state"));
        }
        self.sb = sb;
        self.sc = sc;
        self.remaining = remaining;
        Ok(())
    }
}
