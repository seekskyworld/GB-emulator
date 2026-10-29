//! T63：POKERED-REPLAY（确定性回放基线）。pokered 缺失时相关用例打印 SKIP 并通过。

use std::path::{Path, PathBuf};

use gb_cli::json;
use gb_cli::replay::{self, compare, from_json, gate, parse_script, run, to_json, Status, Step};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pokered_path() -> PathBuf {
    match std::env::var_os("GBEMU_POKERED") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => workspace().join("roms/pokered.gb"),
    }
}

fn pokered() -> Option<Vec<u8>> {
    match std::fs::read(pokered_path()) {
        Ok(r) => Some(r),
        Err(_) => {
            println!("SKIP: pokered not found at {} (set GBEMU_POKERED)", pokered_path().display());
            None
        }
    }
}

fn script_path() -> PathBuf {
    workspace().join("tools/pokered-replay-actions.json")
}

fn script() -> Vec<Step> {
    parse_script(&std::fs::read_to_string(script_path()).unwrap()).unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gbcli-replay-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------- 不依赖 ROM ----------

#[test]
fn script_parsing() {
    let s = parse_script(r#"[{"buttons":"start","hold":8,"wait":12},{"buttons":"down+A","hold":4,"repeat":3,"note":"x"},{"wait":30}]"#)
        .unwrap();
    assert_eq!(s.len(), 3);
    assert_eq!(s[0], Step { mask: gb_core::buttons::START, hold: 8, wait: 12, repeat: 1 });
    assert_eq!(s[1].mask, gb_core::buttons::DOWN | gb_core::buttons::A);
    assert_eq!(s[1].frames(), 12);
    assert_eq!(s[2], Step { mask: 0, hold: 0, wait: 30, repeat: 1 });
    for bad in [
        "{}",
        "[]",
        r#"[{"wait":0}]"#,
        r#"[{"buttons":"jump","hold":1}]"#,
        r#"[{"buttons":"a"}]"#,
        r#"[{"buttons":"a","hold":-1}]"#,
        r#"[{"buttons":"a","hold":1.5}]"#,
        r#"[3]"#,
    ] {
        assert!(parse_script(bad).is_err(), "{bad}");
    }
    // 注释与排版不影响脚本指纹
    let a = parse_script(r#"[{"buttons":"a","hold":4,"wait":2,"note":"one"}]"#).unwrap();
    let b = parse_script(r#"[ {"note":"two", "wait":2, "hold":4, "buttons":"A"} ]"#).unwrap();
    assert_eq!(replay::steps_hash(&a), replay::steps_hash(&b));
}

#[test]
fn repo_script_is_valid_and_about_4500_frames() {
    let s = script();
    let total: u64 = s.iter().map(Step::frames).sum();
    assert!((3000..=5000).contains(&total), "{total} frames");
}

#[test]
fn missing_rom_is_skip() {
    let st = gate(Path::new("/nonexistent/pokered.gb"), &script_path(), &workspace().join("reports/pokered-replay.json"));
    assert!(matches!(st, Status::Skip(_)), "{st:?}");
    assert_eq!(st.word(), "skip");
}

/// 合成 ROM：主循环选中功能键、读 P1，写到 wPartyCount 的地址（D163），让按键反映到检查点里。
fn synth_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    let code: [u8; 11] = [
        0x3E, 0x10, //       0100 LD A,10       ; P1 bit5=0：选功能键
        0xE0, 0x00, //       0102 LDH (00),A
        0xF0, 0x00, //       0104 LDH A,(00)
        0xEA, 0x63, 0xD1, // 0106 LD (D163),A
        0x18, 0xF5, //       0109 JR 0100
    ];
    rom[0x100..0x100 + code.len()].copy_from_slice(&code);
    rom[0x134..0x13B].copy_from_slice(b"REPLAYT");
    rom
}

#[test]
fn synthetic_rom_is_deterministic_and_records_buttons() {
    let steps = parse_script(r#"[{"buttons":"a","hold":300,"wait":300},{"buttons":"start","hold":300}]"#).unwrap();
    let a = run(synth_rom(), &steps).unwrap();
    let b = run(synth_rom(), &steps).unwrap();
    assert_eq!(a.checkpoints, b.checkpoints);
    assert_eq!(a.checkpoints.iter().map(|c| c.frame).collect::<Vec<_>>(), vec![300, 600, 900]);
    // 第 300 帧按着 A（P1 低位 bit0 为 0），第 600 帧松开，第 900 帧按着 START（bit3 为 0）
    assert_eq!(a.checkpoints[0].party_count & 0x0F, 0x0E);
    assert_eq!(a.checkpoints[1].party_count & 0x0F, 0x0F);
    assert_eq!(a.checkpoints[2].party_count & 0x0F, 0x07);
    assert_eq!(compare(&a, &b), None);
}

#[test]
fn compare_reports_first_mismatch_with_frame_and_field() {
    let steps = parse_script(r#"[{"buttons":"a","hold":300,"wait":600}]"#).unwrap();
    let rec = run(synth_rom(), &steps).unwrap();
    let text = to_json(&rec, "x.json").to_pretty();
    let back = from_json(&json::parse(&text).unwrap()).unwrap();
    assert_eq!(compare(&back, &rec), None, "JSON round trip");

    let mut t = back.clone();
    t.checkpoints[1].frame_hash ^= 1;
    let d = compare(&t, &rec).unwrap();
    assert!(d.starts_with("frame 600:") && d.contains("frame_hash"), "{d}");
    let mut t = back.clone();
    t.checkpoints[2].x = 9;
    t.checkpoints[2].map = 0x25;
    let d = compare(&t, &rec).unwrap();
    assert!(d.starts_with("frame 900:") && d.contains("map") && d.contains("x "), "{d}");
    let mut t = back.clone();
    t.checkpoints.pop();
    assert!(compare(&t, &rec).unwrap().contains("length"));
    let mut t = back.clone();
    t.script_hash ^= 1;
    assert!(compare(&t, &rec).unwrap().contains("script"));
    let mut t = back;
    t.rom.push('!');
    assert!(compare(&t, &rec).unwrap().contains("ROM"));
}

// ---------- pokered ----------

#[test]
fn pokered_same_script_twice_gives_identical_checkpoints() {
    let Some(rom) = pokered() else { return };
    let steps = script();
    let a = run(rom.clone(), &steps).unwrap();
    let b = run(rom, &steps).unwrap();
    assert_eq!(a.checkpoints, b.checkpoints);
    assert_eq!(a.last_frame, b.last_frame);
    assert!(a.checkpoints.len() >= 10);
    // 走完开场：最后一个检查点不在标题画面（地图号非 0、主角名已设置、坐标在动）
    let last = a.checkpoints.last().unwrap();
    assert_ne!(last.map, 0);
    assert_eq!(replay::decode_name(&last.player_name), "RED");
    assert!(a.checkpoints.iter().any(|c| (c.x, c.y) != (last.x, last.y)), "position changed during the replay");
}

#[test]
fn pokered_committed_reference_verifies() {
    if pokered().is_none() {
        return;
    }
    let st = gate(&pokered_path(), &script_path(), &workspace().join("reports/pokered-replay.json"));
    assert_eq!(st, Status::Ok);
}

#[test]
fn pokered_tampered_hash_is_changed() {
    let Some(rom) = pokered() else { return };
    let rec = run(rom, &script()).unwrap();
    let mut v = to_json(&rec, "tools/pokered-replay-actions.json");
    // 改参考文件中第 5 个检查点的帧哈希
    let mut tampered_frame = 0.0;
    if let json::Value::Obj(kv) = &mut v {
        for (k, val) in kv.iter_mut() {
            if k == "checkpoints" {
                if let json::Value::Arr(cps) = val {
                    if let json::Value::Obj(c) = &mut cps[4] {
                        for (ck, cv) in c.iter_mut() {
                            if ck == "frame" {
                                if let json::Value::Num(n) = cv {
                                    tampered_frame = *n;
                                }
                            }
                            if ck == "frame_hash" {
                                *cv = json::Value::Str("0123456789abcdef".into());
                            }
                        }
                    }
                }
            }
        }
    }
    let dir = tmp("tamper");
    let good = dir.join("good.json");
    let bad = dir.join("bad.json");
    std::fs::write(&good, to_json(&rec, "x").to_pretty()).unwrap();
    std::fs::write(&bad, v.to_pretty()).unwrap();
    assert_eq!(gate(&pokered_path(), &script_path(), &good), Status::Ok);
    match gate(&pokered_path(), &script_path(), &bad) {
        Status::Changed(d) => {
            assert!(d.starts_with(&format!("frame {}:", tampered_frame as u64)) && d.contains("frame_hash"), "{d}")
        }
        other => panic!("tampered reference must be Changed, got {other:?}"),
    }
    // 参考文件缺失：Changed（不是 Skip）
    assert!(matches!(gate(&pokered_path(), &script_path(), &dir.join("missing.json")), Status::Changed(_)));
    let _ = std::fs::remove_dir_all(dir);
}
