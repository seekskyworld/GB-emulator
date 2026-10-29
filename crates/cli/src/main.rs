//! gb-cli：单 ROM 调试（run）、门槛（gates）、性能（bench）。零依赖。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use gb_cli::emu::{catch, FrameStepper, PcTrace};
use gb_cli::gates::{self, Options};
use gb_cli::{baseline, report};
use gb_cli::png;
use gb_core::{Machine, Regs, SCREEN_H, SCREEN_W};

const USAGE: &str = "\
usage:
  gb-cli run <rom> [--frames N] [--png out.png] [--serial] [--until-ld-b-b] [--trace]
  gb-cli gates [--fast] [--only G1,G4] [--threads N] [--out DIR] [--baseline FILE] [--update-baseline]
  gb-cli bench <rom> [--frames N]
env: GBEMU_REFS (default ./refs), GB_TRACE=1 (same as --trace)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let rest = &args[1..];
    let r = match cmd.as_str() {
        "run" => cmd_run(rest),
        "gates" => cmd_gates(rest),
        "bench" => cmd_bench(rest),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command `{other}`")),
    };
    match r {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("gb-cli: {msg}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// 极简参数解析：位置参数、`--flag`、`--key value`（也接受 `--key=value`）。
struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(raw: &[String], with_value: &[&str], flags: &[&str]) -> Result<Self, String> {
        let mut a = Args { positional: Vec::new(), flags: Vec::new() };
        let mut it = raw.iter();
        while let Some(x) = it.next() {
            if let Some(name) = x.strip_prefix("--") {
                let (name, inline) = match name.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_string())),
                    None => (name, None),
                };
                if with_value.contains(&name) {
                    let v = match inline {
                        Some(v) => v,
                        None => it.next().cloned().ok_or(format!("--{name} needs a value"))?,
                    };
                    a.flags.push((name.to_string(), Some(v)));
                } else if flags.contains(&name) {
                    if inline.is_some() {
                        return Err(format!("--{name} takes no value"));
                    }
                    a.flags.push((name.to_string(), None));
                } else {
                    return Err(format!("unknown option --{name}"));
                }
            } else {
                a.positional.push(x.clone());
            }
        }
        Ok(a)
    }
    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.flags.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
    }
    fn number(&self, name: &str, default: u64) -> Result<u64, String> {
        match self.value(name) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| format!("--{name}: `{v}` is not a number")),
        }
    }
}

fn fmt_regs(r: &Regs) -> String {
    format!(
        "A={:02X} F={:02X} B={:02X} C={:02X} D={:02X} E={:02X} H={:02X} L={:02X} SP={:04X} PC={:04X} IME={} HALT={}",
        r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l, r.sp, r.pc, r.ime as u8, r.halted as u8
    )
}

fn load_machine(path: &str) -> Result<Machine, String> {
    let rom = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    match catch(|| Machine::new(rom)) {
        Ok(Ok(m)) => Ok(m),
        Ok(Err(e)) => Err(format!("cart error: {e}")),
        Err(p) => Err(format!("panic in Machine::new: {p}")),
    }
}

