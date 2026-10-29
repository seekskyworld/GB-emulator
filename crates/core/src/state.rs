//! 存档的小端字节读写与 `Snapshot` trait（T02 冻结并完整实现）。
//!
//! 每个模块的所有者随模块实现 `Snapshot`：`save` 按固定顺序写字段，`load` 按同样顺序读回。
//! 建议每个模块以 `w.tag(b"XXXX")` / `r.tag(b"XXXX")?` 开头，错位时能立即定位到模块。

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// 数据提前结束。
    Truncated,
    /// 文件头 magic 不是 `GBEMUST`。
    BadMagic,
    /// 存档版本与当前实现不符。
    BadVersion(u16),
    /// ROM 指纹（头部校验和、全局校验和、标题）与当前卡带不符。
    RomMismatch,
    /// 模块标签不符或字段取值非法；参数说明位置。
    Corrupt(&'static str),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Truncated => write!(f, "save state truncated"),
            StateError::BadMagic => write!(f, "not a gbemu save state"),
            StateError::BadVersion(v) => write!(f, "unsupported save state version {v}"),
            StateError::RomMismatch => write!(f, "save state belongs to a different ROM"),
            StateError::Corrupt(what) => write!(f, "corrupt save state: {what}"),
        }
    }
}

impl std::error::Error for StateError {}

pub trait Snapshot {
    fn save(&self, w: &mut StateWriter);
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError>;
}

#[derive(Debug, Default)]
pub struct StateWriter {
    buf: Vec<u8>,
}

impl StateWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    /// 4 字节模块标签，用于检测错位。
    pub fn tag(&mut self, t: &[u8; 4]) {
        self.buf.extend_from_slice(t);
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    /// 定长字节（读回时用 `StateReader::bytes_into`，长度由调用方保证一致）。
    pub fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }
    /// 变长字节：u32 长度前缀 + 数据（读回时用 `StateReader::vec`）。
    pub fn vec(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.buf.extend_from_slice(v);
    }
}

#[derive(Debug)]
pub struct StateReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> StateReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], StateError> {
        if self.remaining() < n {
            return Err(StateError::Truncated);
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn tag(&mut self, t: &[u8; 4]) -> Result<(), StateError> {
        if self.take(4)? == t {
            Ok(())
        } else {
            Err(StateError::Corrupt("module tag mismatch"))
        }
    }
    pub fn u8(&mut self) -> Result<u8, StateError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, StateError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StateError::Corrupt("bool")),
        }
    }
    pub fn u16(&mut self) -> Result<u16, StateError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, StateError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, StateError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Result<i64, StateError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn bytes_into(&mut self, out: &mut [u8]) -> Result<(), StateError> {
        out.copy_from_slice(self.take(out.len())?);
        Ok(())
    }
    pub fn vec(&mut self) -> Result<Vec<u8>, StateError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_types() {
        let mut w = StateWriter::new();
        w.tag(b"TEST");
        w.u8(0xAB);
        w.bool(true);
        w.u16(0x1234);
        w.u32(0xDEADBEEF);
        w.u64(0x0123_4567_89AB_CDEF);
        w.i64(-5);
        w.bytes(&[1, 2, 3]);
        w.vec(&[9, 8]);
        let data = w.into_inner();
        let mut r = StateReader::new(&data);
        r.tag(b"TEST").unwrap();
        assert_eq!(r.u8().unwrap(), 0xAB);
        assert!(r.bool().unwrap());
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 0xDEADBEEF);
        assert_eq!(r.u64().unwrap(), 0x0123_4567_89AB_CDEF);
        assert_eq!(r.i64().unwrap(), -5);
        let mut three = [0u8; 3];
        r.bytes_into(&mut three).unwrap();
        assert_eq!(three, [1, 2, 3]);
        assert_eq!(r.vec().unwrap(), vec![9, 8]);
        assert_eq!(r.remaining(), 0);
        assert_eq!(r.u8(), Err(StateError::Truncated));
    }

    #[test]
    fn tag_mismatch_is_error() {
        let mut w = StateWriter::new();
        w.tag(b"AAAA");
        let data = w.into_inner();
        assert!(StateReader::new(&data).tag(b"BBBB").is_err());
    }
}
