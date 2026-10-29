//! gates 的判定逻辑（与运行解耦，便于对抗单测）。全项目的唯一裁判：只在这里出现期望结果。

use gb_core::{FRAME_PIXELS, SCREEN_W};

/// 一次运行是怎么结束的（与判定无关的事实）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEnd {
    /// 检测到结果关键字后正常结束（Blargg），或到达 LD B,B 后多跑 1 帧（acid2）。
    Finished,
    /// 达到帧数上限仍未结束。
    Timeout { frames: u64 },
    /// 模拟器 panic（含消息与位置）。
    Panic(String),
    /// 无法开始或无法继续（卡带错误、step 返回 0 等）。
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail(String),
}

impl Verdict {
    pub fn passed(&self) -> bool {
        matches!(self, Verdict::Pass)
    }
    pub fn reason(&self) -> &str {
        match self {
            Verdict::Pass => "",
            Verdict::Fail(r) => r,
        }
    }
}

/// Blargg 的"通过"关键字：individual 为 `Passed`，合集为 `Passed all tests`。
pub fn blargg_pass_needle(collection: bool) -> &'static str {
    if collection {
        "Passed all tests"
    } else {
        "Passed"
    }
}

const FAILED: &str = "Failed";
const PASSED: &str = "Passed";

/// 串口文本中是否已出现结果关键字（`Passed` 或 `Failed`），用于提前结束。
pub fn blargg_result_seen(serial: &str) -> bool {
    serial.contains(PASSED) || serial.contains(FAILED)
}

/// 结果行是否已完整：最后一个关键字之后已经出现换行。用于在关键字出现后再多收几帧，
/// 以免把 `Passed all tests` 截断成 `Passed al`，或漏掉随后出现的 `Failed`。
pub fn blargg_result_line_complete(serial: &str) -> bool {
    let last = [serial.rfind(PASSED), serial.rfind(FAILED)].into_iter().flatten().max();
    match last {
        Some(i) => serial[i..].contains('\n'),
        None => false,
    }
}

/// 把串口文本的末尾压成一行摘要（最后 2 个非空行，用 ` | ` 连接，最长 `max` 字符）。
pub fn serial_tail(serial: &str, max: usize) -> String {
    let lines: Vec<&str> = serial.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let start = lines.len().saturating_sub(2);
    let joined = lines[start..].join(" | ");
    clip(&joined, max)
}

/// 截到 `max` 个字符，去掉控制字符。
pub fn clip(s: &str, max: usize) -> String {
    let clean: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let n = clean.chars().count();
    if n <= max {
        clean
    } else {
        let mut t: String = clean.chars().take(max.saturating_sub(3)).collect();
        t.push_str("...");
        t
    }
}

/// Blargg 判定：串口文本含 `Passed`（合集为 `Passed all tests`）且不含 `Failed`，
/// 并且运行没有 panic 或出错。超时本身不直接判失败——只看文本；但超时且无关键字必然失败。
pub fn judge_blargg(serial: &str, collection: bool, end: &RunEnd) -> Verdict {
    match end {
        RunEnd::Panic(msg) => return Verdict::Fail(format!("panic: {}", clip(msg, 90))),
        RunEnd::Error(msg) => return Verdict::Fail(format!("error: {}", clip(msg, 90))),
        _ => {}
    }
    if serial.contains(FAILED) {
        return Verdict::Fail(format!("Failed: {}", serial_tail(serial, 80)));
    }
    if serial.contains(blargg_pass_needle(collection)) {
        return Verdict::Pass;
    }
    let what = match end {
        RunEnd::Timeout { frames } => format!("timeout after {frames} frames"),
        _ => "ended without result".to_string(),
    };
    if serial.trim().is_empty() {
        Verdict::Fail(format!("{what}, no serial output"))
    } else {
        Verdict::Fail(format!("{what}, serial: {}", serial_tail(serial, 70)))
    }
}

/// 两帧之间的差异统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStats {
    /// 不同的像素数。
    pub count: usize,
    /// 包围盒 (x0, y0, x1, y1)，含端点；无差异时为 None。
    pub bbox: Option<(usize, usize, usize, usize)>,
}

