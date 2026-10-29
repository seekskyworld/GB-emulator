//! Joypad（sys 通道，T51；PLAN §3.4/§3.7）。内部结构由 sys 自定，对外只要求 `Peripheral` 与 `Snapshot`。
//!
//! T13 临时空壳：P1 读出"无按键"，不产生中断（T51 替换）。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::Peripheral;

#[derive(Debug, Clone, Default)]
pub struct Joypad {
    /// P1 的 bit4–5。
    select: u8,
}

impl Joypad {
    /// 开机：P1=CF。
    pub fn new() -> Self {
        Self { select: 0 }
    }

    /// 设置按键状态（`crate::buttons`，1 = 按下）。
    pub fn set_buttons(&mut self, _mask: u8) {}
}

impl Peripheral for Joypad {
    fn tick_mcycle(&mut self) -> u8 {
        0
    }
    fn read_reg(&self, _addr: u16) -> u8 {
        0xC0 | self.select | 0x0F
    }
    fn write_reg(&mut self, _addr: u16, v: u8) {
        self.select = v & 0x30;
    }
}

impl Snapshot for Joypad {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"JOYP");
        w.u8(self.select);
    }
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"JOYP")?;
        self.select = r.u8()? & 0x30;
        Ok(())
    }
}
