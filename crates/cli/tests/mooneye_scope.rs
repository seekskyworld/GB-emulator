//! T70：Mooneye 纳入规则（每一类后缀与被排除的目录）与判定的对抗单测。

use gb_cli::gates::{mooneye_manifest, mooneye_scope, Kind, MOONEYE};
use gb_cli::judge::{judge_mooneye, RunEnd, Verdict, MOONEYE_PASS_REGS};

fn included(rel: &str) -> bool {
    mooneye_scope(rel).0
}

#[test]
fn suffix_rules() {
    // 纳入
    for rel in [
        "acceptance/add_sp_e_timing.gb",                 // 无后缀
        "acceptance/timer/tim00_div_trigger.gb",         // 无后缀（下划线不是分隔符）
        "acceptance/di_timing-GS.gb",                    // -GS
        "acceptance/halt_ime1_timing2-GS.gb",            // -GS
        "acceptance/bits/unused_hwio-GS.gb",             // 子目录 + -GS
        "acceptance/boot_regs-dmgABC.gb",                // -dmgABC
        "acceptance/boot_div-dmgABCmgb.gb",              // -dmgABCmgb
        "acceptance/serial/boot_sclk_align-dmgABCmgb.gb", // -dmgABCmgb
        "emulator-only/mbc1/rom_1Mb.gb",                 // emulator-only/ 无后缀
        "acceptance/x-G.gb",                             // 只有 G
    ] {
        assert!(included(rel), "{rel} should be included: {:?}", mooneye_scope(rel));
    }
    // 排除：其余后缀
    for rel in [
        "acceptance/boot_div-dmg0.gb",
        "acceptance/boot_regs-mgb.gb",
        "acceptance/boot_div-S.gb",
        "acceptance/boot_div2-S.gb", // 最后一个 '-' 之后才是后缀
        "acceptance/boot_regs-sgb.gb",
        "acceptance/boot_regs-sgb2.gb",
        "acceptance/boot_regs-cgb.gb",
        "acceptance/boot_div-cgbABCDE.gb",
        "acceptance/boot_div-A.gb",
        "acceptance/unused_hwio-C.gb",
    ] {
        let (inc, why) = mooneye_scope(rel);
        assert!(!inc, "{rel} should be excluded");
        assert!(why.contains("suffix"), "{rel}: {why}");
    }
}

#[test]
fn excluded_directories() {
    for rel in [
        "manual-only/sprite_priority.gb",
        "madness/mgb_oam_dma_halt_sprites.gb",
        "utils/bootrom_dumper.gb",
        "utils/dump_boot_hwio.gb",
        "misc/boot_div-A.gb",
        "misc/bits/unused_hwio-C.gb",
        "misc/no_suffix.gb", // 目录排除优先于后缀
        "manual-only/x-GS.gb",
        "top_level.gb",
    ] {
        let (inc, why) = mooneye_scope(rel);
        assert!(!inc, "{rel} should be excluded");
        assert!(why.contains("directory"), "{rel}: {why}");
    }
}

#[test]
fn real_suite_counts() {
    let Some(refs) = std::env::var_os("GBEMU_REFS").map(std::path::PathBuf::from) else {
        println!("SKIP: GBEMU_REFS not set");
        return;
    };
    let Ok(m) = mooneye_manifest(&refs) else {
        println!("SKIP: mooneye suite missing");
        return;
    };
    assert_eq!(m.len(), 115);
    assert!(m.iter().all(|e| e.group == MOONEYE && e.kind == Kind::Mooneye && e.max_frames() == 1792));
    let inc: Vec<String> = m.iter().filter(|e| e.scored).map(|e| e.name()).collect();
    assert!(inc.iter().all(|n| n.starts_with("acceptance/") || n.starts_with("emulator-only/")));
    for n in ["acceptance/boot_regs-dmgABC", "acceptance/di_timing-GS", "emulator-only/mbc5/rom_64Mb"] {
        assert!(inc.iter().any(|x| x == n), "{n}");
    }
    for n in ["acceptance/boot_div2-S", "acceptance/boot_regs-mgb", "misc/boot_div-A", "utils/bootrom_dumper"] {
        assert!(!inc.iter().any(|x| x == n), "{n}");
    }
    // 名字唯一（rom_1Mb 等在多个目录中重名，所以名字带目录）
    let mut names: Vec<String> = m.iter().map(|e| e.name()).collect();
    names.dedup();
    assert_eq!(names.len(), 115);
}

// ---------- 判定 ----------

#[test]
fn fibonacci_passes() {
    assert_eq!(judge_mooneye(&RunEnd::Finished, Some(MOONEYE_PASS_REGS)), Verdict::Pass);
    assert_eq!(MOONEYE_PASS_REGS, [3, 5, 8, 13, 21, 34]);
}

#[test]
fn forged_results_fail() {
    let fin = RunEnd::Finished;
    // 全为 0x42：测试自报失败
    let v = judge_mooneye(&fin, Some([0x42; 6]));
    assert!(!v.passed() && v.reason().contains("reported failure"), "{v:?}");
    // 只有 5 个寄存器正确（每个位置各试一次）
    for i in 0..6 {
        let mut r = MOONEYE_PASS_REGS;
        r[i] ^= 0x01;
        let v = judge_mooneye(&fin, Some(r));
        assert!(!v.passed() && v.reason().contains("unexpected"), "reg {i}: {v:?}");
    }
    // 顺序错
    assert!(!judge_mooneye(&fin, Some([5, 3, 8, 13, 21, 34])).passed());
    // 全 0（开机后什么都没做就 LD B,B）
    assert!(!judge_mooneye(&fin, Some([0; 6])).passed());
    // 超时、panic、出错，即使寄存器是对的
    let v = judge_mooneye(&RunEnd::Timeout { frames: 1792 }, Some(MOONEYE_PASS_REGS));
    assert!(!v.passed() && v.reason().contains("timeout"), "{v:?}");
    assert!(!judge_mooneye(&RunEnd::Panic("x".into()), Some(MOONEYE_PASS_REGS)).passed());
    assert!(!judge_mooneye(&RunEnd::Error("cart".into()), Some(MOONEYE_PASS_REGS)).passed());
    // 结束但没有取到寄存器
    assert!(!judge_mooneye(&fin, None).passed());
}
