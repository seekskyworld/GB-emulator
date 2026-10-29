//! 宝可梦确定性回放基线（PLAN §4.4，T63）：固定按键脚本 + 每 300 帧一个检查点（帧哈希与关键 WRAM）。
//!
//! 只用 `Machine::set_buttons` 与 `run_frame`，不依赖墙钟，因此同一 ROM、同一脚本的结果完全确定。
//! 检查点里的 WRAM 地址来自 pokered 反汇编（`refs/pokered`）的符号，只在 gb-cli 中出现，core 不感知。

use std::path::Path;

use gb_core::Machine;

use crate::emu::catch;
use crate::json::{self, Value};

/// 检查点间隔（帧）。
pub const INTERVAL: u64 = 300;
/// gates 里的组名。
pub const GROUP: &str = "POKERED-REPLAY";

/// wCurMap。
pub const W_CUR_MAP: u16 = 0xD35E;
/// wYCoord。
pub const W_Y_COORD: u16 = 0xD361;
/// wXCoord。
pub const W_X_COORD: u16 = 0xD362;
/// wPlayerName（11 字节）。
pub const W_PLAYER_NAME: u16 = 0xD158;
/// wPartyCount。
pub const W_PARTY_COUNT: u16 = 0xD163;

/// 按键脚本的一步：按住 `mask` `hold` 帧，再全部松开 `wait` 帧；重复 `repeat` 次。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub mask: u8,
    pub hold: u32,
    pub wait: u32,
    pub repeat: u32,
}

impl Step {
    pub fn frames(&self) -> u64 {
        (self.hold as u64 + self.wait as u64) * self.repeat as u64
    }
}

/// 按键名（不区分大小写）：a b select start right left up down；用 `+` 组合；空串或 `none` 表示不按。
pub fn parse_buttons(s: &str) -> Result<u8, String> {
    let mut mask = 0u8;
    for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        mask |= match part.to_ascii_lowercase().as_str() {
            "a" => gb_core::buttons::A,
            "b" => gb_core::buttons::B,
            "select" => gb_core::buttons::SELECT,
            "start" => gb_core::buttons::START,
            "right" => gb_core::buttons::RIGHT,
            "left" => gb_core::buttons::LEFT,
            "up" => gb_core::buttons::UP,
            "down" => gb_core::buttons::DOWN,
            "none" => 0,
            other => return Err(format!("unknown button `{other}`")),
        };
    }
    Ok(mask)
}

/// 解析脚本：JSON 数组，元素为 `{"buttons": "start", "hold": 8, "wait": 12, "repeat": 1, "note": "..."}`；
/// `hold`、`wait` 默认 0，`repeat` 默认 1，`note` 只是注释。
pub fn parse_script(text: &str) -> Result<Vec<Step>, String> {
    let v = json::parse(text)?;
    let arr = v.as_array().ok_or("script must be a JSON array")?;
    let num = |o: &Value, k: &str, d: u32, i: usize| -> Result<u32, String> {
        match o.get(k) {
            None => Ok(d),
            Some(Value::Num(n)) if *n >= 0.0 && n.fract() == 0.0 && *n <= 100_000.0 => Ok(*n as u32),
            Some(_) => Err(format!("step {i}: `{k}` must be a non-negative integer")),
        }
    };
    let mut steps = Vec::with_capacity(arr.len());
    for (i, o) in arr.iter().enumerate() {
        if !matches!(o, Value::Obj(_)) {
            return Err(format!("step {i}: must be an object"));
        }
        let buttons = match o.get("buttons") {
            None => "",
            Some(b) => b.as_str().ok_or(format!("step {i}: `buttons` must be a string"))?,
        };
        let step = Step {
            mask: parse_buttons(buttons).map_err(|e| format!("step {i}: {e}"))?,
            hold: num(o, "hold", 0, i)?,
            wait: num(o, "wait", 0, i)?,
            repeat: num(o, "repeat", 1, i)?,
        };
        if step.mask != 0 && step.hold == 0 {
            return Err(format!("step {i}: buttons without hold"));
        }
        steps.push(step);
    }
    if steps.iter().map(Step::frames).sum::<u64>() == 0 {
        return Err("script runs 0 frames".into());
    }
    Ok(steps)
}

/// 一个检查点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub frame: u64,
    pub frame_hash: u64,
    pub map: u8,
    pub y: u8,
    pub x: u8,
    pub player_name: [u8; 11],
    pub party_count: u8,
}

/// FNV-1a 64。
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn checkpoint(m: &Machine, frame: u64) -> Checkpoint {
    let mut name = [0u8; 11];
    for (i, b) in name.iter_mut().enumerate() {
        *b = m.peek(W_PLAYER_NAME + i as u16);
    }
    Checkpoint {
        frame,
        frame_hash: fnv1a64(m.frame()),
        map: m.peek(W_CUR_MAP),
        y: m.peek(W_Y_COORD),
        x: m.peek(W_X_COORD),
        player_name: name,
        party_count: m.peek(W_PARTY_COUNT),
    }
}

