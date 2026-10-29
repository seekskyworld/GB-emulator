//! gates：门槛 ROM 清单、并行运行、汇总与报告（PLAN §4.1）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use gb_core::Machine;

use crate::emu::{catch, FrameStepper, PcTrace};
use crate::json::Value;
use crate::judge::{self, DiffStats, RunEnd, Verdict};

/// 模拟时间的帧率（DMG：4194304 / 70224 ≈ 59.73 Hz）。
pub const FPS: f64 = 59.73;
/// Blargg individual 超时（模拟秒）。
pub const BLARGG_INDIVIDUAL_SECS: f64 = 60.0;
/// Blargg 合集超时（模拟秒）。
pub const BLARGG_COLLECTION_SECS: f64 = 180.0;
/// acid2 的总帧数上限（含 LD B,B 之后多跑的 1 帧）。
pub const ACID2_MAX_FRAMES: u64 = 120;
/// 关键字出现后最多再收的帧数（等结果行的换行，并捕获紧随其后的 `Failed`）。
pub const BLARGG_GRACE_FRAMES: u64 = 60;
/// Mooneye 超时（模拟秒）。
pub const MOONEYE_SECS: f64 = 30.0;
/// Mooneye 测试 ROM 目录（相对 `$GBEMU_REFS`；版本由 PLAN §1.1 B1 固定）。
pub const MOONEYE_DIR: &str = "mooneye/mts-20260714-0944-31510e1";
/// Mooneye 的组名（`--only MOONEYE`、汇总行）。
pub const MOONEYE: &str = "MOONEYE";
/// Blargg 判定结束后、截图之前多跑的帧数（只影响截图，不影响判定）。
pub const BLARGG_SCREENSHOT_FRAMES: u32 = 30;
/// gates.json 里每个 ROM 保存的串口文本上限。
pub const SERIAL_JSON_MAX: usize = 2048;

pub fn secs_to_frames(secs: f64) -> u64 {
    (secs * FPS).ceil() as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Blargg { collection: bool },
    Acid2,
    Mooneye,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub group: &'static str,
    /// 相对 `$GBEMU_REFS` 的路径。
    pub rom: String,
    pub kind: Kind,
    /// 是否计入汇总行、退出码与棘轮（Mooneye 中只有纳入范围的项为 true；G1–G4 恒为 true）。
    pub scored: bool,
    /// Mooneye 纳入/排除的理由（G1–G4 为空）。
    pub scope: String,
}

impl Entry {
    /// 文件名去掉扩展名（如 `03-op sp,hl`），用作日志与截图文件名。
    pub fn name(&self) -> String {
        if self.kind == Kind::Mooneye {
            let rel = self.rom.strip_prefix(MOONEYE_DIR).unwrap_or(&self.rom).trim_start_matches('/');
            return rel.strip_suffix(".gb").unwrap_or(rel).to_string();
        }
        let file = self.rom.rsplit('/').next().unwrap_or(&self.rom);
        file.strip_suffix(".gb").unwrap_or(file).to_string()
    }
    pub fn is_collection(&self) -> bool {
        matches!(self.kind, Kind::Blargg { collection: true })
    }
    pub fn max_frames(&self) -> u64 {
        match self.kind {
            Kind::Blargg { collection: false } => secs_to_frames(BLARGG_INDIVIDUAL_SECS),
            Kind::Blargg { collection: true } => secs_to_frames(BLARGG_COLLECTION_SECS),
            Kind::Acid2 => ACID2_MAX_FRAMES,
            Kind::Mooneye => secs_to_frames(MOONEYE_SECS),
        }
    }
}

pub const GROUPS: [&str; 4] = ["G1", "G2", "G3", "G4"];

const CPU_INSTRS: [&str; 11] = [
    "01-special",
    "02-interrupts",
    "03-op sp,hl",
    "04-op r,imm",
    "05-op rp",
    "06-ld r,r",
    "07-jr,jp,call,ret,rst",
    "08-misc instrs",
    "09-op r,r",
    "10-bit ops",
    "11-op a,(hl)",
];

const MEM_TIMING: [&str; 3] = ["01-read_timing", "02-write_timing", "03-modify_timing"];

