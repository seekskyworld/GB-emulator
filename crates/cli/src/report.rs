//! gates 报告：gates.json、gates.md、logs/<rom>.log、Blargg 截图、acid2 实际图与差异图。

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use gb_core::{FRAME_PIXELS, SCREEN_H, SCREEN_W};

use crate::gates::{mooneye_raw, results_json, Kind, Options, RomResult};
use crate::judge::{DiffStats, RunEnd};
use crate::png;

/// 差异图：参考帧的灰度作底，差异像素标为纯红。`actual` 或 `reference` 长度不对时，越界部分按差异处理。
pub fn diff_image_rgb(actual: &[u8], reference: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_PIXELS * 3);
    for i in 0..FRAME_PIXELS {
        let (a, r) = (actual.get(i), reference.get(i));
        if a != r || a.is_none() {
            out.extend_from_slice(&[0xFF, 0x00, 0x00]);
        } else {
            let g = *r.unwrap();
            out.extend_from_slice(&[g, g, g]);
        }
    }
    out
}

/// 差异明细：前 `max` 个差异像素（x, y, 参考值, 实际值）与按行统计。
pub fn diff_details(actual: &[u8], reference: &[u8], max: usize) -> String {
    let mut s = String::new();
    let mut shown = 0;
    let mut rows = vec![0usize; SCREEN_H];
    for i in 0..FRAME_PIXELS {
        let (a, r) = (actual.get(i).copied(), reference.get(i).copied());
        if a != r {
            rows[i / SCREEN_W] += 1;
            if shown < max {
                let f = |v: Option<u8>| v.map_or("--".to_string(), |v| format!("{v:02X}"));
                let _ = writeln!(s, "  ({:3},{:3}) expected {} actual {}", i % SCREEN_W, i / SCREEN_W, f(r), f(a));
                shown += 1;
            }
        }
    }
    let row_list: Vec<String> =
        rows.iter().enumerate().filter(|(_, &n)| n > 0).map(|(y, n)| format!("{y}:{n}")).collect();
    let _ = writeln!(s, "rows with differences (y:count): {}", row_list.join(" "));
    s
}

fn end_text(end: &RunEnd) -> String {
    match end {
        RunEnd::Finished => "finished".to_string(),
        RunEnd::Timeout { frames } => format!("timeout ({frames} frames)"),
        RunEnd::Panic(m) => format!("panic: {m}"),
        RunEnd::Error(m) => format!("error: {m}"),
    }
}

fn trace_text(trace: &[u16]) -> String {
    if trace.is_empty() {
        return "(empty)".to_string();
    }
    trace.chunks(16).map(|c| c.iter().map(|pc| format!("{pc:04X}")).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n")
}

/// 单个 ROM 的完整日志。
pub fn log_text(r: &RomResult, diff: Option<(&DiffStats, &str)>) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "rom: {}", r.rom);
    let _ = writeln!(s, "group: {}", r.group);
    let _ = writeln!(s, "result: {}", if r.pass { "PASS" } else { "FAIL" });
    if !r.pass {
        let _ = writeln!(s, "reason: {}", r.reason);
    }
    let _ = writeln!(s, "end: {}", end_text(&r.end));
    let _ = writeln!(s, "frames: {}", r.frames);
    if r.kind == Kind::Mooneye {
        let _ = writeln!(s, "scope: {} ({})", if r.scored { "included" } else { "excluded" }, r.scope);
        match r.regs {
            Some(g) => {
                let _ = writeln!(
                    s,
                    "regs at first LD B,B: B={:02X} C={:02X} D={:02X} E={:02X} H={:02X} L={:02X}",
                    g[0], g[1], g[2], g[3], g[4], g[5]
                );
            }
            None => {
                let _ = writeln!(s, "regs at first LD B,B: (not reached)");
            }
        }
    }
    let _ = writeln!(s, "\n--- serial ({} bytes) ---", r.serial.len());
    s.push_str(&r.serial_text());
    if !s.ends_with('\n') {
        s.push('\n');
    }
    let _ = writeln!(s, "\n--- last {} PCs (oldest first, PC before each step) ---", r.trace.len());
    let _ = writeln!(s, "{}", trace_text(&r.trace));
    if let Some((d, details)) = diff {
        let _ = writeln!(s, "\n--- frame diff vs reference ---");
        let _ = writeln!(s, "{}", d.describe());
        s.push_str(details);
    }
    s
}

