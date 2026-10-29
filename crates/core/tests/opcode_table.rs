//! Opcode 表测试（T10/T11，PLAN §4.2）：读取 `tests/data/Opcodes.json`，对 500 个合法 opcode
//! （无前缀 244 个：去掉 11 个非法 opcode 与 0xCB 前缀本身；CB 前缀 256 个）在平铺 RAM 的假总线上执行，
//! 断言助记符（译码分支）、指令长度、M-cycle 数（条件指令 taken / not taken 各测）与标志位约束。

#[path = "cpu_common.rs"]
mod common;

use common::json::parse;
use common::{Access, FlatBus};
use gb_core::cpu::Cpu;
use gb_core::decode::{decode, decode_cb, is_illegal, ILLEGAL};

#[derive(Debug)]
struct Entry {
    prefixed: bool,
    op: u8,
    mnemonic: String,
    bytes: u16,
    /// T-cycle；条件指令为 [taken, not taken]。
    cycles: Vec<u64>,
    /// Z N H C 的约束：`-` 不变，`0`/`1` 强制，其余为按结果计算。
    flags: [String; 4],
    operands: Vec<String>,
}

impl Entry {
    fn name(&self) -> String {
        if self.prefixed {
            format!("CB {:02X} {} {}", self.op, self.mnemonic, self.operands.join(","))
        } else {
            format!("{:02X} {} {}", self.op, self.mnemonic, self.operands.join(","))
        }
    }
    fn condition(&self) -> Option<&str> {
        if self.cycles.len() != 2 {
            return None;
        }
        let c = self.operands.first().map(String::as_str);
        assert!(matches!(c, Some("NZ" | "Z" | "NC" | "C")), "{}: conditional without cc", self.name());
        c
    }
    fn is_control_flow(&self) -> bool {
        !self.prefixed && matches!(self.mnemonic.as_str(), "JP" | "JR" | "CALL" | "RET" | "RETI" | "RST")
    }
}

fn load_entries() -> Vec<Entry> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/Opcodes.json");
    let text = std::fs::read_to_string(path).expect("read Opcodes.json");
    let root = parse(&text).expect("parse Opcodes.json");
    let mut out = Vec::new();
    for (table, prefixed) in [("unprefixed", false), ("cbprefixed", true)] {
        for (k, v) in root.field(table).as_obj() {
            let op = u8::from_str_radix(k.trim_start_matches("0x"), 16).expect("opcode key");
            let fl = v.field("flags");
            out.push(Entry {
                prefixed,
                op,
                mnemonic: v.field("mnemonic").as_str().to_string(),
                bytes: v.field("bytes").as_u64() as u16,
                cycles: v.field("cycles").as_arr().iter().map(|c| c.as_u64()).collect(),
                flags: ["Z", "N", "H", "C"].map(|f| fl.field(f).as_str().to_string()),
                operands: v.field("operands").as_arr().iter().map(|o| o.field("name").as_str().to_string()).collect(),
            });
        }
    }
    out
}

fn legal_entries() -> Vec<Entry> {
    load_entries().into_iter().filter(|e| e.prefixed || !(is_illegal(e.op) || e.op == 0xCB)).collect()
}

const PC0: u16 = 0x4000;
const SP0: u16 = 0xD000;
/// 4 组 F 输入：覆盖每个条件的成立与不成立，也用来验证 `-` 位不变。
const F_INPUTS: [u8; 4] = [0x00, 0xF0, 0x50, 0xA0];

struct Run {
    after: Cpu,
    log: Vec<Access>,
    bus: FlatBus,
}

