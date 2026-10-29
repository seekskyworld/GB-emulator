//! T52：存档往返（PLAN §4.2）：`save → 跑 600 帧 → hash(画面+WRAM+HRAM+寄存器)` 必须等于
//! `load → 跑 600 帧 → hash`，在 cpu_instrs（individual）与 pokered 上各跑一次；ROM 缺失时 SKIP。
//! 另含：ROM 指纹不匹配返回错误、截断或损坏的存档返回错误且机器状态不变（合成 ROM，不依赖 refs）。

use std::path::{Path, PathBuf};

use gb_core::{buttons, Machine, StateError};

const RUN_FRAMES: u32 = 600;

/// FNV-1a 64。
fn fnv(h: &mut u64, data: &[u8]) {
    for &b in data {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
}

/// 画面 + WRAM + HRAM + 寄存器 + 周期数的 hash。
fn state_hash(m: &Machine) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    fnv(&mut h, m.frame());
    let wram: Vec<u8> = (0xC000u16..0xE000).map(|a| m.peek(a)).collect();
    fnv(&mut h, &wram);
    let hram: Vec<u8> = (0xFF80u16..=0xFFFF).map(|a| m.peek(a)).collect();
    fnv(&mut h, &hram);
    let r = m.regs();
    fnv(&mut h, &[r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l, r.ime as u8, r.halted as u8]);
    fnv(&mut h, &r.sp.to_le_bytes());
    fnv(&mut h, &r.pc.to_le_bytes());
    fnv(&mut h, &m.cycles().to_le_bytes());
    h
}

/// 固定的输入脚本（两次运行完全相同），让游戏在 600 帧内经过几个画面。
fn input_for(frame: u32) -> u8 {
    match frame % 200 {
        60..=64 => buttons::START,
        120..=124 => buttons::A,
        150..=152 => buttons::DOWN,
        _ => 0,
    }
}

/// 从当前状态跑 `RUN_FRAMES` 帧，返回结束时的 hash 与收到的串口字节。
fn run(m: &mut Machine) -> (u64, Vec<u8>) {
    let mut serial = Vec::new();
    for f in 0..RUN_FRAMES {
        m.set_buttons(input_for(f));
        m.run_frame();
        serial.extend(m.take_serial());
    }
    m.set_buttons(0);
    (state_hash(m), serial)
}

/// PLAN §4.2 的往返：预热 `warmup` 帧后存档；继续跑 600 帧得到 hash1；
/// 回读存档（同一台机器）再跑 600 帧得到 hash2；新建机器回读再跑得到 hash3。三者必须相同。
fn roundtrip(rom: Vec<u8>, warmup: u32) {
    let mut m = Machine::new(rom.clone()).expect("cart");
    for _ in 0..warmup {
        m.run_frame();
    }
    m.take_serial();
    let at_save = state_hash(&m);
    let cycles_at_save = m.cycles();
    let state = m.save_state();
    assert_eq!(m.save_state(), state, "save_state must be deterministic and side-effect free");
    assert_eq!(state_hash(&m), at_save, "save_state must not change the machine");

    let (h1, s1) = run(&mut m);
    assert_ne!(h1, at_save, "600 frames should change the state (otherwise the test proves nothing)");

    m.load_state(&state).expect("load into the same machine");
    assert_eq!(m.cycles(), cycles_at_save);
    assert_eq!(state_hash(&m), at_save, "state right after load equals state at save");
    let (h2, s2) = run(&mut m);
    assert_eq!(h1, h2, "save -> 600 frames must equal load -> 600 frames");
    assert_serial_eq(&s1, &s2, "serial output after load must match");

    let mut fresh = Machine::new(rom).expect("cart");
    fresh.run_frame(); // 让新机器偏离开机状态，确认 load 覆盖了全部状态
    fresh.take_serial(); // 串口输出队列属于宿主侧，不在存档里
    fresh.load_state(&state).expect("load into a fresh machine");
    assert_eq!(state_hash(&fresh), at_save);
    let (h3, s3) = run(&mut fresh);
    assert_eq!(h1, h3, "fresh machine after load must follow the same trajectory");
    assert_serial_eq(&s1, &s3, "serial output on a fresh machine after load must match");
    // 跑完之后的存档也一致
    assert_eq!(m.save_state(), fresh.save_state());
}

