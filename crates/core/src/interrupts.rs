//! IF/IE 寄存器（sys 通道，T14 的寄存器侧；Pan Docs Interrupts）。
//!
//! - IF（FF0F）只有低 5 位存在，读出时高 3 位为 1（PLAN §3.3）。
//! - IE（FFFF）8 位全部可读写（开机为 0x00）。
//! - 外设在每个 M-cycle 返回的中断位由 Bus 经 `request` 或进 IF；CPU 分派时经 `ack` 清除对应位。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};

/// IF 中实际存在的位（VBlank、STAT、Timer、Serial、Joypad）。
const IF_MASK: u8 = 0x1F;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interrupts {
    /// IF 的低 5 位。
    flags: u8,
    /// IE 的全部 8 位。
    enable: u8,
}

impl Interrupts {
    /// DMG 开机状态（Power_Up_Sequence）：IF = 0xE1（VBlank 已请求），IE = 0x00。
    pub fn new() -> Self {
        Self { flags: 0x01, enable: 0x00 }
    }

    /// 读 FF0F：高 3 位恒为 1。
    pub fn read_if(&self) -> u8 {
        0xE0 | self.flags
    }

    /// 写 FF0F：只保留低 5 位。
    pub fn write_if(&mut self, v: u8) {
        self.flags = v & IF_MASK;
    }

    /// 读 FFFF。
    pub fn read_ie(&self) -> u8 {
        self.enable
    }

    /// 写 FFFF。
    pub fn write_ie(&mut self, v: u8) {
        self.enable = v;
    }

    /// 把外设新请求的中断位（`crate::irq` 掩码）或进 IF。
    pub fn request(&mut self, mask: u8) {
        self.flags |= mask & IF_MASK;
    }

    /// `IE & IF & 0x1F`。
    pub fn pending(&self) -> u8 {
        self.enable & self.flags & IF_MASK
    }

    /// 清除 IF 的第 `bit` 位（序号 0..=4）；超出范围的序号被忽略。
    pub fn ack(&mut self, bit: u8) {
        if bit < 5 {
            self.flags &= !(1 << bit);
        }
    }
}

impl Snapshot for Interrupts {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"INTR");
        w.u8(self.flags);
        w.u8(self.enable);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"INTR")?;
        let flags = r.u8()?;
        let enable = r.u8()?;
        if flags & !IF_MASK != 0 {
            return Err(StateError::Corrupt("interrupts: IF has bits above 4"));
        }
        self.flags = flags;
        self.enable = enable;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_up_values() {
        let i = Interrupts::new();
        assert_eq!(i.read_if(), 0xE1);
        assert_eq!(i.read_ie(), 0x00);
        assert_eq!(i.pending(), 0);
    }

    #[test]
    fn if_upper_bits_read_as_one() {
        let mut i = Interrupts::new();
        i.write_if(0x00);
        assert_eq!(i.read_if(), 0xE0);
        i.write_if(0xFF);
        assert_eq!(i.read_if(), 0xFF);
        i.write_if(0x0A);
        assert_eq!(i.read_if(), 0xEA);
    }

    #[test]
    fn pending_request_ack() {
        let mut i = Interrupts::new();
        i.write_if(0);
        i.write_ie(0xFF);
        i.request(0x04 | 0x10);
        assert_eq!(i.pending(), 0x14);
        i.ack(2);
        assert_eq!(i.pending(), 0x10);
        assert_eq!(i.read_if(), 0xF0);
        i.write_ie(0x01);
        assert_eq!(i.pending(), 0);
        i.ack(7); // 越界序号无效
        assert_eq!(i.read_if(), 0xF0);
    }

    #[test]
    fn snapshot_roundtrip() {
        let mut a = Interrupts::new();
        a.write_if(0x15);
        a.write_ie(0xE3);
        let mut w = StateWriter::new();
        a.save(&mut w);
        let data = w.into_inner();
        let mut b = Interrupts::new();
        b.load(&mut StateReader::new(&data)).unwrap();
        assert_eq!(a, b);
    }
}
