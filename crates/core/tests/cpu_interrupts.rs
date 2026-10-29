//! 中断分派、EI/DI/RETI、HALT 与 HALT bug（T14 的 CPU 侧；PLAN §3.3、Pan Docs Interrupts.md / halt.md）。
//!
//! 假总线：IE/IF 就是 RAM 的 0xFFFF/0xFF0F；`raise` 在指定 M-cycle（从 1 计数）开始时置 IF 位，
//! 与 Bus 的 D002 顺序一致（先推进外设，再访问）。

#[path = "cpu_common.rs"]
mod common;

use common::{Access, FlatBus, IE, IF};
use gb_core::Cpu;

const VBLANK: u8 = 0x01;
const STAT: u8 = 0x02;
const TIMER: u8 = 0x04;
const SERIAL: u8 = 0x08;
const JOYPAD: u8 = 0x10;

fn setup(program: &[u8]) -> (Cpu, FlatBus) {
    let mut bus = FlatBus::with_irq();
    bus.load(0x0100, program);
    let mut cpu = Cpu::new();
    cpu.sp = 0xD000;
    (cpu, bus)
}

fn step(cpu: &mut Cpu, bus: &mut FlatBus) -> Vec<Access> {
    bus.log.clear();
    cpu.step(bus);
    std::mem::take(&mut bus.log)
}

fn stacked_pc(cpu: &Cpu, bus: &FlatBus) -> u16 {
    u16::from_le_bytes([bus.mem[cpu.sp as usize], bus.mem[cpu.sp as usize + 1]])
}

#[test]
fn dispatch_is_exactly_5_mcycles() {
    let (mut cpu, mut bus) = setup(&[]);
    cpu.pc = 0x1234;
    cpu.ime = true;
    bus.mem[IE as usize] = TIMER;
    bus.mem[IF as usize] = TIMER;
    let log = step(&mut cpu, &mut bus);
    assert_eq!(
        log,
        vec![Access::Idle, Access::Idle, Access::Write(0xCFFF, 0x12), Access::Write(0xCFFE, 0x34), Access::Idle]
    );
    assert_eq!(cpu.pc, 0x0050);
    assert_eq!(cpu.sp, 0xCFFE);
    assert!(!cpu.ime);
    assert_eq!(bus.mem[IF as usize], 0);
    assert_eq!(bus.acks, vec![2]);
}

#[test]
fn priority_is_lowest_bit_first() {
    let (mut cpu, mut bus) = setup(&[]);
    bus.mem[IE as usize] = 0x1F;
    bus.mem[IF as usize] = 0x1F;
    for (vector, bit) in [(0x40, 0u8), (0x48, 1), (0x50, 2), (0x58, 3), (0x60, 4)] {
        cpu.ime = true;
        step(&mut cpu, &mut bus);
        assert_eq!(cpu.pc, vector);
        assert_eq!(*bus.acks.last().unwrap(), bit);
    }
    assert_eq!(bus.mem[IF as usize], 0);

    // IE 屏蔽的位不参与竞争。
    let (mut cpu, mut bus) = setup(&[]);
    cpu.ime = true;
    bus.mem[IE as usize] = TIMER | JOYPAD;
    bus.mem[IF as usize] = VBLANK | STAT | TIMER | SERIAL | JOYPAD;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x50);
    assert_eq!(bus.mem[IF as usize], VBLANK | STAT | SERIAL | JOYPAD);
}

#[test]
fn ime_clear_means_no_dispatch() {
    let (mut cpu, mut bus) = setup(&[0x00, 0x00]);
    bus.mem[IE as usize] = 0x1F;
    bus.mem[IF as usize] = 0x1F;
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0100, 0x00)]);
    assert_eq!(cpu.pc, 0x0101);
    assert_eq!(bus.mem[IF as usize], 0x1F);
}

#[test]
fn ei_takes_effect_after_next_instruction() {
    // EI; NOP; NOP
    let (mut cpu, mut bus) = setup(&[0xFB, 0x00, 0x00]);
    bus.mem[IE as usize] = VBLANK;
    bus.mem[IF as usize] = VBLANK;
    step(&mut cpu, &mut bus); // EI
    assert!(!cpu.ime && cpu.ei_pending);
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0101, 0x00)]); // NOP 照常执行
    assert!(cpu.ime && !cpu.ei_pending);
    assert_eq!(step(&mut cpu, &mut bus).len(), 5); // 分派
    assert_eq!(cpu.pc, 0x40);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0102);
}