/// 文件名安全化：只替换路径分隔符（保留空格与逗号，与 ROM 文件名一致）。
fn file_stem(name: &str) -> String {
    name.replace(['/', '\\'], "_")
}

fn gates_md(results: &[RomResult], opts: &Options, regressions: &[String]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# gates\n");
    let _ = writeln!(s, "**{}**\n", crate::gates::summary_with_replay(results, opts.replay.as_ref()));
    if let Some(st) = opts.replay.as_ref().filter(|st| !st.detail().is_empty()) {
        let _ = writeln!(s, "{} {}: {}\n", crate::replay::GROUP, st.word(), st.detail());
    }
    let mut flags = Vec::new();
    if opts.fast {
        flags.push("--fast".to_string());
    }
    if let Some(o) = &opts.only {
        flags.push(format!("--only {}", o.join(",")));
    }
    if let Some(b) = &opts.baseline {
        flags.push(format!("--baseline {}", b.display()));
    }
    let _ = writeln!(s, "options: `{}`\n", if flags.is_empty() { "(none)".to_string() } else { flags.join(" ") });
    if !regressions.is_empty() {
        let _ = writeln!(s, "## REGRESSION\n");
        for r in regressions {
            let _ = writeln!(s, "- `{r}`");
        }
        s.push('\n');
    }
    let _ = writeln!(s, "| group | rom | result | frames | reason |");
    let _ = writeln!(s, "| --- | --- | --- | --- | --- |");
    for r in results.iter().filter(|r| r.kind != Kind::Mooneye) {
        let reason = r.reason.replace('|', "\\|");
        let _ = writeln!(
            s,
            "| {} | `{}` | {} | {} | {} |",
            r.group,
            r.rom,
            if r.pass { "PASS" } else { "FAIL" },
            r.frames,
            reason
        );
    }
    if let Some(raw) = mooneye_raw(results) {
        let scored: Vec<&RomResult> = results.iter().filter(|r| r.kind == Kind::Mooneye && r.scored).collect();
        let _ = writeln!(s, "\n## Mooneye\n");
        let _ = writeln!(
            s,
            "scored (DMG-ABC scope, see `mooneye-scope.md`): **{}/{}**; raw (all ROMs): **{raw}**\n",
            scored.iter().filter(|r| r.pass).count(),
            scored.len()
        );
        let _ = writeln!(s, "| rom | in scope | result | frames | reason |");
        let _ = writeln!(s, "| --- | --- | --- | --- | --- |");
        for r in results.iter().filter(|r| r.kind == Kind::Mooneye) {
            let _ = writeln!(
                s,
                "| `{}` | {} | {} | {} | {} |",
                r.name,
                if r.scored { "yes" } else { "no" },
                if r.pass { "PASS" } else { "FAIL" },
                r.frames,
                r.reason.replace('|', "\\|")
            );
        }
    }
    let _ = writeln!(s, "\nlogs: `logs/<rom>.log`; screenshots: `blargg/<rom>.png`, `acid2-actual.png`, `acid2-diff.png` (on failure).");
    s
}