/// 全部 18 个门槛 ROM（G1 12、G2 1、G3 4、G4 1）。
pub fn manifest() -> Vec<Entry> {
    let blargg = |group, rom: String, collection| Entry {
        group,
        rom,
        kind: Kind::Blargg { collection },
        scored: true,
        scope: String::new(),
    };
    let mut v = Vec::new();
    for n in CPU_INSTRS {
        v.push(blargg("G1", format!("blargg/cpu_instrs/individual/{n}.gb"), false));
    }
    v.push(blargg("G1", "blargg/cpu_instrs/cpu_instrs.gb".into(), true));
    v.push(blargg("G2", "blargg/instr_timing/instr_timing.gb".into(), false));
    v.push(blargg("G3", "blargg/mem_timing/mem_timing.gb".into(), true));
    for n in MEM_TIMING {
        v.push(blargg("G3", format!("blargg/mem_timing/individual/{n}.gb"), false));
    }
    v.push(Entry {
        group: "G4",
        rom: "acid2/dmg-acid2.gb".into(),
        kind: Kind::Acid2,
        scored: true,
        scope: String::new(),
    });
    v
}

/// Mooneye 纳入规则（PLAN §2.4，DMG-ABC）。`rel` 为相对测试集根目录的路径（如 `acceptance/boot_div2-S.gb`）。
/// 只看 `acceptance/` 与 `emulator-only/`；型号后缀为文件名中最后一个 `-` 之后、`.gb` 之前的部分。
/// 纳入：无后缀；后缀含 `G`（dmg+mgb 组）；后缀含 `dmgABC`。其余后缀排除。
pub fn mooneye_scope(rel: &str) -> (bool, String) {
    let top = rel.split('/').next().unwrap_or("");
    if rel.split('/').count() < 2 || !matches!(top, "acceptance" | "emulator-only") {
        return (false, format!("directory {top}/ is not acceptance/ or emulator-only/"));
    }
    let file = rel.rsplit('/').next().unwrap_or(rel);
    let stem = file.strip_suffix(".gb").unwrap_or(file);
    match stem.rsplit_once('-') {
        None => (true, "no model suffix".to_string()),
        Some((_, suffix)) if suffix.contains('G') => (true, format!("suffix -{suffix} contains G (dmg+mgb)")),
        Some((_, suffix)) if suffix.contains("dmgABC") => (true, format!("suffix -{suffix} contains dmgABC")),
        Some((_, suffix)) => (false, format!("suffix -{suffix} does not cover DMG-ABC")),
    }
}

