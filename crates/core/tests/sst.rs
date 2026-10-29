//! SingleStepTests/sm83 v1 接入（T12；PLAN §4.2、DECISIONS D015）。
//!
//! 读取 `$GBEMU_REFS/sst/v1/*.json`（默认 `<repo>/refs`；目录缺失时打印 SKIP 并通过）。每组用例：
//! 按 `initial` 设置寄存器、IME 与 RAM（平铺 64 KiB 假总线，`pending()` 恒为 0），执行 1 次 `step`，
//! 比较 `final` 的寄存器（含 EI 的 `ei` 挂起标志）与 RAM，以及 `cycles` 中逐 M-cycle 的访问种类、
//! 读写的地址与数据（`"---"` 为内部周期，地址/数据只是总线残留值，不比较）。
//!
//! `SST_ONLY=<文件名去掉 .json>`（可用逗号分隔多个，如 `SST_ONLY="cb 46,00"`）只跑指定文件。
//! 失败时打印每个失败文件的首个不一致用例与字段。看汇总行：`cargo test --test sst -- --nocapture`。

#[path = "cpu_common.rs"]
mod common;

use common::json::{parse, Json};
use common::{Access, FlatBus};
use gb_core::cpu::Cpu;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// 与 SST 约定不一致、按 Pan Docs / gbz80(7) / Opcodes.json 实现的指令：只做部分比较。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exception {
    /// 76 HALT：SST 记录 3 个 M-cycle（取指 + 2 个内部周期）；本实现 HALT 指令本身 1 M-cycle
    /// （Opcodes.json 为 4 T），停机期间每次 `step` 各 1 个 idle。比较寄存器、RAM 与首个取指周期，不比较周期数。
    Halt,
    /// 10 STOP：SST 记录 PC+1、3 个 M-cycle；本实现按 Pan Docs / gbz80(7) / Opcodes.json 为 2 字节、
    /// 1 M-cycle（跳过第二个字节）。比较除 PC 与周期数之外的全部字段及首个取指周期。
    Stop,
}

fn exception_for(stem: &str) -> Option<Exception> {
    match stem {
        "76" => Some(Exception::Halt),
        "10" => Some(Exception::Stop),
        _ => None,
    }
}

fn sst_dir() -> Option<PathBuf> {
    let refs = std::env::var_os("GBEMU_REFS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../refs"));
    let dir = refs.join("sst").join("v1");
    dir.is_dir().then_some(dir)
}

fn u(o: &Json, k: &str) -> u64 {
    o.field(k).as_u64()
}

fn run_case(t: &Json, exc: Option<Exception>) -> Result<(), String> {
    let name = t.field("name").as_str();
    let ini = t.field("initial");
    let fin = t.field("final");

    let mut cpu = Cpu::new();
    cpu.a = u(ini, "a") as u8;
    cpu.f = u(ini, "f") as u8;
    cpu.b = u(ini, "b") as u8;
    cpu.c = u(ini, "c") as u8;
    cpu.d = u(ini, "d") as u8;
    cpu.e = u(ini, "e") as u8;
    cpu.h = u(ini, "h") as u8;
    cpu.l = u(ini, "l") as u8;
    cpu.sp = u(ini, "sp") as u16;
    cpu.pc = u(ini, "pc") as u16;
    // `initial.ie` 不是 IE 寄存器（D015），忽略。
    cpu.ime = u(ini, "ime") != 0;
    let mut bus = FlatBus::new();
    for pair in ini.field("ram").as_arr() {
        let p = pair.as_arr();
        bus.mem[p[0].as_u64() as usize] = p[1].as_u64() as u8;
    }

    cpu.step(&mut bus);

    let check = |field: &str, got: u64, want: u64| -> Result<(), String> {
        if got == want {
            Ok(())
        } else {
            Err(format!("case {name:?}: {field} = {got:#04x}, expected {want:#04x}"))
        }
    };
    for (k, got) in [
        ("a", cpu.a),
        ("f", cpu.f),
        ("b", cpu.b),
        ("c", cpu.c),
        ("d", cpu.d),
        ("e", cpu.e),
        ("h", cpu.h),
        ("l", cpu.l),
    ] {
        check(k, got as u64, u(fin, k))?;
    }
    if exc != Some(Exception::Stop) {
        check("pc", cpu.pc as u64, u(fin, "pc"))?;
    }
    check("sp", cpu.sp as u64, u(fin, "sp"))?;
    check("ime", cpu.ime as u64, u(fin, "ime"))?;
    check("ei (EI pending)", cpu.ei_pending as u64, fin.get("ei").map_or(0, Json::as_u64))?;
    for pair in fin.field("ram").as_arr() {
        let p = pair.as_arr();
        let addr = p[0].as_u64() as usize;
        check(&format!("ram[{addr:04X}]"), bus.mem[addr] as u64, p[1].as_u64())?;
    }

    let want = t.field("cycles").as_arr();
    let got = &bus.log;
    let n = match exc {
        Some(_) => 1,
        None => {
            if got.len() != want.len() {
                return Err(format!("case {name:?}: {} M-cycles {got:?}, expected {} {want:?}", got.len(), want.len()));
            }
            want.len()
        }
    };
    for (i, w) in want.iter().enumerate().take(n) {
        let ok = match (w, got.get(i)) {
            (Json::Null, Some(Access::Idle)) => true,
            (Json::Arr(c), Some(acc)) => match (c[2].as_str(), *acc) {
                ("r-m", Access::Read(a, v)) => a as u64 == c[0].as_u64() && v as u64 == c[1].as_u64(),
                ("-wm", Access::Write(a, v)) => a as u64 == c[0].as_u64() && v as u64 == c[1].as_u64(),
                ("---", Access::Idle) => true,
                _ => false,
            },
            _ => false,
        };
        if !ok {
            return Err(format!("case {name:?}: M-cycle {} is {:?}, expected {w:?} (all: {got:?})", i + 1, got.get(i)));
        }
    }
    Ok(())
}