/// 在 PC0 处放入指令（立即数为 0x12 0x34），BC/DE/HL/SP 指向远离指令流的 RAM，执行一次 `step`。
fn run(e: &Entry, f: u8, seed: u8) -> Run {
    let mut bus = FlatBus::new();
    let mut cpu = Cpu::new();
    cpu.pc = PC0;
    cpu.sp = SP0;
    cpu.f = f;
    cpu.a = 0x3C ^ seed.wrapping_mul(37);
    cpu.set_bc(0xC100 | seed as u16);
    cpu.set_de(0xC200 | seed.wrapping_mul(3) as u16);
    cpu.set_hl(0xC300 | seed.wrapping_mul(5) as u16);
    let mut code = Vec::new();
    if e.prefixed {
        code.push(0xCB);
    }
    code.extend([e.op, 0x12, 0x34]);
    bus.load(PC0, &code);
    // 内存操作数与栈上的返回地址。
    bus.mem[cpu.hl() as usize] = seed.wrapping_mul(29) ^ 0x81;
    bus.mem[cpu.bc() as usize] = 0x42;
    bus.mem[cpu.de() as usize] = 0x24;
    bus.load(SP0, &[0x78, 0x56]);
    cpu.step(&mut bus);
    let log = std::mem::take(&mut bus.log);
    Run { after: cpu, log, bus }
}

fn eval_cc(cc: &str, f: u8) -> bool {
    match cc {
        "NZ" => f & 0x80 == 0,
        "Z" => f & 0x80 != 0,
        "NC" => f & 0x10 == 0,
        "C" => f & 0x10 != 0,
        _ => unreachable!(),
    }
}

#[test]
fn opcode_json_has_500_legal_entries() {
    let all = load_entries();
    assert_eq!(all.len(), 512);
    for e in all.iter().filter(|e| !e.prefixed) {
        assert_eq!(e.mnemonic.starts_with("ILLEGAL"), is_illegal(e.op), "{}", e.name());
    }
    assert_eq!(ILLEGAL.len(), 11);
    let legal = legal_entries();
    assert_eq!(legal.iter().filter(|e| !e.prefixed).count(), 244);
    assert_eq!(legal.iter().filter(|e| e.prefixed).count(), 256);
}

