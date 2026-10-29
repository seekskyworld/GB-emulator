//! CPU 测试共用的支撑代码（core 通道）：最小 JSON 解析器与平铺 64 KiB RAM 的假总线。
//!
//! 其他测试文件用 `#[path = "cpu_common.rs"] mod common;` 引入；本文件单独编译时只跑自身的小测试。
#![allow(dead_code)]

use gb_core::CpuBus;

/// 最小 JSON 解析器（核心零依赖，测试也不引入第三方 crate）。
pub mod json {
    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Json>),
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        /// 取字段，缺失时 panic 并给出字段名。
        pub fn field(&self, key: &str) -> &Json {
            self.get(key).unwrap_or_else(|| panic!("missing JSON field {key:?}"))
        }
        pub fn as_arr(&self) -> &[Json] {
            match self {
                Json::Arr(v) => v,
                other => panic!("expected JSON array, got {other:?}"),
            }
        }
        pub fn as_obj(&self) -> &[(String, Json)] {
            match self {
                Json::Obj(v) => v,
                other => panic!("expected JSON object, got {other:?}"),
            }
        }
        pub fn as_str(&self) -> &str {
            match self {
                Json::Str(s) => s,
                other => panic!("expected JSON string, got {other:?}"),
            }
        }
        pub fn as_u64(&self) -> u64 {
            match self {
                Json::Num(n) if *n >= 0.0 && n.fract() == 0.0 => *n as u64,
                other => panic!("expected non-negative integer, got {other:?}"),
            }
        }
        pub fn as_bool(&self) -> bool {
            match self {
                Json::Bool(b) => *b,
                other => panic!("expected JSON bool, got {other:?}"),
            }
        }
    }

    pub fn parse(s: &str) -> Result<Json, String> {
        let mut p = Parser { b: s.as_bytes(), i: 0 };
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(p.err("trailing characters"));
        }
        Ok(v)
    }

    struct Parser<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Parser<'_> {
        fn err(&self, msg: &str) -> String {
            format!("JSON error at byte {}: {msg}", self.i)
        }
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
                self.i += 1;
            }
        }
        fn peek(&self) -> Option<u8> {
            self.b.get(self.i).copied()
        }
        fn expect(&mut self, c: u8) -> Result<(), String> {
            if self.peek() == Some(c) {
                self.i += 1;
                Ok(())
            } else {
                Err(self.err(&format!("expected {:?}", c as char)))
            }
        }
        fn lit(&mut self, s: &str, v: Json) -> Result<Json, String> {
            if self.b[self.i..].starts_with(s.as_bytes()) {
                self.i += s.len();
                Ok(v)
            } else {
                Err(self.err("bad literal"))
            }
        }
        fn value(&mut self) -> Result<Json, String> {
            self.ws();
            match self.peek() {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => Ok(Json::Str(self.string()?)),
                Some(b't') => self.lit("true", Json::Bool(true)),
                Some(b'f') => self.lit("false", Json::Bool(false)),
                Some(b'n') => self.lit("null", Json::Null),
                Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
                _ => Err(self.err("unexpected character")),
            }
        }
        fn object(&mut self) -> Result<Json, String> {
            self.expect(b'{')?;
            let mut kv = Vec::new();
            self.ws();
            if self.peek() == Some(b'}') {
                self.i += 1;
                return Ok(Json::Obj(kv));
            }
            loop {
                self.ws();
                let k = self.string()?;
                self.ws();
                self.expect(b':')?;
                let v = self.value()?;
                kv.push((k, v));
                self.ws();
                match self.peek() {
                    Some(b',') => self.i += 1,
                    Some(b'}') => {
                        self.i += 1;
                        return Ok(Json::Obj(kv));
                    }
                    _ => return Err(self.err("expected ',' or '}'")),
                }
            }
        }
        fn array(&mut self) -> Result<Json, String> {
            self.expect(b'[')?;
            let mut v = Vec::new();
            self.ws();
            if self.peek() == Some(b']') {
                self.i += 1;
                return Ok(Json::Arr(v));
            }
            loop {
                v.push(self.value()?);
                self.ws();
                match self.peek() {
                    Some(b',') => self.i += 1,
                    Some(b']') => {
                        self.i += 1;
                        return Ok(Json::Arr(v));
                    }
                    _ => return Err(self.err("expected ',' or ']'")),
                }
            }
        }
        fn string(&mut self) -> Result<String, String> {
            self.expect(b'"')?;
            let mut out = String::new();
            loop {
                let start = self.i;
                while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\' {
                    self.i += 1;
                }
                out.push_str(std::str::from_utf8(&self.b[start..self.i]).map_err(|_| self.err("bad utf-8"))?);
                match self.peek() {
                    Some(b'"') => {
                        self.i += 1;
                        return Ok(out);
                    }
                    Some(b'\\') => {
                        self.i += 1;
                        let c = self.peek().ok_or_else(|| self.err("eof in escape"))?;
                        self.i += 1;
                        match c {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{8}'),
                            b'f' => out.push('\u{c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let hex = self.b.get(self.i..self.i + 4).ok_or_else(|| self.err("short \\u"))?;
                                let s = std::str::from_utf8(hex).map_err(|_| self.err("bad \\u"))?;
                                let cp = u32::from_str_radix(s, 16).map_err(|_| self.err("bad \\u"))?;
                                self.i += 4;
                                out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                            }
                            _ => return Err(self.err("bad escape")),
                        }
                    }
                    _ => return Err(self.err("unterminated string")),
                }
            }
        }
        fn number(&mut self) -> Result<Json, String> {
            let start = self.i;
            while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                self.i += 1;
            }
            let s = std::str::from_utf8(&self.b[start..self.i]).unwrap();
            s.parse::<f64>().map(Json::Num).map_err(|_| self.err("bad number"))
        }
    }
}

