//! 串口（sys 通道，T16；PLAN §3.7；Pan Docs Serial_Data_Transfer_(Link_Cable)、Timer_Obscure_Behaviour）。
//!
//! - 写 SC 时若 bit7=1 且 bit0=1（内部时钟），立即把 SB 追加到输出缓冲区（`take_output` 取走），
//!   并开始一次 8 位传输。
//! - 内部时钟为 8192 Hz（周期 512 T），取自系统计数器（DIV 所在的 16 位计数器，见 `timer.rs`）的 bit 8：
//!   该位每出现一次下降沿，移出 SB 的最高位、移入 1（未连接链接线时输入恒为 1）。第 8 次移位后
//!   SB=0xFF、清 SC.7、请求串口中断（IF bit3）。因此一次传输历时 7×512 T 加上"开始时刻到下一个
//!   下降沿"的距离（3588–4096 T），与 DIV 的相位对齐；写 DIV 清零计数器时若 bit 8 为 1，同样算一次下降沿。
//! - 外部时钟（SC.0=0）的传输永不完成（没有对端提供时钟）。
//! - 读 SC 时未使用的位（bit6–1）为 1；读 SB 返回当前移位寄存器。
//!
//! 系统计数器由 Bus 在每个 M-cycle 推进定时器之后经 `set_system_counter` 交给本模块，
//! 本模块在 `tick_mcycle` 中与上一个 M-cycle 的值比较。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::timer::BOOT_COUNTER;
use crate::{irq, Peripheral};

/// 驱动串口时钟的系统计数器位（8192 Hz）。
const CLOCK_BIT: u16 = 1 << 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serial {
    sb: u8,
    /// SC 中存在的位：bit7（传输使能）与 bit0（时钟选择）。
    sc: u8,
    /// 内部时钟传输还要移的位数；0 = 没有进行中的内部时钟传输。
    bits_left: u8,
    /// 本 M-cycle 的系统计数器值（Bus 提供）。
    counter: u16,
    /// 上一个 M-cycle 的系统计数器值，用于检测 bit 8 的下降沿。
    last_counter: u16,
    /// 已发出的字节（不进存档）。
    out: Vec<u8>,
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl Serial {
    /// DMG 开机：SB=00、SC=7E（读出值）；系统计数器取开机值。
    pub fn new() -> Self {
        Self {
            sb: 0x00,
            sc: 0x00,
            bits_left: 0,
            counter: BOOT_COUNTER,
            last_counter: BOOT_COUNTER,
            out: Vec::new(),
        }
    }

    /// 取走自上次调用以来发出的字节。
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }

    /// 是否有进行中的内部时钟传输。
    pub fn transferring(&self) -> bool {
        self.bits_left != 0
    }

    /// 提供本 M-cycle 的系统计数器值（Bus 在推进定时器之后、调用 `tick_mcycle` 之前调用）。
    pub fn set_system_counter(&mut self, counter: u16) {
        self.counter = counter;
    }
}

impl Peripheral for Serial {
    fn tick_mcycle(&mut self) -> u8 {
        let fell = self.last_counter & CLOCK_BIT != 0 && self.counter & CLOCK_BIT == 0;
        self.last_counter = self.counter;
        if self.bits_left == 0 || !fell {
            return 0;
        }
        self.sb = (self.sb << 1) | 1;
        self.bits_left -= 1;
        if self.bits_left == 0 {
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
                    self.bits_left = 8;
                } else {
                    // 清 bit7 中止传输；外部时钟传输等待永远不会到来的时钟。
                    self.bits_left = 0;
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
        w.u8(self.bits_left);
        w.u16(self.counter);
        w.u16(self.last_counter);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"SERI")?;
        let sb = r.u8()?;
        let sc = r.u8()?;
        let bits_left = r.u8()?;
        let counter = r.u16()?;
        let last_counter = r.u16()?;
        if sc & !0x81 != 0 || bits_left > 8 || (bits_left != 0 && sc != 0x81) {
            return Err(StateError::Corrupt("serial: SC/transfer state"));
        }
        self.sb = sb;
        self.sc = sc;
        self.bits_left = bits_left;
        self.counter = counter;
        self.last_counter = last_counter;
        Ok(())
    }
}