#[test]
fn ei_followed_by_di_never_enables() {
    // EI; DI; NOP; NOP（Mooneye rapid_di_ei 的"EI 后紧跟 DI"）
    let (mut cpu, mut bus) = setup(&[0xFB, 0xF3, 0x00, 0x00]);
    bus.mem[IE as usize] = SERIAL;
    bus.mem[IF as usize] = SERIAL;
    for _ in 0..4 {
        assert_eq!(step(&mut cpu, &mut bus).len(), 1);
        assert!(!cpu.ime);
    }
    assert_eq!(cpu.pc, 0x0104);
    assert_eq!(bus.mem[IF as usize], SERIAL);

    // EI; DI; EI; DI
    let (mut cpu, mut bus) = setup(&[0xFB, 0xF3, 0xFB, 0xF3, 0x00]);
    bus.mem[IE as usize] = SERIAL;
    bus.mem[IF as usize] = SERIAL;
    for _ in 0..5 {
        assert_eq!(step(&mut cpu, &mut bus).len(), 1);
    }
    assert!(!cpu.ime);
    assert_eq!(bus.acks, Vec::<u8>::new());
}

#[test]
fn ei_sequence_dispatches_after_second_ei() {
    // 连续的 EI：第二条 EI 执行完 IME=1，返回地址是第三条 EI（Mooneye ei_sequence）。
    let (mut cpu, mut bus) = setup(&[0xFB, 0xFB, 0xFB, 0xFB]);
    bus.mem[IE as usize] = SERIAL;
    bus.mem[IF as usize] = SERIAL;
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert!(cpu.ime);
    assert_eq!(step(&mut cpu, &mut bus).len(), 5);
    assert_eq!(cpu.pc, 0x58);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0102);
    // 分派取代了 EI 之后的指令：处理程序的第一条指令之后 IME 不会被重新打开。
    step(&mut cpu, &mut bus);
    assert!(!cpu.ime);
}

#[test]
fn di_is_immediate() {
    // IME=1，DI 取指的那个 M-cycle 里产生中断：DI 立即生效，不分派。
    let (mut cpu, mut bus) = setup(&[0xF3, 0x00]);
    cpu.ime = true;
    bus.mem[IE as usize] = TIMER;
    bus.raise.push((1, TIMER));
    step(&mut cpu, &mut bus);
    assert!(!cpu.ime);
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0101, 0x00)]);
}

#[test]
fn reti_sets_ime_immediately() {
    // RETI 返回 0x0200；随后立即分派（无延迟）。
    let (mut cpu, mut bus) = setup(&[0xD9]);
    bus.load(0xD000, &[0x00, 0x02]);
    bus.mem[IE as usize] = STAT;
    bus.mem[IF as usize] = STAT;
    assert_eq!(step(&mut cpu, &mut bus).len(), 4);
    assert!(cpu.ime);
    assert_eq!(cpu.pc, 0x0200);
    assert_eq!(step(&mut cpu, &mut bus).len(), 5);
    assert_eq!(cpu.pc, 0x48);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0200);
}

#[test]
fn halt_ime0_wakes_without_dispatch() {
    // HALT; INC A
    let (mut cpu, mut bus) = setup(&[0x76, 0x3C]);
    bus.mem[IE as usize] = TIMER;
    step(&mut cpu, &mut bus);
    assert!(cpu.halted);
    assert_eq!(cpu.pc, 0x0101);
    // 停机期间每次 step 只做 1 次 idle。
    for _ in 0..5 {
        assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]);
        assert!(cpu.halted);
    }
    // 第 7 个 M-cycle 产生中断（IE 屏蔽的位不唤醒：先产生一个未使能的中断）。
    bus.raise.push((7, SERIAL));
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]);
    assert!(cpu.halted);
    bus.raise.push((8, TIMER));
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]);
    // 唤醒后立即执行 HALT 之后的指令，不分派、不清 IF。
    let a = cpu.a;
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0101, 0x3C)]);
    assert!(!cpu.halted);
    assert_eq!(cpu.a, a.wrapping_add(1));
    assert_eq!(bus.mem[IF as usize], TIMER | SERIAL);
    assert!(bus.acks.is_empty());
}

