//! 串口（sys 通道，T16；PLAN §3.4/§3.7）。内部结构由 sys 自定，对外只要求 `Peripheral` 与 `Snapshot`。
//!
//! T13 临时空壳：SB/SC 为存储，内部时钟传输开始时记录 SB，不计时（T16 替换）。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::Peripheral;

#[derive(Debug, Clone, Default)]
pub struct Serial {
    sb: u8,
    sc: u8,
    out: Vec<u8>,
}

impl Serial {
    /// 开机：SB=00、SC=7E。
    pub fn new() -> Self {
        Self { sb: 0, sc: 0, out: Vec::new() }
    }

    /// 取走已发出的字节。
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }
}

impl Peripheral for Serial {
    fn tick_mcycle(&mut self) -> u8 {
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
    }
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"SERI")?;
        self.sb = r.u8()?;
        self.sc = r.u8()?;
        Ok(())
    }
}
