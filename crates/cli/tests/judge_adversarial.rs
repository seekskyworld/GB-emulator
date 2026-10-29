//! T05b：判定逻辑的对抗单测（PLAN §4.1）——伪造结果必须全部判为失败；另有正例防止"永远失败"。

use gb_cli::judge::{
    blargg_result_line_complete, blargg_result_seen, diff_frames, judge_acid2, judge_blargg, RunEnd, Verdict,
};
use gb_core::FRAME_PIXELS;

const FIN: RunEnd = RunEnd::Finished;

fn fails(v: &Verdict) -> bool {
    !v.passed() && !v.reason().is_empty()
}

// ---------- Blargg：伪造结果 ----------

#[test]
fn serial_only_says_pass() {
    for text in ["Pass", "Pass\n", "03-op sp,hl\n\nPass\n", "PASS\n", "passed\n", "Pas sed\n"] {
        for collection in [false, true] {
            let v = judge_blargg(text, collection, &FIN);
            assert!(fails(&v), "{text:?} collection={collection} must fail, got {v:?}");
        }
    }
}

#[test]
fn passed_and_failed_together() {
    for text in [
        "Passed\nFailed\n",
        "Failed #3\nPassed\n",
        "01-special\n\nPassed\n\nFailed #2\n",
        "cpu_instrs\n\n01:ok 02:ok\n\nPassed all tests\nFailed 1 tests.\n",
        "PassedFailed",
    ] {
        for collection in [false, true] {
            let v = judge_blargg(text, collection, &FIN);
            assert!(fails(&v), "{text:?} collection={collection} must fail, got {v:?}");
        }
    }
}

#[test]
fn no_output_before_timeout() {
    let v = judge_blargg("", false, &RunEnd::Timeout { frames: 3584 });
    assert!(fails(&v), "{v:?}");
    assert!(v.reason().contains("timeout") && v.reason().contains("no serial output"), "{v:?}");
    let v = judge_blargg("", true, &RunEnd::Timeout { frames: 10752 });
    assert!(fails(&v));
    // 有输出但没有结果关键字，同样失败
    let v = judge_blargg("01-special\n\n\n", false, &RunEnd::Timeout { frames: 3584 });
    assert!(fails(&v) && v.reason().contains("timeout"), "{v:?}");
    // 只有空白
    assert!(fails(&judge_blargg(" \n\n ", false, &RunEnd::Timeout { frames: 1 })));
    assert!(fails(&judge_blargg("", false, &FIN)));
}

#[test]
fn collection_needs_passed_all_tests() {
    for text in ["Passed\n", "Passed all\n", "Passed all test\n", "passed all tests\n"] {
        let v = judge_blargg(text, true, &FIN);
        assert!(fails(&v), "{text:?} must fail for a collection, got {v:?}");
    }
}

#[test]
fn panic_or_error_fails_even_with_passed_text() {
    let v = judge_blargg("Passed\n", false, &RunEnd::Panic("boom at x.rs:1".into()));
    assert!(fails(&v) && v.reason().starts_with("panic:"), "{v:?}");
    let v = judge_blargg("Passed all tests\n", true, &RunEnd::Error("step returned 0".into()));
    assert!(fails(&v) && v.reason().starts_with("error:"), "{v:?}");
}

#[test]
fn failure_reason_quotes_serial_tail() {
    let v = judge_blargg("01-special\n\n\nDAA\n\nFailed #6\n", false, &FIN);
    assert!(fails(&v));
    assert!(v.reason().contains("Failed #6") && v.reason().contains("DAA"), "{v:?}");
}

// ---------- Blargg：正例（防止判定器退化成永远失败） ----------

#[test]
fn genuine_passes() {
    assert_eq!(judge_blargg("03-op sp,hl\n\n\nPassed\n", false, &FIN), Verdict::Pass);
    assert_eq!(judge_blargg("instr_timing\n\n\nPassed\n", false, &RunEnd::Timeout { frames: 3584 }), Verdict::Pass);
    let multi = "cpu_instrs\n\n01:ok  02:ok  03:ok  04:ok  05:ok  06:ok  07:ok  08:ok  09:ok  10:ok  11:ok  \n\nPassed all tests\n";
    assert_eq!(judge_blargg(multi, true, &FIN), Verdict::Pass);
}