#[test]
fn halt_ime1_dispatches_and_returns_after_halt() {
    let (mut cpu, mut bus) = setup(&[0x76, 0x00]);
    cpu.ime = true;
    bus.mem[IE as usize] = VBLANK;
    step(&mut cpu, &mut bus);
    assert!(cpu.halted);
    bus.raise.push((4, VBLANK));
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]); // 2
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]); // 3
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]); // 4：中断在此产生
    assert_eq!(step(&mut cpu, &mut bus).len(), 5); // 立即唤醒并分派
    assert!(!cpu.halted && !cpu.ime);
    assert_eq!(cpu.pc, 0x40);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0101);
}

/// 唤醒不额外耗时：与一串 NOP 等待的时序完全相同（Mooneye halt_ime0_nointr_timing / halt_ime1_timing2）。
#[test]
fn halt_wakeup_has_no_extra_delay() {
    // NOP 对照：第 6 个 M-cycle 里产生中断，NOP 在该周期结束，第 7..=11 个 M-cycle 分派。
    let (mut cpu, mut bus) = setup(&[0x00; 16]);
    cpu.ime = true;
    bus.mem[IE as usize] = TIMER;
    bus.raise.push((6, TIMER));
    while cpu.pc != 0x0050 {
        cpu.step(&mut bus);
        assert!(bus.mcycles <= 16);
    }
    let nop_mcycles = bus.mcycles;
    assert_eq!(nop_mcycles, 11);

    for ime in [true, false] {
        // HALT 占第 1 个 M-cycle，第 2..=6 个 M-cycle 停机 idle，中断在第 6 个里产生。
        let (mut cpu, mut bus) = setup(&[0x76, 0x3C]);
        cpu.ime = ime;
        bus.mem[IE as usize] = TIMER;
        bus.raise.push((6, TIMER));
        step(&mut cpu, &mut bus);
        for _ in 2..=6 {
            assert_eq!(step(&mut cpu, &mut bus), vec![Access::Idle]);
        }
        let log = step(&mut cpu, &mut bus);
        if ime {
            assert_eq!(log.len(), 5);
            assert_eq!((cpu.pc, bus.mcycles), (0x0050, nop_mcycles));
        } else {
            // 第 7 个 M-cycle 就取 HALT 之后的指令。
            assert_eq!(log, vec![Access::Read(0x0101, 0x3C)]);
            assert_eq!(bus.mcycles, 7);
        }
    }
}

#[test]
fn halt_bug_reads_next_byte_twice() {
    // IME=0 且已有挂起中断：HALT 立即结束，下一字节被执行两次。
    let (mut cpu, mut bus) = setup(&[0x76, 0x3C, 0x00]);
    bus.mem[IE as usize] = VBLANK;
    bus.mem[IF as usize] = VBLANK;
    cpu.a = 0x10;
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0100, 0x76)]);
    assert!(!cpu.halted && cpu.halt_bug);
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0101, 0x3C)]);
    assert_eq!(cpu.pc, 0x0101);
    assert_eq!(step(&mut cpu, &mut bus), vec![Access::Read(0x0101, 0x3C)]);
    assert_eq!(cpu.pc, 0x0102);
    assert_eq!(cpu.a, 0x12);
    assert!(!cpu.halt_bug);
    assert_eq!(bus.mem[IF as usize], VBLANK); // 不分派、不清 IF
}

#[test]
fn halt_bug_with_two_byte_instruction() {
    // HALT; LD A,$14 → 实际执行 LD A,$3E; INC D（Pan Docs 的经典例子）。
    let (mut cpu, mut bus) = setup(&[0x76, 0x3E, 0x14]);
    bus.mem[IE as usize] = TIMER;
    bus.mem[IF as usize] = TIMER;
    cpu.d = 0x07;
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.a, 0x3E);
    assert_eq!(cpu.pc, 0x0102);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.d, 0x08);
    assert_eq!(cpu.pc, 0x0103);
}

#[test]
fn halt_bug_rst_pushes_its_own_address() {
    // HALT; RST $38：RST 的返回地址指向 RST 自身（Pan Docs halt.md）。
    let (mut cpu, mut bus) = setup(&[0x76, 0xFF]);
    bus.mem[IE as usize] = JOYPAD;
    bus.mem[IF as usize] = JOYPAD;
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x0038);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0101);
}

