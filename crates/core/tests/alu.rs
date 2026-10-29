//! ALU 单测（T11，PLAN §4.2）：期望值按 `refs/gbz80.7.txt` 与 Pan Docs `CPU_Registers_and_Flags.md` 手算，
//! 每个函数至少 4 组输入。标志位写成 F 寄存器的位掩码：Z=0x80 N=0x40 H=0x20 C=0x10。

#[path = "cpu_common.rs"]
mod common;

use gb_core::alu::{self, Flags};

const Z: u8 = 0x80;
const N: u8 = 0x40;
const H: u8 = 0x20;
const C: u8 = 0x10;
const ALL: u8 = Z | N | H | C;

fn fl(bits: u8) -> Flags {
    Flags::from_u8(bits)
}

/// (输入 a, 输入 b, 输入 F, 期望结果, 期望 F)
type Case8 = (u8, u8, u8, u8, u8);

fn check8(name: &str, f: fn(u8, u8, Flags) -> (u8, Flags), cases: &[Case8]) {
    for &(a, b, fin, r, fout) in cases {
        let (got, gf) = f(a, b, fl(fin));
        assert_eq!((got, gf.to_u8()), (r, fout), "{name}({a:#04x}, {b:#04x}, F={fin:#04x})");
    }
}

/// 一元运算：(输入 v, 输入 F, 期望结果, 期望 F)
fn check1(name: &str, f: fn(u8, Flags) -> (u8, Flags), cases: &[(u8, u8, u8, u8)]) {
    for &(v, fin, r, fout) in cases {
        let (got, gf) = f(v, fl(fin));
        assert_eq!((got, gf.to_u8()), (r, fout), "{name}({v:#04x}, F={fin:#04x})");
    }
}

#[test]
fn flags_register_roundtrip_clears_low_nibble() {
    // POP AF 写入的 F 只保留高 4 位。
    assert_eq!(fl(0xFF).to_u8(), 0xF0);
    assert_eq!(fl(0x0F), Flags::default());
    assert_eq!(fl(0xA5), Flags { z: true, n: false, h: true, c: false });
    assert_eq!(Flags { z: false, n: false, h: false, c: true }.to_u8(), 0x10);
    assert_eq!(Flags::new(true, true, false, false).to_u8(), 0xC0);
}

#[test]
fn add() {
    check8("add", |a, b, f| alu::add8(a, b, f, false), &[
        (0x3A, 0xC6, 0, 0x00, Z | H | C), // 0x100：结果 0，低 4 位 A+6 进位
        (0x3C, 0xFF, 0, 0x3B, H | C),
        (0x3C, 0x12, 0, 0x4E, 0),
        (0x0F, 0x01, 0, 0x10, H),
        (0x01, 0x01, C, 0x02, 0), // ADD 不使用进位
        (0x80, 0x80, N, 0x00, Z | C), // N 被清零
    ]);
}

#[test]
fn adc_half_carry_includes_carry_in() {
    check8("adc", |a, b, f| alu::add8(a, b, f, true), &[
        (0xE1, 0x0F, C, 0xF1, H), // 1 + F + 1 = 0x11 → H
        (0xE1, 0x3B, C, 0x1D, C), // 1 + B + 1 = 0xD → 无 H；总和 0x11D → C
        (0xE1, 0x1E, C, 0x00, Z | H | C),
        (0x0F, 0x00, C, 0x10, H), // 只靠进位输入产生半进位
        (0xFF, 0x00, C, 0x00, Z | H | C), // 只靠进位输入产生全进位
        (0x0F, 0x00, 0, 0x0F, 0), // C=0 时与 ADD 相同
    ]);
}

#[test]
fn sub() {
    check8("sub", |a, b, f| alu::sub8(a, b, f, false), &[
        (0x3E, 0x3E, 0, 0x00, Z | N),
        (0x3E, 0x0F, 0, 0x2F, N | H), // E < F → 低 4 位借位
        (0x3E, 0x40, 0, 0xFE, N | C),
        (0x10, 0x01, 0, 0x0F, N | H),
        (0x05, 0x02, C, 0x03, N), // SUB 不使用进位
    ]);
}