impl DiffStats {
    pub fn describe(&self) -> String {
        match self.bbox {
            None => "0 px differ".to_string(),
            Some((x0, y0, x1, y1)) => {
                format!("{} px differ, bbox ({x0},{y0})-({x1},{y1})", self.count)
            }
        }
    }
}

/// 逐字节比较两帧（长度不同时多出的部分全算差异）。
pub fn diff_frames(actual: &[u8], reference: &[u8]) -> DiffStats {
    let n = actual.len().max(reference.len());
    let mut st = DiffStats::default();
    for i in 0..n {
        if actual.get(i) != reference.get(i) {
            st.count += 1;
            let (x, y) = (i % SCREEN_W, i / SCREEN_W);
            st.bbox = Some(match st.bbox {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    st
}

/// acid2 判定：运行正常结束（到达 LD B,B 后又跑了 1 帧）、帧与参考逐字节完全相同才通过。
/// 返回差异统计（有帧可比时）。
pub fn judge_acid2(
    end: &RunEnd,
    actual: Option<&[u8]>,
    reference: &[u8],
) -> (Verdict, Option<DiffStats>) {
    if reference.len() != FRAME_PIXELS {
        return (
            Verdict::Fail(format!("reference has {} bytes, expected {FRAME_PIXELS}", reference.len())),
            None,
        );
    }
    let diff = actual.map(|a| diff_frames(a, reference));
    let prefix = match end {
        RunEnd::Panic(msg) => Some(format!("panic: {}", clip(msg, 90))),
        RunEnd::Error(msg) => Some(format!("error: {}", clip(msg, 90))),
        RunEnd::Timeout { frames } => Some(format!("no LD B,B within {frames} frames")),
        RunEnd::Finished => None,
    };
    let verdict = match (prefix, actual, diff) {
        (Some(p), _, Some(d)) => Verdict::Fail(format!("{p}; {}", d.describe())),
        (Some(p), _, None) => Verdict::Fail(p),
        (None, None, _) | (None, _, None) => Verdict::Fail("no frame captured".to_string()),
        (None, Some(a), Some(d)) => {
            if a.len() != FRAME_PIXELS {
                Verdict::Fail(format!("frame has {} bytes, expected {FRAME_PIXELS}", a.len()))
            } else if d.count == 0 && a == reference {
                Verdict::Pass
            } else {
                Verdict::Fail(d.describe())
            }
        }
    };
    (verdict, diff)
}

/// Mooneye 通过时首次 LD B,B 处 B/C/D/E/H/L 的值（斐波那契数）。
pub const MOONEYE_PASS_REGS: [u8; 6] = [3, 5, 8, 13, 21, 34];
/// Mooneye 报告失败时 6 个寄存器的值。
pub const MOONEYE_FAIL_BYTE: u8 = 0x42;

/// Mooneye 判定：首次 LD B,B 时 B/C/D/E/H/L 恰为 3/5/8/13/21/34 才通过；
/// 全为 0x42、超时（没有 LD B,B）、panic、出错或其他任何寄存器值都失败。
pub fn judge_mooneye(end: &RunEnd, regs: Option<[u8; 6]>) -> Verdict {
    match end {
        RunEnd::Panic(msg) => return Verdict::Fail(format!("panic: {}", clip(msg, 90))),
        RunEnd::Error(msg) => return Verdict::Fail(format!("error: {}", clip(msg, 90))),
        RunEnd::Timeout { frames } => return Verdict::Fail(format!("timeout: no LD B,B within {frames} frames")),
        RunEnd::Finished => {}
    }
    let Some(r) = regs else {
        return Verdict::Fail("no registers captured at LD B,B".to_string());
    };
    if r == MOONEYE_PASS_REGS {
        return Verdict::Pass;
    }
    let shown = format!(
        "B={:02X} C={:02X} D={:02X} E={:02X} H={:02X} L={:02X}",
        r[0], r[1], r[2], r[3], r[4], r[5]
    );
    if r.iter().all(|&b| b == MOONEYE_FAIL_BYTE) {
        Verdict::Fail(format!("test reported failure ({shown})"))
    } else {
        Verdict::Fail(format!("unexpected registers at LD B,B ({shown})"))
    }
}
