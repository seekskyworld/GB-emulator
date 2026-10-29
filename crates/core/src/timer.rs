//! 定时器（sys 通道，T15；PLAN §3.4/§3.7）。内部结构由 sys 自定，对外只要求 `Peripheral` 与 `Snapshot`。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::Peripheral;

#[derive(Debug, Clone, Default)]
pub struct Timer {}

impl Timer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Peripheral for Timer {
    fn tick_mcycle(&mut self) -> u8 {
        todo!("sys: Timer::tick_mcycle (T15)")
    }
    fn read_reg(&self, _addr: u16) -> u8 {
        todo!("sys: Timer::read_reg (T15) — FF04–FF07")
    }
    fn write_reg(&mut self, _addr: u16, _v: u8) {
        todo!("sys: Timer::write_reg (T15)")
    }
}

impl Snapshot for Timer {
    fn save(&self, _w: &mut StateWriter) {
        todo!("sys: Snapshot for Timer")
    }
    fn load(&mut self, _r: &mut StateReader) -> Result<(), StateError> {
        todo!("sys: Snapshot for Timer")
    }
}
