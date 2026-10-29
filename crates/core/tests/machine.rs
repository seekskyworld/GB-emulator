//! T13：整机连线（开机状态、step、run_frame 的帧预算）与存档文件头（PLAN §3.8）。

use gb_core::machine::{FINGERPRINT_LEN, STATE_MAGIC, STATE_VERSION};
use gb_core::{Machine, StateError, FRAME_T};

/// 合成 32 KiB ROM only：0x100 `NOP; JP $0150`，0x150 起 16 个 NOP 后 `JP $0150`。
fn rom_with(title: &[u8], header_checksum: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x104].copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
    rom[0x134..0x134 + title.len()].copy_from_slice(title);
    rom[0x147] = 0x00;
    rom[0x148] = 0x00;
    rom[0x149] = 0x00;
    rom[0x14D] = header_checksum;
    rom[0x14E] = 0x12;
    rom[0x14F] = 0x34;
    rom[0x160..0x163].copy_from_slice(&[0xC3, 0x50, 0x01]);
    rom
}

fn machine() -> Machine {
    Machine::new(rom_with(b"MACHTEST", 0x5A)).expect("synthetic ROM")
}

/// 只有文件头（magic + 版本 + 指纹）的存档。
fn header(version: u16, fp: &[u8]) -> Vec<u8> {
    let mut v = STATE_MAGIC.to_vec();
    v.extend_from_slice(&version.to_le_bytes());
    v.extend_from_slice(fp);
    v
}

#[test]
fn power_up_cpu_registers() {
    let m = machine();
    let r = m.regs();
    assert_eq!((r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l), (0x01, 0xB0, 0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D));
    assert_eq!((r.sp, r.pc, r.ime, r.halted), (0xFFFE, 0x0100, false, false));
    // 头部校验和为 0 时 H、C 清零。
    let m0 = Machine::new(rom_with(b"MACHTEST", 0x00)).unwrap();
    assert_eq!(m0.regs().f, 0x80);
}

#[test]
fn power_up_io_through_machine_peek() {
    let m = machine();
    assert_eq!(m.peek(0xFF00), 0xCF);
    assert_eq!(m.peek(0xFF02), 0x7E);
    assert_eq!(m.peek(0xFF04), 0xAB);
    assert_eq!(m.peek(0xFF07), 0xF8);
    assert_eq!(m.peek(0xFF0F), 0xE1);
    assert_eq!(m.peek(0xFF26), 0xF1);
    assert_eq!(m.peek(0xFFFF), 0x00);
    assert_eq!(m.cycles(), 0, "peek must not advance the clock");
}

/// 这个 ROM 上每一步的 T 数：0x0100 `NOP`（4）、`JP $0150`（16），之后循环 16 个 `NOP` + `JP $0150`（共 80 T，
/// 不整除 70224，所以每帧末尾都有余量）。
fn expected_step_t() -> impl Iterator<Item = u32> {
    [4u32, 16].into_iter().chain(std::iter::repeat([4u32; 16].into_iter().chain([16]).collect::<Vec<_>>()).flatten())
}

#[test]
fn step_runs_nop_sequence() {
    let mut m = machine();
    let mut total = 0u64;
    for (i, want) in expected_step_t().take(1000).enumerate() {
        let t = m.step();
        assert_eq!(t, want, "step {i}");
        total += t as u64;
    }
    assert_eq!(m.cycles(), total);
}

#[test]
fn run_frame_advances_exactly_one_frame() {
    let mut m = machine();
    m.run_frame();
    // 跑到第一个不早于 70224 T 的指令边界为止。
    let mut acc = 0u64;
    let boundary = expected_step_t().map(|t| { acc += t as u64; acc }).find(|&c| c >= FRAME_T as u64).unwrap();
    assert_eq!(m.cycles(), boundary);
    assert!(m.cycles() - (FRAME_T as u64) < 16);
}

#[test]
fn run_frame_remainder_carries_over() {
    // n 帧之后的总 T 数必须是第一个不早于 n×70224 的指令边界：上一帧多跑的 T 从下一帧扣除，既不丢也不重复。
    let mut m = machine();
    let mut acc = 0u64;
    let mut boundaries = expected_step_t().map(move |t| {
        acc += t as u64;
        acc
    });
    let mut last = 0u64;
    for n in 1..=50u64 {
        m.run_frame();
        let target = n * FRAME_T as u64;
        while last < target {
            last = boundaries.next().unwrap();
        }
        assert_eq!(m.cycles(), last, "frame {n}");
    }
}