fn walk_gb(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for ent in std::fs::read_dir(dir)? {
        let p = ent?.path();
        if p.is_dir() {
            walk_gb(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "gb") {
            out.push(p);
        }
    }
    Ok(())
}

/// Mooneye 全部 ROM（按路径排序），每项带纳入标记与理由。目录缺失时报错。
pub fn mooneye_manifest(refs: &Path) -> Result<Vec<Entry>, String> {
    let root = refs.join(MOONEYE_DIR);
    let mut files = Vec::new();
    walk_gb(&root, &mut files).map_err(|e| format!("cannot list {}: {e}", root.display()))?;
    let mut rels: Vec<String> = files
        .iter()
        .filter_map(|p| p.strip_prefix(&root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    rels.sort();
    Ok(rels
        .into_iter()
        .map(|rel| {
            let (scored, scope) = mooneye_scope(&rel);
            Entry { group: MOONEYE, rom: format!("{MOONEYE_DIR}/{rel}"), kind: Kind::Mooneye, scored, scope }
        })
        .collect())
}

/// `--only` 是否点名了 Mooneye。
pub fn only_mentions_mooneye(only: Option<&[String]>) -> bool {
    only.is_some_and(|g| g.iter().any(|x| x == MOONEYE))
}

/// 完整选择：G1–G4 按 `--only`/`--fast` 过滤；`--mooneye` 或 `--only` 含 MOONEYE 时追加 Mooneye 全部 ROM。
pub fn select_all(only: Option<&[String]>, fast: bool, mooneye: bool, refs: &Path) -> Result<Vec<Entry>, String> {
    let mut v = select(only, fast);
    if mooneye || only_mentions_mooneye(only) {
        v.extend(mooneye_manifest(refs)?);
    }
    Ok(v)
}

/// 按 `--only`、`--fast` 过滤清单。
pub fn select(only: Option<&[String]>, fast: bool) -> Vec<Entry> {
    manifest()
        .into_iter()
        .filter(|e| only.is_none_or(|g| g.iter().any(|x| x == e.group)))
        .filter(|e| !(fast && e.is_collection()))
        .collect()
}

/// 单个 ROM 的运行结果。
#[derive(Debug, Clone)]
pub struct RomResult {
    pub kind: Kind,
    pub group: String,
    pub name: String,
    pub rom: String,
    pub pass: bool,
    pub reason: String,
    pub frames: u64,
    pub serial: Vec<u8>,
    pub trace: Vec<u16>,
    pub end: RunEnd,
    pub diff: Option<DiffStats>,
    pub frame: Option<Vec<u8>>,
    /// 见 `Entry::scored`。
    pub scored: bool,
    /// 见 `Entry::scope`。
    pub scope: String,
    /// Mooneye：首次 LD B,B 时的 B/C/D/E/H/L。
    pub regs: Option<[u8; 6]>,
}

impl RomResult {
    pub fn serial_text(&self) -> String {
        String::from_utf8_lossy(&self.serial).into_owned()
    }
}

/// 运行过程中累计、panic 后仍保留的状态。
#[derive(Default)]
struct Progress {
    serial: Vec<u8>,
    frames: u64,
    trace: PcTrace,
    frame: Option<Vec<u8>>,
    regs: Option<[u8; 6]>,
}

fn finish(e: &Entry, p: Progress, end: RunEnd, verdict: Verdict, diff: Option<DiffStats>) -> RomResult {
    RomResult {
        kind: e.kind,
        group: e.group.to_string(),
        name: e.name(),
        rom: e.rom.clone(),
        pass: verdict.passed(),
        reason: verdict.reason().to_string(),
        frames: p.frames,
        serial: p.serial,
        trace: p.trace.to_vec(),
        end,
        diff,
        frame: p.frame,
        scored: e.scored,
        scope: e.scope.clone(),
        regs: p.regs,
    }
}

/// 把运行闭包的结果折叠成 `RunEnd`。
fn fold_end(r: Result<Result<RunEnd, String>, String>) -> RunEnd {
    match r {
        Ok(Ok(end)) => end,
        Ok(Err(msg)) => RunEnd::Error(msg),
        Err(panic) => RunEnd::Panic(panic),
    }
}

/// 运行一个 Blargg ROM：逐帧推进、累计串口，检测到关键字后等结果行完整（或 grace 帧）即结束。
pub fn run_blargg(e: &Entry, rom: Vec<u8>) -> RomResult {
    let collection = e.is_collection();
    let max = e.max_frames();
    let mut p = Progress::default();
    let mut machine: Option<Machine> = None;
    let r = catch(|| -> Result<RunEnd, String> {
        let m = machine.insert(Machine::new(rom).map_err(|err| format!("cart: {err}"))?);
        let mut stepper = FrameStepper::new();
        let mut seen_at: Option<u64> = None;
        let mut end = RunEnd::Timeout { frames: max };
        while p.frames < max {
            stepper.run_frame(m, &mut p.trace)?;
            p.frames += 1;
            let new = m.take_serial();
            if !new.is_empty() {
                p.serial.extend_from_slice(&new);
            }
            if seen_at.is_none() || !new.is_empty() {
                let text = String::from_utf8_lossy(&p.serial);
                if seen_at.is_none() && judge::blargg_result_seen(&text) {
                    seen_at = Some(p.frames);
                }
                if seen_at.is_some() && judge::blargg_result_line_complete(&text) {
                    end = RunEnd::Finished;
                    break;
                }
            }
            if let Some(at) = seen_at {
                if p.frames - at >= BLARGG_GRACE_FRAMES {
                    end = RunEnd::Finished;
                    break;
                }
            }
        }
        if seen_at.is_some() {
            end = RunEnd::Finished;
        }
        p.frame = Some(m.frame().to_vec());
        Ok(end)
    });
    let end = fold_end(r);
    let text = String::from_utf8_lossy(&p.serial).into_owned();
    let verdict = judge::judge_blargg(&text, collection, &end);
    // 判定已定（D017）。正常结束时再跑 BLARGG_SCREENSHOT_FRAMES 帧只为截图：屏幕比串口晚刷出最后一行。
    // 这几帧的串口、轨迹与帧数都不进结果；期间若 panic，保留判定时的截图，结果不变。
    if end == RunEnd::Finished {
        if let Some(mut m) = machine {
            if let Ok(frame) = catch(move || {
                for _ in 0..BLARGG_SCREENSHOT_FRAMES {
                    m.run_frame();
                }
                m.frame().to_vec()
            }) {
                p.frame = Some(frame);
            }
        }
    }
    finish(e, p, end, verdict, None)
}

/// 运行 acid2：跑到首次 LD B,B（按帧检查）后再跑 1 帧，总上限 120 帧；与参考逐字节比较。
pub fn run_acid2(e: &Entry, rom: Vec<u8>, reference: &[u8]) -> RomResult {
    let max = e.max_frames();
    let mut p = Progress::default();
    let r = catch(|| -> Result<RunEnd, String> {
        let mut m = Machine::new(rom).map_err(|err| format!("cart: {err}"))?;
        let mut stepper = FrameStepper::new();
        let mut end = RunEnd::Timeout { frames: max - 1 };
        // 留 1 帧给命中之后：命中必须发生在前 max-1 帧内。
        while p.frames < max - 1 {
            stepper.run_frame(&mut m, &mut p.trace)?;
            p.frames += 1;
            p.serial.extend_from_slice(&m.take_serial());
            if m.ld_b_b_hit() {
                stepper.run_frame(&mut m, &mut p.trace)?;
                p.frames += 1;
                p.serial.extend_from_slice(&m.take_serial());
                end = RunEnd::Finished;
                break;
            }
        }
        p.frame = Some(m.frame().to_vec());
        Ok(end)
    });
    let end = fold_end(r);
    let (verdict, diff) = judge::judge_acid2(&end, p.frame.as_deref(), reference);
    finish(e, p, end, verdict, diff)
}

/// 运行一个 Mooneye ROM：逐条指令检查 LD B,B，首次命中时取 B/C/D/E/H/L；上限 30 秒模拟时间。
pub fn run_mooneye(e: &Entry, rom: Vec<u8>) -> RomResult {
    let max = e.max_frames();
    let mut p = Progress::default();
    let r = catch(|| -> Result<RunEnd, String> {
        let mut m = Machine::new(rom).map_err(|err| format!("cart: {err}"))?;
        let mut stepper = FrameStepper::new();
        while p.frames < max {
            let hit = stepper.run_frame_until_ld_b_b(&mut m, &mut p.trace)?;
            p.serial.extend_from_slice(&m.take_serial());
            if hit {
                let r = m.regs();
                p.regs = Some([r.b, r.c, r.d, r.e, r.h, r.l]);
                p.frame = Some(m.frame().to_vec());
                return Ok(RunEnd::Finished);
            }
            p.frames += 1;
        }
        p.frame = Some(m.frame().to_vec());
        Ok(RunEnd::Timeout { frames: max })
    });
    let end = fold_end(r);
    let verdict = judge::judge_mooneye(&end, p.regs);
    finish(e, p, end, verdict, None)
}

fn missing(e: &Entry, what: String) -> RomResult {
    let end = RunEnd::Error(what.clone());
    finish(e, Progress::default(), end, Verdict::Fail(what), None)
}

/// 运行单个清单项（读 ROM、读参考、分派）。永不 panic。
pub fn run_entry(e: &Entry, refs: &Path) -> RomResult {
    let path = refs.join(&e.rom);
    let rom = match std::fs::read(&path) {
        Ok(r) => r,
        Err(err) => return missing(e, format!("cannot read ROM: {err}")),
    };
    match e.kind {
        Kind::Blargg { .. } => run_blargg(e, rom),
        Kind::Mooneye => run_mooneye(e, rom),
        Kind::Acid2 => {
            let rp = refs.join("acid2/reference-dmg.gray");
            match std::fs::read(&rp) {
                Ok(reference) => run_acid2(e, rom, &reference),
                Err(err) => missing(e, format!("cannot read acid2/reference-dmg.gray: {err}")),
            }
        }
    }
}

/// 线程池并行运行；结果按清单顺序返回。最耗时的合集先启动。
pub fn run_all(entries: &[Entry], refs: &Path, threads: usize) -> Vec<RomResult> {
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(entries[i].max_frames()));
    let next = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<RomResult>>> = Mutex::new(vec![None; entries.len()]);
    let threads = threads.clamp(1, entries.len().max(1));
    std::thread::scope(|s| {
        for t in 0..threads {
            let (next, slots, order) = (&next, &slots, &order);
            std::thread::Builder::new()
                .name(format!("gates-{t}"))
                .stack_size(64 << 20)
                .spawn_scoped(s, move || loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&i) = order.get(k) else { break };
                    let r = run_entry(&entries[i], refs);
                    slots.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
                })
                .expect("spawn gates worker");
        }
    });
    slots
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .zip(entries)
        .map(|(r, e)| r.unwrap_or_else(|| missing(e, "worker did not report".into())))
        .collect()
}