/// 跑一个文件：返回用例数，或首个失败（附失败用例总数）。
fn run_file(path: &Path) -> Result<usize, String> {
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let exc = exception_for(&stem);
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let root = parse(&text)?;
    let cases = root.as_arr();
    let mut first = None;
    let mut failed = 0;
    for t in cases {
        if let Err(e) = run_case(t, exc) {
            failed += 1;
            first.get_or_insert(e);
        }
    }
    match first {
        None if cases.is_empty() => Err("no cases".into()),
        None => Ok(cases.len()),
        Some(e) => Err(format!("{failed}/{} cases fail; first: {e}", cases.len())),
    }
}

#[test]
fn single_step_tests() {
    let Some(dir) = sst_dir() else {
        println!("SKIP sst: <GBEMU_REFS>/sst/v1 not found (run tools/fetch_refs.sh or set GBEMU_REFS)");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read sst dir")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    if let Ok(only) = std::env::var("SST_ONLY") {
        let wanted: Vec<String> = only.split(',').map(|s| s.trim().to_ascii_lowercase()).collect();
        files.retain(|p| wanted.contains(&p.file_stem().unwrap().to_string_lossy().to_ascii_lowercase()));
        assert!(!files.is_empty(), "SST_ONLY={only:?} matched no file in {}", dir.display());
    }

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(String, Result<usize, String>)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = files.get(i) else { break };
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                let r = run_file(path);
                results.lock().unwrap().push((stem, r));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));

    let passed = results.iter().filter(|r| r.1.is_ok()).count();
    let cases: usize = results.iter().filter_map(|r| r.1.as_ref().ok()).sum();
    let partial: Vec<&str> = results.iter().filter(|r| exception_for(&r.0).is_some()).map(|r| r.0.as_str()).collect();
    println!(
        "SST: {passed}/{} files passed ({cases} cases); partial-check exceptions: {:?}",
        results.len(),
        partial
    );
    let failures: Vec<String> = results.iter().filter_map(|(f, r)| r.as_ref().err().map(|e| format!("ERROR sst {f}: {e}"))).collect();
    for line in failures.iter().take(40) {
        println!("{line}");
    }
    assert!(failures.is_empty(), "{} SST files failed:\n{}", failures.len(), failures.join("\n"));
}

/// 对抗自检：比较逻辑不能是空转的——构造的错误期望必须被判为失败。
#[test]
fn harness_detects_mismatches() {
    // LD (HL),A（0x77）：取指 + 写 1 次。
    let good = r#"{"name":"77 self","initial":{"pc":256,"sp":65534,"a":171,"b":0,"c":0,"d":0,"e":0,"f":176,"h":192,"l":0,"ime":0,"ie":0,"ram":[[256,119]]},
        "final":{"a":171,"b":0,"c":0,"d":0,"e":0,"f":176,"h":192,"l":0,"pc":257,"sp":65534,"ime":0,"ram":[[256,119],[49152,171]]},
        "cycles":[[256,119,"r-m"],[49152,171,"-wm"]]}"#;
    let case = |s: &str| parse(s).unwrap();
    assert_eq!(run_case(&case(good), None), Ok(()));
    let bad = [
        good.replace(r#""pc":257"#, r#""pc":258"#),
        good.replace(r#""f":176,"h":192,"l":0,"pc""#, r#""f":160,"h":192,"l":0,"pc""#),
        good.replace("[49152,171]]", "[49152,170]]"),
        good.replace(r#"[49152,171,"-wm"]"#, r#"[49152,171,"r-m"]"#),
        good.replace(r#"[49152,171,"-wm"]"#, r#"[49153,171,"-wm"]"#),
        good.replace(r#"[49152,171,"-wm"]"#, r#"[49152,171,"-wm"],[0,0,"---"]"#),
        good.replace(r#""ime":0,"ram":[[256,119],"#, r#""ime":0,"ei":1,"ram":[[256,119],"#),
    ];
    for b in &bad {
        assert!(run_case(&case(b), None).is_err(), "mismatch not detected:\n{b}");
    }
    // 内部周期的地址/数据只是总线残留值，不比较。
    let nop = r#"{"name":"03 self","initial":{"pc":256,"sp":0,"a":0,"b":0,"c":0,"d":0,"e":0,"f":0,"h":0,"l":0,"ime":0,"ie":0,"ram":[[256,3]]},
        "final":{"a":0,"b":0,"c":1,"d":0,"e":0,"f":0,"h":0,"l":0,"pc":257,"sp":0,"ime":0,"ram":[[256,3]]},
        "cycles":[[256,3,"r-m"],[1234,56,"---"]]}"#;
    assert_eq!(run_case(&case(nop), None), Ok(()));
}