#[test]
fn take_serial_initially_empty() {
    let mut m = machine();
    assert!(m.take_serial().is_empty());
}

#[test]
fn fingerprint_layout() {
    let m = machine();
    let fp = m.rom_fingerprint();
    assert_eq!(fp.len(), FINGERPRINT_LEN);
    assert_eq!(fp[0], 0x5A, "header checksum 0x14D");
    assert_eq!(&fp[1..3], &[0x12, 0x34], "global checksum 0x14E-0x14F");
    assert_eq!(&fp[3..11], b"MACHTEST");
    assert!(fp[11..].iter().all(|&b| b == 0));
}

#[test]
fn load_state_rejects_bad_magic() {
    let mut m = machine();
    assert_eq!(m.load_state(b""), Err(StateError::BadMagic));
    assert_eq!(m.load_state(b"GBEM"), Err(StateError::BadMagic));
    let mut bad = header(STATE_VERSION, &m.rom_fingerprint());
    bad[0] = b'X';
    assert_eq!(m.load_state(&bad), Err(StateError::BadMagic));
}

#[test]
fn load_state_rejects_bad_version() {
    let mut m = machine();
    let fp = m.rom_fingerprint();
    assert_eq!(m.load_state(&header(STATE_VERSION + 1, &fp)), Err(StateError::BadVersion(STATE_VERSION + 1)));
}

#[test]
fn load_state_rejects_other_rom_and_keeps_state() {
    let mut m = machine();
    for _ in 0..10 {
        m.step();
    }
    let before = (m.regs(), m.cycles(), m.peek(0xC000), m.peek(0xFF04));
    // 不同标题。
    let other = Machine::new(rom_with(b"OTHERROM", 0x5A)).unwrap();
    assert_eq!(m.load_state(&header(STATE_VERSION, &other.rom_fingerprint())), Err(StateError::RomMismatch));
    // 只有头部校验和不同。
    let other = Machine::new(rom_with(b"MACHTEST", 0x5B)).unwrap();
    assert_eq!(m.load_state(&header(STATE_VERSION, &other.rom_fingerprint())), Err(StateError::RomMismatch));
    assert_eq!((m.regs(), m.cycles(), m.peek(0xC000), m.peek(0xFF04)), before);
}

#[test]
fn load_state_truncated_header() {
    let mut m = machine();
    let fp = m.rom_fingerprint();
    let h = header(STATE_VERSION, &fp);
    assert_eq!(m.load_state(&h[..8]), Err(StateError::Truncated));
    assert_eq!(m.load_state(&h[..h.len() - 1]), Err(StateError::Truncated));
}

// ---- 整机存档往返（各模块的 Snapshot 都已实现；T52 另有基于 ROM 的往返测试）----

#[test]
fn save_state_header_and_roundtrip() {
    let mut m = machine();
    for _ in 0..3 {
        m.run_frame();
    }
    let s = m.save_state();
    assert_eq!(&s[..7], STATE_MAGIC);
    assert_eq!(u16::from_le_bytes([s[7], s[8]]), STATE_VERSION);
    assert_eq!(&s[9..9 + FINGERPRINT_LEN], &m.rom_fingerprint());

    let snap = (m.regs(), m.cycles());
    m.run_frame();
    m.load_state(&s).unwrap();
    assert_eq!((m.regs(), m.cycles()), snap);
    assert_eq!(m.save_state(), s);
}

#[test]
fn load_state_failure_keeps_state() {
    let mut m = machine();
    m.run_frame();
    let s = m.save_state();
    m.run_frame();
    let before = m.save_state();
    // 文件头正确但正文被截断：失败且状态不变。
    assert!(m.load_state(&s[..s.len() - 5]).is_err());
    assert_eq!(m.save_state(), before);
    // 尾部多余数据同样拒绝。
    let mut longer = s.clone();
    longer.push(0);
    assert!(m.load_state(&longer).is_err());
    assert_eq!(m.save_state(), before);
}