/// 汇总行，如 `G1 0/12 G2 0/1 G3 0/4 G4 0/1`（只列出本次运行到的组）。
pub fn summary_line(results: &[RomResult]) -> String {
    let mut parts = Vec::new();
    for g in GROUPS {
        let total = results.iter().filter(|r| r.group == g).count();
        if total == 0 {
            continue;
        }
        let pass = results.iter().filter(|r| r.group == g && r.pass).count();
        parts.push(format!("{g} {pass}/{total}"));
    }
    // Mooneye 只计纳入范围的项；原始结果（全部 115 个）见 `mooneye_raw`。
    let scored: Vec<&RomResult> = results.iter().filter(|r| r.group == MOONEYE && r.scored).collect();
    if results.iter().any(|r| r.group == MOONEYE) {
        let pass = scored.iter().filter(|r| r.pass).count();
        parts.push(format!("{MOONEYE} {pass}/{}", scored.len()));
    }
    if parts.is_empty() {
        "(no gates selected)".to_string()
    } else {
        parts.join(" ")
    }
}

/// Mooneye 原始结果 `通过/全部`（不论是否纳入）；没有运行 Mooneye 时为 None。
pub fn mooneye_raw(results: &[RomResult]) -> Option<String> {
    let all: Vec<&RomResult> = results.iter().filter(|r| r.group == MOONEYE).collect();
    if all.is_empty() {
        return None;
    }
    Some(format!("{}/{}", all.iter().filter(|r| r.pass).count(), all.len()))
}