#[test]
fn sbc_half_borrow_includes_carry_in() {
    check8("sbc", |a, b, f| alu::sub8(a, b, f, true), &[
        (0x3B, 0x2A, C, 0x10, N), // B - A - 1 = 0 → 无 H
        (0x3B, 0x3A, C, 0x00, Z | N),
        (0x3B, 0x4F, C, 0xEB, N | H | C), // B < F + 1
        (0x00, 0x00, C, 0xFF, N | H | C), // 只靠借位输入产生 H 与 C
        (0x10, 0x0F, C, 0x00, Z | N | H), // 0 < F + 1 → H；0x10 不小于 0x10 → 无 C
        (0xFF, 0xFF, C, 0xFF, N | H | C),
        (0x3B, 0x2A, 0, 0x11, N), // C=0 时与 SUB 相同
    ]);
}

#[test]
fn and_xor_or() {
    check8("and", alu::and8, &[
        (0x5A, 0x3F, 0, 0x1A, H),
        (0x5A, 0x38, 0, 0x18, H),
        (0x5A, 0x00, 0, 0x00, Z | H),
        (0xFF, 0xFF, ALL, 0xFF, H), // N、C 被清零
    ]);
    check8("xor", alu::xor8, &[
        (0xFF, 0xFF, 0, 0x00, Z),
        (0xFF, 0x0F, 0, 0xF0, 0),
        (0xFF, 0x8A, 0, 0x75, 0),
        (0x00, 0x00, ALL, 0x00, Z),
    ]);
    check8("or", alu::or8, &[
        (0x5A, 0x5A, 0, 0x5A, 0),
        (0x5A, 0x03, 0, 0x5B, 0),
        (0x5A, 0x0F, ALL, 0x5F, 0),
        (0x00, 0x00, 0, 0x00, Z),
    ]);
}

#[test]
fn cp() {
    for &(a, b, fin, fout) in &[
        (0x3C, 0x2F, 0, N | H),
        (0x3C, 0x3C, 0, Z | N),
        (0x3C, 0x40, 0, N | C),
        (0x00, 0x01, 0, N | H | C),
        (0x05, 0x05, C, Z | N), // CP 不使用进位
    ] {
        assert_eq!(alu::cp8(a, b, fl(fin)).to_u8(), fout, "cp({a:#04x}, {b:#04x})");
    }
}

#[test]
fn inc_dec_preserve_carry() {
    check1("inc", alu::inc8, &[
        (0xFF, 0, 0x00, Z | H),
        (0x50, C, 0x51, C), // C 保留
        (0x0F, 0, 0x10, H),
        (0x00, N | C, 0x01, C), // N 清零，C 保留
        (0xFF, C, 0x00, Z | H | C),
    ]);
    check1("dec", alu::dec8, &[
        (0x01, C, 0x00, Z | N | C), // C 保留
        (0x00, 0, 0xFF, N | H),
        (0x10, 0, 0x0F, N | H),
        (0x51, Z, 0x50, N), // Z 由结果决定
        (0x00, C, 0xFF, N | H | C),
    ]);
}

#[test]
fn daa_after_addition() {
    // 先用 add8 得到 ADD 之后的 A 与 F，再 DAA；期望值是十进制加法的结果。
    for &(x, y, r, fout) in &[
        (0x15, 0x27, 0x42, 0),     // 15 + 27 = 42：低位 C > 9 → +06
        (0x99, 0x01, 0x00, Z | C), // 99 + 1 = 100：+66，进位
        (0x09, 0x08, 0x17, 0),     // 9 + 8 = 17：H=1 → +06
        (0x90, 0x90, 0x80, C),     // 90 + 90 = 180：C=1 → +60
        (0x45, 0x55, 0x00, Z | C), // 45 + 55 = 100
        (0x38, 0x41, 0x79, 0),     // 无需调整
    ] {
        let (a, f) = alu::add8(x, y, Flags::default(), false);
        let (got, gf) = alu::daa(a, f);
        assert_eq!((got, gf.to_u8()), (r, fout), "daa after {x:02X}+{y:02X}");
    }
    // 直接给定 A/F 的边界：A > 0x99 且 C=0 → +60 并置 C；N 保持 0；H 清零。
    check1("daa", alu::daa, &[
        (0x9A, 0, 0x00, Z | C),
        (0xA0, 0, 0x00, Z | C),
        (0x0A, H, 0x10, 0),
        (0x00, C, 0x60, C),
    ]);
}

