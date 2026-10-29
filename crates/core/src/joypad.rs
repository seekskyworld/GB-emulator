//! Joypad（sys 通道，T51；PLAN §3.7；Pan Docs Joypad_Input、Interrupt_Sources）。
//!
//! - P1（FF00）：bit5=0 选功能键（A B Select Start），bit4=0 选方向键（Right Left Up Down），低电平有效；
//!   两组都选中时低 4 位是两组的"与"（任一组按下即为 0），都不选时读 0xF。高 2 位读 1。
//! - 只有 bit4–5 可写；低 4 位只读。
//! - 中断：每个 M-cycle 计算一次 P1 低 4 位，任一位出现 1→0 跳变时请求 IF bit4（只有被选中组的按键
//!   能把线拉低，所以只有被选中组的下降沿会触发）。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::{irq, Peripheral};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joypad {
    /// P1 的 bit4–5（0 = 选中）。
    select: u8,
    /// 按键状态，位定义见 `crate::buttons`（1 = 按下）。
    buttons: u8,
    /// 上一个 M-cycle 的 P1 低 4 位，用于检测 1→0 跳变。
    last_lines: u8,
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

impl Joypad {
    /// DMG 开机：P1=CF（两组都选中、无按键）。
    pub fn new() -> Self {
        Self { select: 0x00, buttons: 0, last_lines: 0x0F }
    }

    /// 设置按键状态（`crate::buttons`，1 = 按下）。中断在下一个 M-cycle 的比较中产生。
    pub fn set_buttons(&mut self, mask: u8) {
        self.buttons = mask;
    }

    /// 当前 P1 低 4 位（低电平有效）。
    fn lines(&self) -> u8 {
        let mut pressed = 0;
        if self.select & 0x10 == 0 {
            pressed |= self.buttons >> 4; // Right Left Up Down → bit0..3
        }
        if self.select & 0x20 == 0 {
            pressed |= self.buttons & 0x0F; // A B Select Start → bit0..3
        }
        !pressed & 0x0F
    }
}

impl Peripheral for Joypad {
    fn tick_mcycle(&mut self) -> u8 {
        let lines = self.lines();
        let fell = self.last_lines & !lines;
        self.last_lines = lines;
        if fell != 0 {
            irq::JOYPAD
        } else {
            0
        }
    }

    fn read_reg(&self, _addr: u16) -> u8 {
        0xC0 | self.select | self.lines()
    }

    fn write_reg(&mut self, _addr: u16, v: u8) {
        self.select = v & 0x30;
    }
}

impl Snapshot for Joypad {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"JOYP");
        w.u8(self.select);
        w.u8(self.buttons);
        w.u8(self.last_lines);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"JOYP")?;
        let select = r.u8()?;
        let buttons = r.u8()?;
        let last_lines = r.u8()?;
        if select & !0x30 != 0 || last_lines & !0x0F != 0 {
            return Err(StateError::Corrupt("joypad: P1 state"));
        }
        *self = Self { select, buttons, last_lines };
        Ok(())
    }
}