/// 串口文本前 `max` 字节（按 UTF-8 宽松解码）。
pub fn serial_head(serial: &[u8], max: usize) -> String {
    String::from_utf8_lossy(&serial[..serial.len().min(max)]).into_owned()
}

/// 汇总行加上回放结论（`REPLAY ok|changed|skip`）。
pub fn summary_with_replay(results: &[RomResult], replay: Option<&crate::replay::Status>) -> String {
    let base = summary_line(results);
    match replay {
        None => base,
        Some(st) if results.is_empty() => format!("REPLAY {}", st.word()),
        Some(st) => format!("{base} REPLAY {}", st.word()),
    }
}

/// 把回放结论写进 gates.json（`summary` 与 `replay` 字段）。
pub fn attach_replay(v: &mut Value, results: &[RomResult], replay: Option<&crate::replay::Status>, strict: bool) {
    let Some(st) = replay else { return };
    if let Value::Obj(kv) = v {
        for (k, val) in kv.iter_mut() {
            if k == "summary" {
                *val = Value::Str(summary_with_replay(results, replay));
            }
        }
        kv.push((
            "replay".to_string(),
            Value::Obj(vec![
                ("group".to_string(), Value::Str(crate::replay::GROUP.to_string())),
                ("status".to_string(), Value::Str(st.word().to_string())),
                ("detail".to_string(), Value::Str(st.detail().to_string())),
                ("strict".to_string(), Value::Bool(strict)),
            ]),
        ));
    }
}

