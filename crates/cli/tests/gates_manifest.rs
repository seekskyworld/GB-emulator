//! gates 清单、超时、输出约束与运行器健壮性（ROM 缺失时 SKIP）。

use std::path::PathBuf;

use gb_cli::gates::{self, manifest, run_entry, select, stdout_lines, Kind};

#[test]
fn manifest_has_18_roms_in_4_groups() {
    let m = manifest();
    assert_eq!(m.len(), 18);
    let count = |g: &str| m.iter().filter(|e| e.group == g).count();
    assert_eq!((count("G1"), count("G2"), count("G3"), count("G4")), (12, 1, 4, 1));
    assert!(m.iter().all(|e| !e.rom.contains("mem_timing-2")), "mem_timing-2 is not a gate");
    let collections: Vec<&str> = m.iter().filter(|e| e.is_collection()).map(|e| e.rom.as_str()).collect();
    assert_eq!(collections, vec!["blargg/cpu_instrs/cpu_instrs.gb", "blargg/mem_timing/mem_timing.gb"]);
    assert_eq!(m.iter().filter(|e| e.kind == Kind::Acid2).count(), 1);
    // 名字唯一（日志与截图文件名）
    let mut names: Vec<String> = m.iter().map(|e| e.name()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 18);
}

#[test]
fn fast_and_only_filters() {
    assert_eq!(select(None, true).len(), 16);
    assert!(select(None, true).iter().all(|e| !e.is_collection()));
    let only = vec!["G1".to_string(), "G4".to_string()];
    let s = select(Some(&only), false);
    assert_eq!(s.len(), 13);
    assert!(s.iter().all(|e| e.group == "G1" || e.group == "G4"));
    assert_eq!(select(Some(&only), true).len(), 12);
}

#[test]
fn timeouts_in_simulated_frames() {
    let m = manifest();
    let ind = m.iter().find(|e| e.name() == "01-special").unwrap();
    let col = m.iter().find(|e| e.name() == "cpu_instrs").unwrap();
    let acid = m.iter().find(|e| e.kind == Kind::Acid2).unwrap();
    assert_eq!(ind.max_frames(), 3584); // ceil(60 * 59.73)
    assert_eq!(col.max_frames(), 10752); // ceil(180 * 59.73)
    assert_eq!(acid.max_frames(), 120);
}

fn refs() -> Option<PathBuf> {
    let r = PathBuf::from(std::env::var_os("GBEMU_REFS")?);
    r.join("blargg").is_dir().then_some(r)
}

#[test]
fn all_manifest_roms_exist_in_refs() {
    let Some(r) = refs() else {
        println!("SKIP: GBEMU_REFS not set or has no blargg/");
        return;
    };
    for e in manifest() {
        assert!(r.join(&e.rom).is_file(), "missing {}", e.rom);
    }
    let gray = std::fs::metadata(r.join("acid2/reference-dmg.gray")).unwrap();
    assert_eq!(gray.len(), 23040);
}

#[test]
fn runner_never_panics_and_missing_rom_fails() {
    let dir = std::env::temp_dir().join(format!("gbcli-norefs-{}", std::process::id()));
    for e in manifest() {
        let r = run_entry(&e, &dir);
        assert!(!r.pass, "{}: missing ROM must fail", e.rom);
        assert!(r.reason.contains("cannot read"), "{}", r.reason);
    }
    // 真实 acid2（上限 120 帧）：无论核心实现到哪一步，都必须返回结果而不是让 panic 逃出
    if let Some(r) = refs() {
        let e = manifest().into_iter().find(|e| e.kind == Kind::Acid2).unwrap();
        let res = run_entry(&e, &r);
        assert!(res.frames <= 120);
        if !res.pass {
            assert!(!res.reason.is_empty());
        }
    }
}

#[test]
fn stdout_is_capped_at_30_lines() {
    let dir = std::env::temp_dir().join(format!("gbcli-norefs2-{}", std::process::id()));
    let results: Vec<_> = manifest().iter().map(|e| run_entry(e, &dir)).collect();
    let regs: Vec<String> = results.iter().map(|r| format!("REGRESSION {}", r.rom)).collect();
    let lines = stdout_lines(&results, &regs);
    assert!(lines.len() <= gates::MAX_STDOUT_LINES, "{} lines", lines.len());
    assert_eq!(lines[0], "G1 0/12 G2 0/1 G3 0/4 G4 0/1");
    assert!(lines[1].starts_with("REGRESSION "), "REGRESSION lines come first");
    assert!(lines.last().unwrap().contains("omitted"));
    // 不超限时：1 行汇总 + 18 行 ERROR
    let lines = stdout_lines(&results, &[]);
    assert_eq!(lines.len(), 19);
    assert!(lines[1..].iter().all(|l| l.starts_with("ERROR ")));
}
