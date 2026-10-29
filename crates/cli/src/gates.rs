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
/// gates.json 里每个 ROM 保存的串口文本上限。
pub const SERIAL_JSON_MAX: usize = 2048;

pub fn secs_to_frames(secs: f64) -> u64 {
    (secs * FPS).ceil() as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Blargg { collection: bool },
    Acid2,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub group: &'static str,
    /// 相对 `$GBEMU_REFS` 的路径。
    pub rom: String,
    pub kind: Kind,
}

impl Entry {
    /// 文件名去掉扩展名（如 `03-op sp,hl`），用作日志与截图文件名。
    pub fn name(&self) -> String {
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
    let blargg = |group, rom: String, collection| Entry { group, rom, kind: Kind::Blargg { collection } };
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
    v.push(Entry { group: "G4", rom: "acid2/dmg-acid2.gb".into(), kind: Kind::Acid2 });
    v
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
}

fn finish(e: &Entry, p: Progress, end: RunEnd, verdict: Verdict, diff: Option<DiffStats>) -> RomResult {
    RomResult {
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
    let r = catch(|| -> Result<RunEnd, String> {
        let mut m = Machine::new(rom).map_err(|err| format!("cart: {err}"))?;
        let mut stepper = FrameStepper::new();
        let mut seen_at: Option<u64> = None;
        let mut end = RunEnd::Timeout { frames: max };
        while p.frames < max {
            stepper.run_frame(&mut m, &mut p.trace)?;
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
    if parts.is_empty() {
        "(no gates selected)".to_string()
    } else {
        parts.join(" ")
    }
}

/// 串口文本前 `max` 字节（按 UTF-8 宽松解码）。
pub fn serial_head(serial: &[u8], max: usize) -> String {
    String::from_utf8_lossy(&serial[..serial.len().min(max)]).into_owned()
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
    Value::Obj(vec![
        ("summary".to_string(), s(&summary_line(results))),
        ("fast".to_string(), Value::Bool(fast)),
        (
            "only".to_string(),
            match only {
                None => Value::Null,
                Some(g) => Value::Arr(g.iter().map(|x| s(x)).collect()),
            },
        ),
        ("results".to_string(), Value::Arr(items)),
    ])
}

/// gates 命令的选项。
#[derive(Debug, Clone)]
pub struct Options {
    pub fast: bool,
    pub only: Option<Vec<String>>,
    pub threads: usize,
    pub refs: PathBuf,
    pub out: PathBuf,
}

/// stdout 行数上限。
pub const MAX_STDOUT_LINES: usize = 30;

/// 生成 stdout 文本（第一行汇总，之后每个失败项一行 `ERROR`，总行数不超过 30）。
pub fn stdout_lines(results: &[RomResult], extra_first: &[String]) -> Vec<String> {
    let mut lines = vec![summary_line(results)];
    let mut body: Vec<String> = extra_first.to_vec();
    for r in results.iter().filter(|r| !r.pass) {
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

/// 写出报告：`<out>/gates.json`。
pub fn write_reports(results: &[RomResult], opts: &Options) -> std::io::Result<()> {
    std::fs::create_dir_all(&opts.out)?;
    let json = results_json(results, opts.fast, opts.only.as_deref());
    std::fs::write(opts.out.join("gates.json"), json.to_pretty())?;
    Ok(())
}