/// gates.json 内容。
pub fn results_json(results: &[RomResult], fast: bool, only: Option<&[String]>) -> Value {
    let s = |x: &str| Value::Str(x.to_string());
    let items = results
        .iter()
        .map(|r| {
            let mut kv = vec![
                ("group".to_string(), s(&r.group)),
                ("name".to_string(), s(&r.name)),
                ("rom".to_string(), s(&r.rom)),
                ("pass".to_string(), Value::Bool(r.pass)),
                ("reason".to_string(), s(&r.reason)),
                ("frames".to_string(), Value::Num(r.frames as f64)),
                ("serial".to_string(), Value::Str(serial_head(&r.serial, SERIAL_JSON_MAX))),
            ];
            if r.kind == Kind::Mooneye {
                kv.push(("scored".to_string(), Value::Bool(r.scored)));
                kv.push(("scope".to_string(), s(&r.scope)));
                kv.push((
                    "regs".to_string(),
                    match r.regs {
                        None => Value::Null,
                        Some(rg) => Value::Arr(rg.iter().map(|&n| Value::Num(n as f64)).collect()),
                    },
                ));
            }
            if let Some(d) = r.diff {
                kv.push(("diff_pixels".to_string(), Value::Num(d.count as f64)));
                kv.push((
                    "diff_bbox".to_string(),
                    match d.bbox {
                        None => Value::Null,
                        Some((x0, y0, x1, y1)) => Value::Arr(
                            [x0, y0, x1, y1].iter().map(|&n| Value::Num(n as f64)).collect(),
                        ),
                    },
                ));
            }
            Value::Obj(kv)
        })
        .collect();
    let mut top = vec![("summary".to_string(), s(&summary_line(results)))];
    if let Some(raw) = mooneye_raw(results) {
        top.push(("mooneye_raw".to_string(), s(&raw)));
    }
    top.extend([
        ("fast".to_string(), Value::Bool(fast)),
        (
            "only".to_string(),
            match only {
                None => Value::Null,
                Some(g) => Value::Arr(g.iter().map(|x| s(x)).collect()),
            },
        ),
        ("results".to_string(), Value::Arr(items)),
    ]);
    Value::Obj(top)
}

/// gates 命令的选项。
#[derive(Debug, Clone)]
pub struct Options {
    pub fast: bool,
    pub only: Option<Vec<String>>,
    pub threads: usize,
    pub refs: PathBuf,
    pub out: PathBuf,
    /// `--mooneye`：追加 Mooneye 全部 ROM。
    pub mooneye: bool,
    /// POKERED-REPLAY 的结论（`--replay` 或 `--only POKERED-REPLAY` 时运行；未运行为 None）。
    pub replay: Option<crate::replay::Status>,
    /// `--strict-replay`：回放变化视为失败（默认只 WARN，不影响退出码、不计 REGRESSION）。
    pub strict_replay: bool,
    /// `--baseline <file>`：棘轮基线（gates.json 格式）。
    pub baseline: Option<PathBuf>,
    /// `--update-baseline`：无 REGRESSION 时把本次结果合并进基线。
    pub update_baseline: bool,
}

/// 退出码：有 REGRESSION 为 2，否则有计分项失败为 1，全部通过为 0（未纳入的 Mooneye 项不影响）。
pub fn exit_code(results: &[RomResult], regressions: &[String], other_errors: bool) -> u8 {
    if !regressions.is_empty() {
        2
    } else if other_errors || results.iter().any(|r| r.scored && !r.pass) {
        1
    } else {
        0
    }
}

/// stdout 行数上限。
pub const MAX_STDOUT_LINES: usize = 30;

/// 生成 stdout 文本（第一行汇总，之后每个失败项一行 `ERROR`，总行数不超过 30）。
pub fn stdout_lines(results: &[RomResult], extra_first: &[String]) -> Vec<String> {
    let mut lines = vec![summary_line(results)];
    let mut body: Vec<String> = extra_first.to_vec();
    for r in results.iter().filter(|r| r.scored && !r.pass) {
        body.push(format!("ERROR {} {}", r.rom, judge::clip(&r.reason, 110)));
    }
    let room = MAX_STDOUT_LINES - 1;
    if body.len() > room {
        let hidden = body.len() - (room - 1);
        body.truncate(room - 1);
        body.push(format!("... {hidden} more lines omitted (see reports/gates.json)"));
    }
    lines.extend(body);
    lines
}

