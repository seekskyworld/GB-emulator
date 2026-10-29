//! `Snapshot for Cpu` 往返测试（core 通道）：保存并恢复全部字段，包括 EI 挂起、HALT bug、锁死等内部状态。

#[path = "cpu_common.rs"]
mod common;

use common::{FlatBus, IE, IF};
use gb_core::state::{Snapshot, StateError, StateReader, StateWriter};
use gb_core::Cpu;

fn save(cpu: &Cpu) -> Vec<u8> {
    let mut w = StateWriter::new();
    cpu.save(&mut w);
    w.into_inner()
}

fn odd_cpu() -> Cpu {
    let mut cpu = Cpu::new();
    cpu.a = 0x12;
    cpu.f = 0xA0;
    cpu.b = 0x34;
    cpu.c = 0x56;
    cpu.d = 0x78;
    cpu.e = 0x9A;
    cpu.h = 0xBC;
    cpu.l = 0xDE;
    cpu.sp = 0xFFF0;
    cpu.pc = 0x4321;
    cpu.ime = true;
    cpu.halted = true;
    cpu.ld_b_b = true;
    cpu.ei_pending = true;
    cpu.halt_bug = true;
    cpu.locked = true;
    cpu
}

#[test]
fn roundtrip_restores_every_field() {
    // 每个布尔字段单独翻转一次，确认它们各自被保存（而不是碰巧相等）。
    let base = odd_cpu();
    let variants: Vec<Cpu> = (0..6)
        .map(|i| {
            let mut c = base.clone();
            match i {
                0 => c.ime = false,
                1 => c.halted = false,
                2 => c.ld_b_b = false,
                3 => c.ei_pending = false,
                4 => c.halt_bug = false,
                _ => c.locked = false,
            }
            c
        })
        .chain([base.clone(), Cpu::new()])
        .collect();
    for cpu in variants {
        let data = save(&cpu);
        assert_eq!(&data[..4], b"CPU ");
        let mut restored = Cpu::new();
        restored.pc = 0;
        let mut r = StateReader::new(&data);
        restored.load(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(restored, cpu);
    }
}

#[test]
fn load_rejects_bad_data_and_keeps_state() {
    let data = save(&odd_cpu());
    let fresh = Cpu::new();

    let mut cpu = fresh.clone();
    let mut bad_tag = data.clone();
    bad_tag[0] = b'X';
    assert!(matches!(cpu.load(&mut StateReader::new(&bad_tag)), Err(StateError::Corrupt(_))));
    assert_eq!(cpu, fresh);

    let mut cpu = fresh.clone();
    assert_eq!(cpu.load(&mut StateReader::new(&data[..data.len() - 1])), Err(StateError::Truncated));
    assert_eq!(cpu, fresh);

    let mut cpu = fresh.clone();
    let mut bad_f = data.clone();
    bad_f[5] |= 0x01; // F 在 tag 与 A 之后
    assert!(matches!(cpu.load(&mut StateReader::new(&bad_f)), Err(StateError::Corrupt(_))));
    assert_eq!(cpu, fresh);
}

/// 在 HALT bug 刚触发、EI 尚未生效时存档；读档后的执行轨迹必须与不存档完全一致。
#[test]
fn restored_cpu_continues_identically() {
    // EI; HALT（有挂起中断 → bug）; INC A; INC A ...
    let mut bus = FlatBus::with_irq();
    bus.load(0x0100, &[0xFB, 0x76, 0x3C, 0x3C, 0x3C]);
    bus.mem[IE as usize] = 0x01;
    bus.mem[IF as usize] = 0x01;
    let mut cpu = Cpu::new();
    cpu.sp = 0xD000;
    cpu.step(&mut bus); // EI
    let mid = save(&cpu);
    let mem_mid = bus.mem.clone();

    let run = |mut cpu: Cpu, mem: Vec<u8>| {
        let mut bus = FlatBus::with_irq();
        bus.mem = mem;
        for _ in 0..6 {
            cpu.step(&mut bus);
        }
        (cpu, bus.mem, bus.log)
    };
    let mut restored = Cpu::new();
    restored.load(&mut StateReader::new(&mid)).unwrap();
    assert!(restored.ei_pending);
    let expected = run(cpu, mem_mid.clone());
    let got = run(restored, mem_mid);
    assert_eq!(got.0, expected.0);
    assert_eq!(got.1, expected.1);
    assert_eq!(got.2, expected.2);
    // 轨迹本身符合预期：HALT bug + 分派，返回地址是 HALT 自身（0x0101）。
    assert_eq!(expected.0.pc, 0x0040 + 4);
}