fn cmd_run(raw: &[String]) -> Result<ExitCode, String> {
    let a = Args::parse(raw, &["frames", "png"], &["serial", "until-ld-b-b", "trace"])?;
    let [rom] = a.positional.as_slice() else {
        return Err("run needs exactly one ROM path".into());
    };
    let frames = a.number("frames", 600)?;
    let serial = a.has("serial");
    let until = a.has("until-ld-b-b");
    let trace_on = a.has("trace") || std::env::var("GB_TRACE").is_ok_and(|v| v == "1");

    let mut m = match load_machine(rom) {
        Ok(m) => m,
        Err(e) => {
            println!("ERROR {e}");
            return Ok(ExitCode::FAILURE);
        }
    };
    let mut trace = PcTrace::default();
    let mut done = 0u64;
    let mut hit = false;
    let mut serial_bytes = 0usize;
    let stdout = std::io::stdout();
    let r = catch(|| -> Result<(), String> {
        let mut stepper = FrameStepper::new();
        while done < frames {
            if until {
                hit = stepper.run_frame_until_ld_b_b(&mut m, &mut trace)?;
            } else if trace_on {
                stepper.run_frame(&mut m, &mut trace)?;
            } else {
                m.run_frame();
            }
            if !hit {
                done += 1;
            }
            if serial {
                let s = m.take_serial();
                if !s.is_empty() {
                    serial_bytes += s.len();
                    let mut out = stdout.lock();
                    let _ = out.write_all(&String::from_utf8_lossy(&s).into_owned().into_bytes());
                    let _ = out.flush();
                }
            }
            if hit {
                break;
            }
        }
        Ok(())
    });
    if serial && serial_bytes > 0 {
        println!();
    }
    let mut ok = true;
    match &r {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            println!("ERROR {e}");
            ok = false;
        }
        Err(p) => {
            println!("PANIC {p}");
            ok = false;
        }
    }
    // panic 之后机器状态可能不一致，读取寄存器与画面也包在 catch 里。
    let regs = catch(|| (m.regs(), m.cycles()));
    match regs {
        Ok((regs, cycles)) => {
            println!("frames={done} cycles={cycles}");
            if until {
                if hit {
                    println!("LD B,B hit during frame {}", done + 1);
                } else if ok {
                    println!("LD B,B not hit within {frames} frames");
                    ok = false;
                }
            }
            println!("{}", fmt_regs(&regs));
        }
        Err(p) => println!("PANIC reading registers: {p}"),
    }
    if trace_on || until {
        println!("last PCs (oldest first): {}", trace.format());
    }
    if let Some(path) = a.value("png") {
        match catch(|| m.frame().to_vec()) {
            Ok(f) => match png::write_gray(Path::new(path), SCREEN_W as u32, SCREEN_H as u32, &f) {
                Ok(()) => println!("wrote {path}"),
                Err(e) => {
                    println!("ERROR cannot write {path}: {e}");
                    ok = false;
                }
            },
            Err(p) => {
                println!("PANIC reading frame: {p}");
                ok = false;
            }
        }
    }
    Ok(if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn refs_dir() -> PathBuf {
    match std::env::var_os("GBEMU_REFS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => std::env::current_dir().unwrap_or_default().join("refs"),
    }
}

fn cmd_gates(raw: &[String]) -> Result<ExitCode, String> {
    let a = Args::parse(raw, &["only", "threads", "out", "baseline"], &["fast", "update-baseline"])?;
    if !a.positional.is_empty() {
        return Err(format!("unexpected argument `{}`", a.positional[0]));
    }
    let only = match a.value("only") {
        None => None,
        Some(v) => {
            let groups: Vec<String> =
                v.split(',').map(|g| g.trim().to_ascii_uppercase()).filter(|g| !g.is_empty()).collect();
            for g in &groups {
                if !gates::GROUPS.contains(&g.as_str()) {
                    return Err(format!("--only: unknown group `{g}` (known: G1,G2,G3,G4)"));
                }
            }
            Some(groups)
        }
    };
    let threads = a.number("threads", 4)?.max(1) as usize;
    let out = match a.value("out") {
        Some(o) => PathBuf::from(o),
        None => std::env::current_dir().unwrap_or_default().join("reports"),
    };
    let opts = Options {
        fast: a.has("fast"),
        only,
        threads,
        refs: refs_dir(),
        out,
        baseline: a.value("baseline").map(PathBuf::from),
        update_baseline: a.has("update-baseline"),
    };

    if !opts.refs.join("blargg").is_dir() {
        println!("ERROR refs missing, run tools/fetch_refs.sh");
        return Ok(ExitCode::FAILURE);
    }
    let baseline_path = opts.baseline.clone().unwrap_or_else(|| opts.out.join("baseline.json"));
    // 基线：--baseline 指定时必须可读（--update-baseline 且文件尚不存在时除外，此时新建）。
    let mut errors: Vec<String> = Vec::new();
    let old_baseline = if opts.baseline.is_some() || (opts.update_baseline && baseline_path.exists()) {
        if !baseline_path.exists() && opts.update_baseline {
            None
        } else {
            match baseline::read(&baseline_path) {
                Ok(v) => Some(v),
                Err(e) => {
                    errors.push(format!("ERROR baseline {e}"));
                    None
                }
            }
        }
    } else {
        None
    };

    let entries = gates::select(opts.only.as_deref(), opts.fast);
    let results = gates::run_all(&entries, &opts.refs, opts.threads);
    let current = gates::results_json(&results, opts.fast, opts.only.as_deref());

    // 有旧基线就比较（包括只给 --update-baseline、默认基线已存在的情况），保证基线只升不降。
    let regressions = match &old_baseline {
        Some(b) => match baseline::regressions(b, &current) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("ERROR baseline {}: {e}", baseline_path.display()));
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let reference = std::fs::read(opts.refs.join("acid2/reference-dmg.gray")).ok();
    if let Err(e) = report::write_reports(&results, &opts, reference.as_deref(), &regressions) {
        errors.push(format!("ERROR cannot write reports to {}: {e}", opts.out.display()));
    }
    let mut notes = Vec::new();
    if opts.update_baseline {
        if !regressions.is_empty() {
            notes.push(format!("baseline not updated: {} REGRESSION", regressions.len()));
        } else if errors.iter().any(|e| e.starts_with("ERROR baseline")) {
            notes.push("baseline not updated: baseline unreadable".to_string());
        } else {
            let written = baseline::merge(old_baseline.as_ref(), &current).and_then(|v| {
                if let Some(dir) = baseline_path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                std::fs::write(&baseline_path, v.to_pretty()).map_err(|e| e.to_string())?;
                Ok(v)
            });
            match written {
                Ok(v) => notes.push(format!(
                    "baseline updated: {} ({})",
                    baseline_path.display(),
                    v.get("summary").and_then(|s| s.as_str()).unwrap_or("")
                )),
                Err(e) => errors.push(format!("ERROR cannot write baseline {}: {e}", baseline_path.display())),
            }
        }
    }

    let mut first: Vec<String> = regressions.iter().map(|r| format!("REGRESSION {r}")).collect();
    first.extend(errors.iter().cloned());
    first.extend(notes);
    for line in gates::stdout_lines(&results, &first) {
        println!("{line}");
    }
    Ok(ExitCode::from(gates::exit_code(&results, &regressions, !errors.is_empty())))
}

fn cmd_bench(raw: &[String]) -> Result<ExitCode, String> {
    let a = Args::parse(raw, &["frames"], &[])?;
    let [rom] = a.positional.as_slice() else {
        return Err("bench needs exactly one ROM path".into());
    };
    let frames = a.number("frames", 3600)?.max(1);
    let mut m = match load_machine(rom) {
        Ok(m) => m,
        Err(e) => {
            println!("ERROR {e}");
            return Ok(ExitCode::FAILURE);
        }
    };
    let start = Instant::now();
    let r = catch(|| {
        for _ in 0..frames {
            m.run_frame();
        }
    });
    let secs = start.elapsed().as_secs_f64();
    if let Err(p) = r {
        println!("PANIC {p}");
        return Ok(ExitCode::FAILURE);
    }
    let fps = frames as f64 / secs.max(1e-9);
    let build = if cfg!(debug_assertions) { " [debug build: use --release for real numbers]" } else { "" };
    println!(
        "bench {rom}: {frames} frames in {secs:.3} s = {fps:.1} fps ({:.1}x realtime){build}",
        fps / gates::FPS
    );
    Ok(ExitCode::SUCCESS)
}