/// 一次总线访问（每项 = 1 M-cycle）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read(u16, u8),
    Write(u16, u8),
    Idle,
}

/// 平铺 64 KiB RAM 的假总线，记录每次 read/write/idle。
///
/// 中断：`irq = false`（默认）时 `pending()` 恒为 0；`irq = true` 时 IE/IF 就是 RAM 中的
/// `0xFFFF`/`0xFF0F`（CPU 写这两个地址会直接生效，便于测试 ie_push 一类的边界），
/// `ack` 清除 `0xFF0F` 的对应位。`raise` 中的 `(m, mask)` 表示在第 m 个 M-cycle（从 1 计数）
/// 开始时把 mask 或进 IF——与 Bus 的 D002 顺序一致（先推进外设，再访问）。
pub struct FlatBus {
    pub mem: Vec<u8>,
    pub log: Vec<Access>,
    pub irq: bool,
    pub raise: Vec<(u64, u8)>,
    pub mcycles: u64,
    pub acks: Vec<u8>,
    pub stops: u32,
}

pub const IF: u16 = 0xFF0F;
pub const IE: u16 = 0xFFFF;

impl Default for FlatBus {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatBus {
    pub fn new() -> Self {
        Self { mem: vec![0; 0x10000], log: Vec::with_capacity(8), irq: false, raise: Vec::new(), mcycles: 0, acks: Vec::new(), stops: 0 }
    }
    pub fn with_irq() -> Self {
        Self { irq: true, ..Self::new() }
    }
    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.mem[addr.wrapping_add(i as u16) as usize] = b;
        }
    }
    fn tick(&mut self) {
        self.mcycles += 1;
        let now = self.mcycles;
        let mut m = 0;
        for &(at, mask) in &self.raise {
            if at == now {
                m |= mask;
            }
        }
        self.mem[IF as usize] |= m;
    }
}

impl CpuBus for FlatBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.tick();
        let v = self.mem[addr as usize];
        self.log.push(Access::Read(addr, v));
        v
    }
    fn write(&mut self, addr: u16, v: u8) {
        self.tick();
        self.mem[addr as usize] = v;
        self.log.push(Access::Write(addr, v));
    }
    fn idle(&mut self) {
        self.tick();
        self.log.push(Access::Idle);
    }
    fn pending(&self) -> u8 {
        if self.irq {
            self.mem[IE as usize] & self.mem[IF as usize] & 0x1F
        } else {
            0
        }
    }
    fn ack(&mut self, bit: u8) {
        assert!(bit < 5, "ack bit {bit} out of range");
        self.acks.push(bit);
        if self.irq {
            self.mem[IF as usize] &= !(1 << bit);
        }
    }
    fn stop(&mut self) {
        self.stops += 1;
    }
}

#[test]
fn json_parser_basics() {
    use json::{parse, Json};
    let v = parse(r#" {"a": [1, 2.5, -3], "b": {"c": "x\"yA"}, "t": true, "n": null} "#).unwrap();
    assert_eq!(v.field("a").as_arr().len(), 3);
    assert_eq!(v.field("a").as_arr()[0].as_u64(), 1);
    assert_eq!(v.field("a").as_arr()[2], Json::Num(-3.0));
    assert_eq!(v.field("b").field("c").as_str(), "x\"yA");
    assert!(v.field("t").as_bool());
    assert_eq!(v.field("n"), &Json::Null);
    assert!(parse("[1,]").is_err());
    assert!(parse("{} x").is_err());
}

#[test]
fn flat_bus_logs_every_mcycle() {
    let mut bus = FlatBus::with_irq();
    bus.raise.push((2, 0x04));
    bus.mem[IE as usize] = 0x04;
    bus.write(0x1234, 0x56);
    assert_eq!(bus.pending(), 0);
    assert_eq!(bus.read(0x1234), 0x56);
    assert_eq!(bus.pending(), 0x04);
    bus.idle();
    bus.ack(2);
    assert_eq!(bus.pending(), 0);
    assert_eq!(bus.log, vec![Access::Write(0x1234, 0x56), Access::Read(0x1234, 0x56), Access::Idle]);
}
