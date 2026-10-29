//! IF/IE 寄存器（sys 通道，T14）。IF 读出时高 3 位为 1（PLAN §3.3）。内部结构由 sys 自定。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};

#[derive(Debug, Clone, Default)]
pub struct Interrupts {}

impl Interrupts {
    pub fn new() -> Self {
        Self {}
    }
}

impl Snapshot for Interrupts {
    fn save(&self, _w: &mut StateWriter) {
        todo!("sys: Snapshot for Interrupts")
    }
    fn load(&mut self, _r: &mut StateReader) -> Result<(), StateError> {
        todo!("sys: Snapshot for Interrupts")
    }
}