#[test]
fn early_stop_detection() {
    assert!(!blargg_result_seen("Pass"));
    assert!(blargg_result_seen("xx Passed"));
    assert!(blargg_result_seen("Failed"));
    // 关键字出现但结果行未完整：继续收（避免把 "Passed all tests" 截成 "Passed al"）
    assert!(!blargg_result_line_complete("Passed al"));
    assert!(!blargg_result_line_complete("Passed\nFailed #"));
    assert!(blargg_result_line_complete("Passed all tests\n"));
    assert!(blargg_result_line_complete("Failed #3\n"));
    assert!(!blargg_result_line_complete("no keyword\n"));
}

// ---------- acid2 ----------

fn frame(v: u8) -> Vec<u8> {
    vec![v; FRAME_PIXELS]
}

#[test]
fn acid2_one_pixel_off_fails() {
    let reference: Vec<u8> = (0..FRAME_PIXELS).map(|i| [0xFF, 0xAA, 0x55, 0x00][i % 4]).collect();
    for idx in [0usize, 80 + 72 * 160, FRAME_PIXELS - 1] {
        let mut actual = reference.clone();
        actual[idx] = if actual[idx] == 0xFF { 0xAA } else { 0xFF };
        let (v, d) = judge_acid2(&FIN, Some(&actual), &reference);
        assert!(fails(&v), "pixel {idx}: {v:?}");
        let d = d.unwrap();
        assert_eq!(d.count, 1);
        let (x, y) = (idx % 160, idx / 160);
        assert_eq!(d.bbox, Some((x, y, x, y)));
        assert!(v.reason().contains("1 px") && v.reason().contains(&format!("({x},{y})-({x},{y})")), "{v:?}");
    }
}

#[test]
fn acid2_bbox_and_count() {
    let reference = frame(0xFF);
    let mut actual = reference.clone();
    actual[10 + 20 * 160] = 0;
    actual[30 + 5 * 160] = 0;
    actual[15 + 100 * 160] = 0;
    let d = diff_frames(&actual, &reference);
    assert_eq!(d.count, 3);
    assert_eq!(d.bbox, Some((10, 5, 30, 100)));
}

#[test]
fn acid2_identical_frame_but_bad_run_fails() {
    let reference = frame(0xAA);
    let (v, _) = judge_acid2(&RunEnd::Timeout { frames: 119 }, Some(&reference), &reference);
    assert!(fails(&v) && v.reason().contains("LD B,B"), "never hit LD B,B: {v:?}");
    let (v, _) = judge_acid2(&RunEnd::Panic("x".into()), Some(&reference), &reference);
    assert!(fails(&v));
    let (v, _) = judge_acid2(&RunEnd::Error("x".into()), Some(&reference), &reference);
    assert!(fails(&v));
    let (v, _) = judge_acid2(&FIN, None, &reference);
    assert!(fails(&v), "no frame: {v:?}");
}

#[test]
fn acid2_size_mismatches_fail() {
    let reference = frame(0xFF);
    let (v, _) = judge_acid2(&FIN, Some(&reference[..FRAME_PIXELS - 1]), &reference);
    assert!(fails(&v), "short frame: {v:?}");
    let (v, _) = judge_acid2(&FIN, Some(&reference), &reference[..100]);
    assert!(fails(&v), "short reference: {v:?}");
    let empty: Vec<u8> = Vec::new();
    let (v, _) = judge_acid2(&FIN, Some(&empty), &empty);
    assert!(fails(&v), "both empty must not pass: {v:?}");
}

#[test]
fn acid2_genuine_pass() {
    let reference: Vec<u8> = (0..FRAME_PIXELS).map(|i| [0xFF, 0xAA, 0x55, 0x00][(i * 7) % 4]).collect();
    let (v, d) = judge_acid2(&FIN, Some(&reference.clone()), &reference);
    assert_eq!(v, Verdict::Pass);
    assert_eq!(d.unwrap().count, 0);
}