/// pokered 字符表（只解码大写字母、数字与 0x50 结束符；其它显示为 `?`），只用于报告的可读性。
pub fn decode_name(raw: &[u8]) -> String {
    let mut s = String::new();
    for &b in raw {
        match b {
            0x50 => break,
            0x80..=0x99 => s.push((b'A' + (b - 0x80)) as char),
            0xF6..=0xFF => s.push((b'0' + (b - 0xF6)) as char),
            0x7F => s.push(' '),
            _ => s.push('?'),
        }
    }
    s
}

/// ROM 指纹：标题、头部校验和、全局校验和。
pub fn rom_fingerprint(rom: &[u8]) -> String {
    if rom.len() < 0x150 {
        return format!("short ROM ({} bytes)", rom.len());
    }
    let title: String =
        rom[0x134..0x144].iter().take_while(|&&b| b != 0).map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' }).collect();
    format!("{title} hdr={:02X} global={:02X}{:02X}", rom[0x14D], rom[0x14E], rom[0x14F])
}

/// 脚本指纹：只看解析后的按键序列（`note` 与排版不影响）。
pub fn steps_hash(steps: &[Step]) -> u64 {
    let canon: String = steps.iter().map(|s| format!("{},{},{},{};", s.mask, s.hold, s.wait, s.repeat)).collect();
    fnv1a64(canon.as_bytes())
}

/// 一次回放的结果。
#[derive(Debug, Clone)]
pub struct Recording {
    pub rom: String,
    pub script_hash: u64,
    pub frames: u64,
    pub checkpoints: Vec<Checkpoint>,
    pub last_frame: Vec<u8>,
}