#[test]
fn daa_after_subtraction() {
    for &(x, y, r, fout) in &[
        (0x42, 0x15, 0x27, N),         // 42 - 15 = 27：H=1 → -06
        (0x10, 0x01, 0x09, N),         // 10 - 1 = 9
        (0x00, 0x01, 0x99, N | C),     // 0 - 1 = 99 借位：-66，C 保持
        (0x50, 0x50, 0x00, Z | N),     // 无需调整
        (0x20, 0x30, 0x90, N | C),     // 20 - 30 = 90 借位：-60
    ] {
        let (a, f) = alu::sub8(x, y, Flags::default(), false);
        let (got, gf) = alu::daa(a, f);
        assert_eq!((got, gf.to_u8()), (r, fout), "daa after {x:02X}-{y:02X}");
    }
    // N=1 时不看 A 的大小，只看 H/C。
    check1("daa", alu::daa, &[(0x9A, N, 0x9A, N), (0x0F, N, 0x0F, N)]);
}

#[test]
fn cpl_scf_ccf() {
    check1("cpl", alu::cpl, &[
        (0x35, 0, 0xCA, N | H),
        (0x00, Z | C, 0xFF, Z | N | H | C), // Z、C 不变
        (0xFF, 0, 0x00, N | H),             // 结果为 0 也不置 Z
        (0xAA, ALL, 0x55, ALL),
    ]);
    for &(fin, sc, cc) in &[
        (0, C, C),
        (Z | N | H, Z | C, Z | C),
        (Z | C, Z | C, Z),
        (N | H | C, C, 0),
    ] {
        assert_eq!(alu::scf(fl(fin)).to_u8(), sc, "scf F={fin:#04x}");
        assert_eq!(alu::ccf(fl(fin)).to_u8(), cc, "ccf F={fin:#04x}");
    }
}

#[test]
fn rotates_through_and_circular() {
    check1("rlc", alu::rlc, &[
        (0x85, ALL, 0x0B, C), // N、H 清零
        (0x00, 0, 0x00, Z),
        (0x01, C, 0x02, 0),
        (0xFF, 0, 0xFF, C),
    ]);
    check1("rrc", alu::rrc, &[
        (0x01, 0, 0x80, C),
        (0x00, C, 0x00, Z),
        (0x3B, 0, 0x9D, C),
        (0x80, ALL, 0x40, 0),
    ]);
    check1("rl", alu::rl, &[
        (0x80, 0, 0x00, Z | C),
        (0x11, 0, 0x22, 0),
        (0x11, C, 0x23, 0),
        (0x95, C, 0x2B, C),
    ]);
    check1("rr", alu::rr, &[
        (0x01, 0, 0x00, Z | C),
        (0x8A, 0, 0x45, 0),
        (0x8A, C, 0xC5, 0),
        (0x81, C, 0xC0, C),
    ]);
}

#[test]
fn accumulator_rotates_clear_z() {
    check1("rlca", alu::rlca, &[
        (0x85, 0, 0x0B, C),
        (0x00, Z, 0x00, 0), // 结果为 0 也不置 Z
        (0x80, 0, 0x01, C),
        (0x7F, ALL, 0xFE, 0),
    ]);
    check1("rrca", alu::rrca, &[
        (0x3B, 0, 0x9D, C),
        (0x00, Z, 0x00, 0),
        (0x01, 0, 0x80, C),
        (0x02, ALL, 0x01, 0),
    ]);
    check1("rla", alu::rla, &[
        (0x95, C, 0x2B, C),
        (0x80, 0, 0x00, C), // 结果为 0 也不置 Z
        (0x00, C, 0x01, 0),
        (0x40, Z | N | H, 0x80, 0),
    ]);
    check1("rra", alu::rra, &[
        (0x81, 0, 0x40, C),
        (0x01, 0, 0x00, C),
        (0x00, C, 0x80, 0),
        (0x8A, ALL, 0xC5, 0),
    ]);
}

#[test]
fn shifts_and_swap() {
    check1("sla", alu::sla, &[
        (0x80, 0, 0x00, Z | C),
        (0xFF, 0, 0xFE, C),
        (0x01, C, 0x02, 0), // 不移入 C
        (0x40, ALL, 0x80, 0),
    ]);
    check1("sra", alu::sra, &[
        (0x8A, 0, 0xC5, 0), // bit7 保持
        (0x01, 0, 0x00, Z | C),
        (0x81, 0, 0xC0, C),
        (0x7F, ALL, 0x3F, C),
    ]);
    check1("srl", alu::srl, &[
        (0x01, 0, 0x00, Z | C),
        (0xFF, 0, 0x7F, C),
        (0x8A, 0, 0x45, 0),
        (0x80, C, 0x40, 0), // 不移入 C
    ]);
    check1("swap", alu::swap, &[
        (0x00, 0, 0x00, Z),
        (0xF0, 0, 0x0F, 0),
        (0x12, 0, 0x21, 0),
        (0xAB, ALL, 0xBA, 0), // N、H、C 清零
    ]);
}