#[test]
fn ei_halt_with_pending_interrupt_returns_to_halt() {
    // EI; HALT 且已有挂起中断：HALT 时 IME 仍为 0 → HALT bug；随后中断被服务，返回地址是 HALT 自身。
    let (mut cpu, mut bus) = setup(&[0xFB, 0x76, 0x00]);
    bus.mem[IE as usize] = VBLANK;
    bus.mem[IF as usize] = VBLANK;
    step(&mut cpu, &mut bus); // EI
    step(&mut cpu, &mut bus); // HALT（bug）
    assert!(cpu.ime && cpu.halt_bug && !cpu.halted);
    assert_eq!(step(&mut cpu, &mut bus).len(), 5);
    assert_eq!(cpu.pc, 0x40);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0101);
    assert!(!cpu.halt_bug);
}

#[test]
fn ei_halt_without_pending_uses_ime1_behaviour() {
    // EI; HALT，HALT 时无挂起中断：HALT 之后 IME=1，中断到来时被服务（Mooneye halt_ime0_ei）。
    let (mut cpu, mut bus) = setup(&[0xFB, 0x76, 0xF3]);
    bus.mem[IE as usize] = VBLANK;
    step(&mut cpu, &mut bus);
    step(&mut cpu, &mut bus);
    assert!(cpu.halted && cpu.ime && !cpu.halt_bug);
    bus.raise.push((bus.mcycles + 1, VBLANK));
    step(&mut cpu, &mut bus);
    assert_eq!(step(&mut cpu, &mut bus).len(), 5);
    assert_eq!(cpu.pc, 0x40);
    assert_eq!(stacked_pc(&cpu, &bus), 0x0102);
}

#[test]
fn dispatch_cancelled_when_pch_push_clears_ie() {
    // Mooneye ie_push 第 1 轮：SP=0 时 PCH 写到 IE（0xFFFF），清掉了唯一挂起的中断 → PC=0x0000，IF 不清，IME=0。
    let (mut cpu, mut bus) = setup(&[]);
    cpu.pc = 0x0200;
    cpu.sp = 0x0000;
    cpu.ime = true;
    bus.mem[IE as usize] = TIMER;
    bus.mem[IF as usize] = TIMER;
    let log = step(&mut cpu, &mut bus);
    assert_eq!(log, vec![Access::Idle, Access::Idle, Access::Write(0xFFFF, 0x02), Access::Write(0xFFFE, 0x00), Access::Idle]);
    assert_eq!(cpu.pc, 0x0000);
    assert!(!cpu.ime);
    assert_eq!(bus.mem[IF as usize], TIMER);
    assert!(bus.acks.is_empty());
}

#[test]
fn dispatch_vector_chosen_after_pch_push() {
    // PCH 写到 IE 后 VBlank 被屏蔽、Timer 仍挂起 → 改为分派 Timer。
    let (mut cpu, mut bus) = setup(&[]);
    cpu.pc = 0x0400;
    cpu.sp = 0x0000;
    cpu.ime = true;
    bus.mem[IE as usize] = VBLANK | TIMER;
    bus.mem[IF as usize] = VBLANK | TIMER;
    step(&mut cpu, &mut bus);
    assert_eq!(cpu.pc, 0x0050);
    assert_eq!(bus.mem[IF as usize], VBLANK);

    // Mooneye ie_push 第 3 轮：PCL 写到 IE 已经太晚，分派照常进行。
    let (mut cpu, mut bus) = setup(&[]);
    cpu.pc = 0x0200;
    cpu.sp = 0x0001;
    cpu.ime = true;
    bus.mem[IE as usize] = SERIAL;
    bus.mem[IF as usize] = SERIAL;
    step(&mut cpu, &mut bus);
    assert_eq!(bus.mem[IE as usize], 0x00);
    assert_eq!(cpu.pc, 0x0058);
    assert_eq!(bus.mem[IF as usize], 0);
}

#[test]
fn interrupt_raised_mid_instruction_is_taken_after_it() {
    // CALL nn（6 M-cycle）期间产生的中断在 CALL 完成后分派，压栈的是 CALL 的目标地址。
    let (mut cpu, mut bus) = setup(&[0xCD, 0x00, 0x30]);
    cpu.ime = true;
    bus.mem[IE as usize] = STAT;
    bus.raise.push((3, STAT));
    assert_eq!(step(&mut cpu, &mut bus).len(), 6);
    assert_eq!(cpu.pc, 0x3000);
    assert_eq!(step(&mut cpu, &mut bus).len(), 5);
    assert_eq!(cpu.pc, 0x48);
    assert_eq!(stacked_pc(&cpu, &bus), 0x3000);
}