/// 串口输出比较（失败时只打印长度与第一个差异，避免刷屏）。
fn assert_serial_eq(a: &[u8], b: &[u8], what: &str) {
    if a != b {
        let i = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        let ctx = |v: &[u8]| v[i.saturating_sub(4)..(i + 4).min(v.len())].to_vec();
        panic!("{what}: lengths {} vs {}, first difference at byte {i}: {:?} vs {:?}", a.len(), b.len(), ctx(a), ctx(b));
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn refs() -> PathBuf {
    match std::env::var_os("GBEMU_REFS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => workspace().join("refs"),
    }
}

fn read_or_skip(path: &Path, what: &str) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("SKIP {what}: {} ({e})", path.display());
            None
        }
    }
}

#[test]
fn roundtrip_cpu_instrs_individual() {
    // 02-interrupts 用到定时器与中断，往返覆盖面比纯 ALU 测试大
    let p = refs().join("blargg/cpu_instrs/individual/02-interrupts.gb");
    let Some(rom) = read_or_skip(&p, "cpu_instrs") else { return };
    roundtrip(rom, 10);
}

#[test]
fn roundtrip_pokered() {
    let p = match std::env::var_os("GBEMU_POKERED") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => workspace().join("roms/pokered.gb"),
    };
    let Some(rom) = read_or_skip(&p, "pokered") else { return };
    // 预热跨过开机动画的开头；存档发生在 MBC3 bank 已切换、卡带 RAM 已使能过的状态
    roundtrip(rom, 300);
}

// ---------- 合成 ROM（总是运行） ----------

/// 32 KiB ROM only：标题 `title`，0x0100 处循环 `INC (HL); INC L` 改写 WRAM，并每轮写一次串口。
fn synth(title: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    let code: [u8; 15] = [
        0x21, 0x00, 0xC0, // 0100 LD HL,C000
        0x34, //             0103 INC (HL)
        0x2C, //             0104 INC L
        0x7D, //             0105 LD A,L
        0xE0, 0x01, //       0106 LDH (01),A   ; SB
        0x3E, 0x81, //       0108 LD A,81
        0xE0, 0x02, //       010A LDH (02),A   ; SC：内部时钟发送
        0x18, 0xF5, //       010C JR 0103
        0x00,
    ];
    rom[0x100..0x100 + code.len()].copy_from_slice(&code);
    rom[0x134..0x134 + title.len()].copy_from_slice(title);
    rom[0x147] = 0x00;
    let mut x: u8 = 0;
    for &b in &rom[0x134..=0x14C] {
        x = x.wrapping_sub(b).wrapping_sub(1);
    }
    rom[0x14D] = x;
    rom
}

#[test]
fn roundtrip_synthetic() {
    roundtrip(synth(b"STATETEST"), 3);
}

#[test]
fn fingerprint_mismatch_is_rejected() {
    let mut a = Machine::new(synth(b"GAME A")).unwrap();
    let mut b = Machine::new(synth(b"GAME B")).unwrap();
    a.run_frame();
    b.run_frame();
    let sa = a.save_state();
    let before = b.save_state();
    assert_eq!(b.load_state(&sa), Err(StateError::RomMismatch));
    assert_eq!(b.save_state(), before, "failed load must leave the machine unchanged");
    // 同一 ROM 则可以
    let mut a2 = Machine::new(synth(b"GAME A")).unwrap();
    assert_eq!(a2.load_state(&sa), Ok(()));
}

#[test]
fn corrupt_states_are_rejected_without_side_effects() {
    let mut m = Machine::new(synth(b"CORRUPT")).unwrap();
    for _ in 0..2 {
        m.run_frame();
    }
    let good = m.save_state();
    m.run_frame();
    let before = m.save_state();

    assert!(m.load_state(&[]).is_err());
    let mut bad_magic = good.clone();
    bad_magic[0] ^= 0xFF;
    assert_eq!(m.load_state(&bad_magic), Err(StateError::BadMagic));
    for cut in [good.len() / 2, good.len() - 1] {
        assert!(m.load_state(&good[..cut]).is_err(), "truncated at {cut}");
        assert_eq!(m.save_state(), before, "machine unchanged after truncated load ({cut})");
    }
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(m.load_state(&trailing).is_err(), "trailing bytes");
    assert_eq!(m.save_state(), before);
    assert_eq!(m.load_state(&good), Ok(()));
}