/// 每个合法 opcode 都有译码分支，且助记符与 Opcodes.json 一致。
#[test]
fn decode_mnemonics_match() {
    let mut bad = Vec::new();
    for e in legal_entries() {
        let m = if e.prefixed { decode_cb(e.op).mnemonic() } else { decode(e.op).mnemonic() };
        if m != e.mnemonic {
            bad.push(format!("{}: decoded as {m}", e.name()));
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// M-cycle 数等于 cycles/4（条件指令 taken / not taken 各测一次以上）。
#[test]
fn mcycle_counts_match() {
    let mut bad = Vec::new();
    for e in legal_entries() {
        let cc = e.condition();
        let (mut saw_taken, mut saw_not) = (false, false);
        for (i, &f) in F_INPUTS.iter().enumerate() {
            let r = run(&e, f, i as u8);
            let expected = match cc {
                Some(cc) if eval_cc(cc, f) => {
                    saw_taken = true;
                    e.cycles[0]
                }
                Some(_) => {
                    saw_not = true;
                    e.cycles[1]
                }
                None => e.cycles[0],
            };
            let got = r.log.len() as u64 * 4;
            if got != expected {
                bad.push(format!("{} F={f:02X}: {got} T, expected {expected} ({:?})", e.name(), r.log));
            }
        }
        if cc.is_some() {
            assert!(saw_taken && saw_not, "{}: both branches must be exercised", e.name());
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// 指令长度：非跳转指令看 PC 增量；跳转成立时看从指令流读取的字节数，CALL/RST 另查压栈的返回地址。
#[test]
fn lengths_match() {
    let mut bad = Vec::new();
    for e in legal_entries() {
        for (i, &f) in F_INPUTS.iter().enumerate() {
            let r = run(&e, f, i as u8);
            let jumped = e.is_control_flow() && e.condition().is_none_or(|cc| eval_cc(cc, f));
            if !jumped {
                let delta = r.after.pc.wrapping_sub(PC0);
                if delta != e.bytes {
                    bad.push(format!("{} F={f:02X}: PC advanced {delta}, expected {}", e.name(), e.bytes));
                }
                continue;
            }
            let stream = r.log.iter().filter(|a| matches!(a, Access::Read(addr, _) if (PC0..PC0 + 3).contains(addr))).count();
            if stream as u16 != e.bytes {
                bad.push(format!("{} F={f:02X}: read {stream} instruction bytes, expected {}", e.name(), e.bytes));
            }
            if matches!(e.mnemonic.as_str(), "CALL" | "RST") {
                let sp = r.after.sp as usize;
                let ret = u16::from_le_bytes([r.bus.mem[sp], r.bus.mem[sp + 1]]);
                if ret != PC0 + e.bytes {
                    bad.push(format!("{} F={f:02X}: pushed {ret:04X}, expected {:04X}", e.name(), PC0 + e.bytes));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// 标志位约束：`-` 不变，`0`/`1` 被强制；F 的低 4 位恒为 0。
#[test]
#[ignore = "T11: enabled once the ALU is complete"]
fn flags_match() {
    let mut bad = Vec::new();
    for e in legal_entries() {
        for seed in 0..8u8 {
            for &f in &F_INPUTS {
                let r = run(&e, f, seed);
                let out = r.after.f;
                if out & 0x0F != 0 {
                    bad.push(format!("{} F={f:02X}: low nibble set in {out:02X}", e.name()));
                }
                for (k, spec) in e.flags.iter().enumerate() {
                    let mask = 0x80 >> k;
                    let (before, after) = (f & mask != 0, out & mask != 0);
                    let ok = match spec.as_str() {
                        "-" => before == after,
                        "0" => !after,
                        "1" => after,
                        _ => true,
                    };
                    if !ok {
                        bad.push(format!("{} F={f:02X} seed {seed}: flag {} is {}, spec {spec}", e.name(), ["Z", "N", "H", "C"][k], after as u8));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// 非法 opcode：取指 1 个 M-cycle 后锁死，此后每次 step 只做 1 次 idle，即使有可分派的中断。
#[test]
fn illegal_opcodes_lock_cpu() {
    for &op in &ILLEGAL {
        let mut bus = FlatBus::with_irq();
        bus.mem[common::IE as usize] = 0x1F;
        bus.load(PC0, &[op, 0x00, 0x00]);
        let mut cpu = Cpu::new();
        cpu.pc = PC0;
        cpu.step(&mut bus);
        assert_eq!(bus.log, vec![Access::Read(PC0, op)], "op {op:02X}");
        assert!(cpu.locked, "op {op:02X}");
        let pc = cpu.pc;
        cpu.ime = true;
        bus.mem[common::IF as usize] = 0x1F;
        for _ in 0..3 {
            bus.log.clear();
            cpu.step(&mut bus);
            assert_eq!(bus.log, vec![Access::Idle], "op {op:02X}");
            assert_eq!(cpu.pc, pc, "op {op:02X}");
        }
        assert_eq!(bus.mem[common::IF as usize], 0x1F, "op {op:02X}: locked CPU must not dispatch");
    }
}

/// 只有 0x40（LD B,B）置 ld_b_b。
#[test]
fn ld_b_b_flag() {
    for op in 0x40..=0x7Fu8 {
        if op == 0x76 {
            continue;
        }
        let mut bus = FlatBus::new();
        bus.load(PC0, &[op]);
        let mut cpu = Cpu::new();
        cpu.pc = PC0;
        cpu.set_hl(0xC000);
        cpu.step(&mut bus);
        assert_eq!(cpu.ld_b_b, op == 0x40, "op {op:02X}");
    }
}

/// STOP：2 字节、1 M-cycle（不读第二个字节），并调用 `bus.stop()`。
#[test]
fn stop_skips_second_byte_and_notifies_bus() {
    let mut bus = FlatBus::new();
    bus.load(PC0, &[0x10, 0x00]);
    let mut cpu = Cpu::new();
    cpu.pc = PC0;
    cpu.step(&mut bus);
    assert_eq!(bus.log, vec![Access::Read(PC0, 0x10)]);
    assert_eq!(cpu.pc, PC0 + 2);
    assert_eq!(bus.stops, 1);
}