/// 写出全部报告。`reference` 为 acid2 参考灰度（读不到时为 None）。
pub fn write_reports(
    results: &[RomResult],
    opts: &Options,
    reference: Option<&[u8]>,
    regressions: &[String],
) -> io::Result<()> {
    let out = &opts.out;
    std::fs::create_dir_all(out.join("logs"))?;
    let mut json = results_json(results, opts.fast, opts.only.as_deref());
    crate::gates::attach_replay(&mut json, results, opts.replay.as_ref(), opts.strict_replay);
    std::fs::write(out.join("gates.json"), json.to_pretty())?;
    std::fs::write(out.join("gates.md"), gates_md(results, opts, regressions))?;

    for r in results {
        let is_acid2 = r.kind == Kind::Acid2;
        let mut details = String::new();
        let diff = match (r.diff.as_ref(), r.frame.as_deref(), reference) {
            (Some(d), Some(f), Some(rf)) if d.count > 0 => {
                details = diff_details(f, rf, 32);
                Some(d)
            }
            (Some(d), _, _) => Some(d),
            _ => None,
        };
        let log = log_text(r, diff.map(|d| (d, details.as_str())));
        let log_dir = if r.kind == Kind::Mooneye { out.join("logs/mooneye") } else { out.join("logs") };
        std::fs::create_dir_all(&log_dir)?;
        std::fs::write(log_dir.join(format!("{}.log", file_stem(&r.name))), log)?;
        if r.kind == Kind::Mooneye {
            continue;
        }

        let frame_ok = r.frame.as_ref().is_some_and(|f| f.len() == FRAME_PIXELS);
        if is_acid2 {
            let actual_png = out.join("acid2-actual.png");
            let diff_png = out.join("acid2-diff.png");
            if r.pass {
                remove_stale(&actual_png)?;
                remove_stale(&diff_png)?;
            } else if frame_ok {
                let f = r.frame.as_deref().unwrap();
                png::write_gray(&actual_png, SCREEN_W as u32, SCREEN_H as u32, f)?;
                match reference {
                    Some(rf) => png::write_rgb(&diff_png, SCREEN_W as u32, SCREEN_H as u32, &diff_image_rgb(f, rf))?,
                    None => remove_stale(&diff_png)?,
                }
            } else {
                remove_stale(&actual_png)?;
                remove_stale(&diff_png)?;
            }
        } else {
            let p = out.join("blargg").join(format!("{}.png", file_stem(&r.name)));
            if frame_ok {
                png::write_gray(&p, SCREEN_W as u32, SCREEN_H as u32, r.frame.as_deref().unwrap())?;
            } else {
                remove_stale(&p)?;
            }
        }
    }
    if results.iter().any(|r| r.kind == Kind::Mooneye) {
        std::fs::write(out.join("mooneye-scope.md"), mooneye_scope_md(results))?;
    }
    Ok(())
}

/// `mooneye-scope.md`：纳入/排除清单及理由（规则见 `gates::mooneye_scope`）。
pub fn mooneye_scope_md(results: &[RomResult]) -> String {
    let all: Vec<&RomResult> = results.iter().filter(|r| r.kind == Kind::Mooneye).collect();
    let inc: Vec<&&RomResult> = all.iter().filter(|r| r.scored).collect();
    let exc: Vec<&&RomResult> = all.iter().filter(|r| !r.scored).collect();
    let mut s = String::new();
    let _ = writeln!(s, "# Mooneye scope (DMG-ABC)\n");
    let _ = writeln!(s, "Test suite: `$GBEMU_REFS/{}`, {} ROMs.\n", crate::gates::MOONEYE_DIR, all.len());
    let _ = writeln!(s, "Rules (PLAN §2.4): only `acceptance/` and `emulator-only/`; the model suffix is the part of the file name after the last `-` and before `.gb`. Included: no suffix, a suffix containing `G` (dmg+mgb group, e.g. `-GS`), or a suffix containing `dmgABC` (e.g. `-dmgABC`, `-dmgABCmgb`). Everything else is excluded (`-dmg0`, `-mgb`, `-S`, `-sgb`, `-sgb2`, `-cgb*`, `-A`, `-C`, and the `manual-only/`, `madness/`, `utils/`, `misc/` directories).\n");
    let pass_inc = inc.iter().filter(|r| r.pass).count();
    let pass_all = all.iter().filter(|r| r.pass).count();
    let _ = writeln!(s, "Included: {} / {}; passing: {pass_inc}/{} (scored), {pass_all}/{} (raw).\n", inc.len(), all.len(), inc.len(), all.len());
    for (title, list) in [("Included", &inc), ("Excluded", &exc)] {
        let _ = writeln!(s, "## {title} ({})\n", list.len());
        let _ = writeln!(s, "| rom | reason | result |");
        let _ = writeln!(s, "| --- | --- | --- |");
        for r in list.iter() {
            let _ = writeln!(s, "| `{}` | {} | {} |", r.name, r.scope, if r.pass { "PASS" } else { "FAIL" });
        }
        s.push('\n');
    }
    s
}

fn remove_stale(p: &Path) -> io::Result<()> {
    match std::fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