#[test]
fn bit_sets_h_and_keeps_c() {
    for &(b, v, fin, fout) in &[
        (7, 0x80, 0, H),
        (7, 0x7F, C, Z | H | C), // C 保留
        (0, 0x01, N, H),         // N 清零
        (4, 0xEF, ALL, Z | H | C),
        (3, 0x08, Z, H), // Z 由被测位决定
    ] {
        assert_eq!(alu::bit(b, v, fl(fin)).to_u8(), fout, "bit {b}, {v:#04x}, F={fin:#04x}");
    }
}

#[test]
fn res_and_set() {
    assert_eq!(alu::res(7, 0xFF), 0x7F);
    assert_eq!(alu::res(0, 0x01), 0x00);
    assert_eq!(alu::res(3, 0xF7), 0xF7);
    assert_eq!(alu::res(5, 0xFF), 0xDF);
    assert_eq!(alu::set(7, 0x00), 0x80);
    assert_eq!(alu::set(0, 0xFE), 0xFF);
    assert_eq!(alu::set(3, 0x08), 0x08);
    assert_eq!(alu::set(4, 0x0F), 0x1F);
}

#[test]
fn add16_hl_carries_from_bits_11_and_15() {
    for &(hl, v, fin, r, fout) in &[
        (0x8A23u16, 0x0605u16, 0u8, 0x9028u16, H),
        (0x8A23, 0x8A23, 0, 0x1446, H | C),
        (0x0001, 0x0001, Z | N, 0x0002, Z), // Z 保留，N 清零
        (0xFFFF, 0x0001, 0, 0x0000, H | C), // 结果为 0 也不置 Z
        (0x0FFF, 0x0001, 0, 0x1000, H),
        (0xF000, 0x1000, 0, 0x0000, C),
    ] {
        let (got, gf) = alu::add16_hl(hl, v, fl(fin));
        assert_eq!((got, gf.to_u8()), (r, fout), "add hl {hl:#06x} + {v:#06x}");
    }
}

#[test]
fn add_sp_e_flags_from_unsigned_low_byte() {
    for &(sp, e, r, fout) in &[
        (0xFFF8u16, 0x02u8, 0xFFFAu16, 0u8),
        (0xFFF8, 0x08, 0x0000, H | C), // Z 恒为 0
        (0x0000, 0xFF, 0xFFFF, 0),     // e = -1：0x00 + 0xFF 无进位
        (0x0001, 0xFF, 0x0000, H | C), // e = -1：0x01 + 0xFF 有进位
        (0x00FF, 0x01, 0x0100, H | C),
        (0x1234, 0x80, 0x11B4, 0),     // e = -128
        (0xD0F0, 0x90, 0xD080, C),     // e = -112：0xF0 + 0x90 有进位，0x0 + 0x0 无半进位
    ] {
        // 输入 F 全置位：Z、N 必须被清零。
        let (got, gf) = alu::add_sp_e(sp, e, fl(ALL));
        assert_eq!((got, gf.to_u8()), (r, fout), "add sp {sp:#06x} + {e:#04x}");
    }
}

/// CPU 层面：`POP AF` 清除 F 低 4 位；`LD HL,SP+e` 与 `ADD SP,e` 使用同一套 H/C 规则。
#[test]
fn pop_af_and_sp_offset_through_cpu() {
    use common::FlatBus;
    use gb_core::Cpu;

    let mut bus = FlatBus::new();
    bus.load(0x0100, &[0xF1, 0xF8, 0xFF, 0xE8, 0x08]); // POP AF; LD HL,SP-1; ADD SP,+8
    bus.load(0xC000, &[0xFF, 0x12]);
    let mut cpu = Cpu::new();
    cpu.sp = 0xC000;
    cpu.step(&mut bus);
    assert_eq!((cpu.a, cpu.f, cpu.sp), (0x12, 0xF0, 0xC002));
    cpu.step(&mut bus); // HL = 0xC002 - 1：0x02 + 0xFF 有进位，0x2 + 0xF 有半进位
    assert_eq!((cpu.hl(), cpu.f), (0xC001, H | C));
    cpu.step(&mut bus); // SP = 0xC002 + 8：0x02 + 0x08 无进位
    assert_eq!((cpu.sp, cpu.f), (0xC00A, 0));
}