/// 跑脚本，按 `INTERVAL` 记录检查点。panic 转成错误。
pub fn run(rom: Vec<u8>, steps: &[Step]) -> Result<Recording, String> {
    let fp = rom_fingerprint(&rom);
    let script_hash = steps_hash(steps);
    let r = catch(move || -> Result<Recording, String> {
        let mut m = Machine::new(rom).map_err(|e| format!("cart: {e}"))?;
        let mut frame = 0u64;
        let mut cps = Vec::new();
        let tick = |m: &mut Machine, mask: u8, n: u32, frame: &mut u64, cps: &mut Vec<Checkpoint>| {
            m.set_buttons(mask);
            for _ in 0..n {
                m.run_frame();
                *frame += 1;
                if (*frame).is_multiple_of(INTERVAL) {
                    cps.push(checkpoint(m, *frame));
                }
            }
        };
        for s in steps {
            for _ in 0..s.repeat {
                tick(&mut m, s.mask, s.hold, &mut frame, &mut cps);
                tick(&mut m, 0, s.wait, &mut frame, &mut cps);
            }
        }
        m.set_buttons(0);
        Ok(Recording {
            rom: fp,
            script_hash,
            frames: frame,
            checkpoints: cps,
            last_frame: m.frame().to_vec(),
        })
    });
    match r {
        Ok(x) => x,
        Err(p) => Err(format!("panic: {p}")),
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// 序列化为 `reports/pokered-replay.json`。
pub fn to_json(r: &Recording, script_path: &str) -> Value {
    let s = |x: String| Value::Str(x);
    let n = |x: u64| Value::Num(x as f64);
    let cps = r
        .checkpoints
        .iter()
        .map(|c| {
            Value::Obj(vec![
                ("frame".into(), n(c.frame)),
                ("frame_hash".into(), s(format!("{:016x}", c.frame_hash))),
                ("map".into(), n(c.map as u64)),
                ("y".into(), n(c.y as u64)),
                ("x".into(), n(c.x as u64)),
                ("player_name".into(), s(hex(&c.player_name))),
                ("player_name_text".into(), s(decode_name(&c.player_name))),
                ("party_count".into(), n(c.party_count as u64)),
            ])
        })
        .collect();
    Value::Obj(vec![
        ("rom".into(), s(r.rom.clone())),
        ("script".into(), s(script_path.to_string())),
        ("script_hash".into(), s(format!("{:016x}", r.script_hash))),
        ("interval".into(), n(INTERVAL)),
        ("frames".into(), n(r.frames)),
        ("checkpoints".into(), Value::Arr(cps)),
    ])
}

/// 读回参考文件。
pub fn from_json(v: &Value) -> Result<Recording, String> {
    let gs = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).map(str::to_string).ok_or(format!("missing `{k}`"));
    let gn = |o: &Value, k: &str| match o.get(k) {
        Some(Value::Num(x)) if *x >= 0.0 && x.fract() == 0.0 => Ok(*x as u64),
        _ => Err(format!("missing or bad `{k}`")),
    };
    let byte = |o: &Value, k: &str| gn(o, k).and_then(|x| u8::try_from(x).map_err(|_| format!("`{k}` out of range")));
    let h64 = |o: &Value, k: &str| gs(o, k).and_then(|x| u64::from_str_radix(&x, 16).map_err(|_| format!("bad `{k}`")));
    let mut cps = Vec::new();
    for (i, c) in v.get("checkpoints").and_then(Value::as_array).ok_or("missing `checkpoints`")?.iter().enumerate() {
        let name = unhex(&gs(c, "player_name")?).filter(|b| b.len() == 11).ok_or(format!("checkpoint {i}: bad player_name"))?;
        cps.push(Checkpoint {
            frame: gn(c, "frame").map_err(|e| format!("checkpoint {i}: {e}"))?,
            frame_hash: h64(c, "frame_hash").map_err(|e| format!("checkpoint {i}: {e}"))?,
            map: byte(c, "map")?,
            y: byte(c, "y")?,
            x: byte(c, "x")?,
            player_name: name.try_into().unwrap(),
            party_count: byte(c, "party_count")?,
        });
    }
    Ok(Recording {
        rom: gs(v, "rom")?,
        script_hash: h64(v, "script_hash")?,
        frames: gn(v, "frames")?,
        checkpoints: cps,
        last_frame: Vec::new(),
    })
}

/// 逐个检查点比较；返回第一个不一致（帧号与字段），一致时为 None。
pub fn compare(reference: &Recording, actual: &Recording) -> Option<String> {
    if reference.rom != actual.rom {
        return Some(format!("ROM differs: reference `{}`, actual `{}`", reference.rom, actual.rom));
    }
    if reference.script_hash != actual.script_hash {
        return Some("script changed (script_hash differs); re-record the reference".to_string());
    }
    for (r, a) in reference.checkpoints.iter().zip(&actual.checkpoints) {
        let f = r.frame;
        if r.frame != a.frame {
            return Some(format!("checkpoint frame numbers differ: {} vs {}", r.frame, a.frame));
        }
        let diffs: [(&str, String, String); 6] = [
            ("frame_hash", format!("{:016x}", r.frame_hash), format!("{:016x}", a.frame_hash)),
            ("map", format!("{:#04x}", r.map), format!("{:#04x}", a.map)),
            ("y", r.y.to_string(), a.y.to_string()),
            ("x", r.x.to_string(), a.x.to_string()),
            ("player_name", decode_name(&r.player_name), decode_name(&a.player_name)),
            ("party_count", r.party_count.to_string(), a.party_count.to_string()),
        ];
        let mut bad: Vec<String> = Vec::new();
        for (k, rv, av) in diffs {
            if rv != av || (k == "player_name" && r.player_name != a.player_name) {
                bad.push(format!("{k} {rv} -> {av}"));
            }
        }
        if !bad.is_empty() {
            return Some(format!("frame {f}: {}", bad.join(", ")));
        }
    }
    if reference.checkpoints.len() != actual.checkpoints.len() || reference.frames != actual.frames {
        return Some(format!(
            "length differs: reference {} checkpoints / {} frames, actual {} / {}",
            reference.checkpoints.len(),
            reference.frames,
            actual.checkpoints.len(),
            actual.frames
        ));
    }
    None
}

/// gates 用的回放结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok,
    Changed(String),
    Skip(String),
}

impl Status {
    pub fn word(&self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Changed(_) => "changed",
            Status::Skip(_) => "skip",
        }
    }
    pub fn detail(&self) -> &str {
        match self {
            Status::Ok => "",
            Status::Changed(d) | Status::Skip(d) => d,
        }
    }
}

/// gates 的 POKERED-REPLAY：ROM 缺失为 Skip；脚本或参考缺失、回放出错、任何不一致都为 Changed。
pub fn gate(rom_path: &Path, script_path: &Path, reference_path: &Path) -> Status {
    let rom = match std::fs::read(rom_path) {
        Ok(r) => r,
        Err(e) => return Status::Skip(format!("ROM {} not readable ({e})", rom_path.display())),
    };
    let script_text = match std::fs::read_to_string(script_path) {
        Ok(t) => t,
        Err(e) => return Status::Changed(format!("script {} not readable ({e})", script_path.display())),
    };
    let steps = match parse_script(&script_text) {
        Ok(s) => s,
        Err(e) => return Status::Changed(format!("bad script: {e}")),
    };
    let reference = match std::fs::read_to_string(reference_path)
        .map_err(|e| e.to_string())
        .and_then(|t| json::parse(&t))
        .and_then(|v| from_json(&v))
    {
        Ok(r) => r,
        Err(e) => return Status::Changed(format!("reference {}: {e}", reference_path.display())),
    };
    match run(rom, &steps) {
        Err(e) => Status::Changed(format!("replay failed: {e}")),
        Ok(actual) => match compare(&reference, &actual) {
            None => Status::Ok,
            Some(d) => Status::Changed(d),
        },
    }
}
