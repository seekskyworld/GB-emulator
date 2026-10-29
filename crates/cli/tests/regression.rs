//! T05b：棘轮（REGRESSION）单测，用构造的 gates.json 验证。

use gb_cli::baseline::{merge, regressions};
use gb_cli::gates::{exit_code, RomResult};
use gb_cli::json::{parse, Value};
use gb_cli::judge::RunEnd;

fn gates_json(items: &[(&str, &str, bool)]) -> Value {
    let results: Vec<String> = items
        .iter()
        .map(|(g, rom, pass)| {
            format!(r#"{{"group":"{g}","name":"x","rom":"{rom}","pass":{pass},"reason":"","frames":1,"serial":""}}"#)
        })
        .collect();
    parse(&format!(r#"{{"summary":"s","fast":false,"only":null,"results":[{}]}}"#, results.join(","))).unwrap()
}

const A: &str = "blargg/cpu_instrs/individual/01-special.gb";
const B: &str = "blargg/cpu_instrs/individual/03-op sp,hl.gb";
const C: &str = "blargg/cpu_instrs/cpu_instrs.gb";
const D: &str = "acid2/dmg-acid2.gb";

#[test]
fn pass_to_fail_is_regression() {
    let base = gates_json(&[("G1", A, true), ("G1", B, true), ("G4", D, false)]);
    let cur = gates_json(&[("G1", A, true), ("G1", B, false), ("G4", D, false)]);
    assert_eq!(regressions(&base, &cur).unwrap(), vec![B.to_string()]);
}

#[test]
fn fail_to_fail_and_fail_to_pass_are_not_regressions() {
    let base = gates_json(&[("G1", A, false), ("G4", D, false)]);
    let cur = gates_json(&[("G1", A, true), ("G4", D, false)]);
    assert!(regressions(&base, &cur).unwrap().is_empty());
}

#[test]
fn not_run_this_time_is_not_regression() {
    // --fast 跳过合集：基线中通过的合集本次不存在，不算回归
    let base = gates_json(&[("G1", A, true), ("G1", C, true)]);
    let cur = gates_json(&[("G1", A, true)]);
    assert!(regressions(&base, &cur).unwrap().is_empty());
}

#[test]
fn new_rom_failing_is_not_regression() {
    let base = gates_json(&[("G1", A, true)]);
    let cur = gates_json(&[("G1", A, true), ("G4", D, false)]);
    assert!(regressions(&base, &cur).unwrap().is_empty());
}

#[test]
fn all_regressions_listed_in_current_order() {
    let base = gates_json(&[("G4", D, true), ("G1", A, true), ("G1", B, true)]);
    let cur = gates_json(&[("G1", A, false), ("G1", B, false), ("G4", D, false)]);
    assert_eq!(regressions(&base, &cur).unwrap(), vec![A.to_string(), B.to_string(), D.to_string()]);
}

#[test]
fn malformed_baseline_is_an_error_not_an_empty_baseline() {
    let cur = gates_json(&[("G1", A, false)]);
    for bad in [r#"{}"#, r#"{"results":{}}"#, r#"{"results":[{"rom":"x"}]}"#, r#"{"results":[{"pass":true}]}"#] {
        assert!(regressions(&parse(bad).unwrap(), &cur).is_err(), "{bad}");
    }
}

fn result(pass: bool) -> RomResult {
    RomResult {
        kind: gb_cli::gates::Kind::Acid2,
        group: "G4".into(),
        name: "dmg-acid2".into(),
        rom: D.into(),
        pass,
        reason: String::new(),
        frames: 0,
        serial: Vec::new(),
        trace: Vec::new(),
        end: RunEnd::Finished,
        diff: None,
        frame: None,
    }
}

#[test]
fn exit_codes() {
    let reg = vec![D.to_string()];
    assert_eq!(exit_code(&[result(false)], &reg, false), 2, "REGRESSION exits 2");
    assert_eq!(exit_code(&[result(true)], &reg, false), 2);
    assert_eq!(exit_code(&[result(false)], &[], false), 1);
    assert_eq!(exit_code(&[result(true)], &[], true), 1, "report/baseline errors exit 1");
    assert_eq!(exit_code(&[result(true)], &[], false), 0);
}

#[test]
fn merge_keeps_unrun_entries_and_overrides_run_ones() {
    let old = gates_json(&[("G1", A, true), ("G1", C, true), ("G4", D, false)]);
    let cur = gates_json(&[("G1", A, true), ("G4", D, true), ("G1", B, true)]);
    let m = merge(Some(&old), &cur).unwrap();
    let got: Vec<(String, bool)> = gb_cli::baseline::entries(&m).unwrap();
    assert_eq!(
        got,
        vec![(A.into(), true), (C.into(), true), (D.into(), true), (B.into(), true)]
    );
    assert_eq!(m.get("summary").and_then(Value::as_str), Some("G1 3/3 G4 1/1"));
    // 合并结果能被再次解析为基线
    let text = m.to_pretty();
    assert!(regressions(&parse(&text).unwrap(), &cur).unwrap().is_empty());
    // 没有旧基线时就是本次结果
    let fresh = merge(None, &cur).unwrap();
    assert_eq!(gb_cli::baseline::entries(&fresh).unwrap().len(), 3);
}
