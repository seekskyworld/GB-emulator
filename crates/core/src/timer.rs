//! 定时器（sys 通道，T15；PLAN §3.4/§3.7）。内部结构由 sys 自定，对外只要求 `Peripheral` 与 `Snapshot`。
//!
//! T13 临时空壳：寄存器为可读写存储、不计时（T15 替换）。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::Peripheral;

#[derive(Debug, Clone)]
pub struct Timer {
    /// FF04–FF07 的存储。
    regs: [u8; 4],
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    /// 开机：DIV=AB、TIMA=00、TMA=00、TAC=F8。
    pub fn new() -> Self {
        Self { regs: [0xAB, 0x00, 0x00, 0x00] }
    }

    /// 写 DIV 或执行 STOP：清零分频计数器。
    pub fn reset_div(&mut self) {
        self.regs[0] = 0;
    }
}

impl Peripheral for Timer {
    fn tick_mcycle(&mut self) -> u8 {
        0
    }
    fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF04..=0xFF06 => self.regs[(addr - 0xFF04) as usize],
            0xFF07 => 0xF8 | self.regs[3],
            _ => 0xFF,
        }
    }
    fn write_reg(&mut self, addr: u16, v: u8) {
        match addr {
            0xFF04 => self.reset_div(),
            0xFF05..=0xFF06 => self.regs[(addr - 0xFF04) as usize] = v,
            0xFF07 => self.regs[3] = v & 0x07,
            _ => {}
        }
    }
}

impl Snapshot for Timer {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"TIMR");
        w.bytes(&self.regs);
    }
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"TIMR")?;
        r.bytes_into(&mut self.regs)
    }
}
