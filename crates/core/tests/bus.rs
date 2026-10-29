//! T13：总线内存映射、镜像、I/O 分派与开机寄存器值（直接断言寄存器/内存值）。

use gb_core::bus::Bus;
use gb_core::cart;
use gb_core::CpuBus;

/// 合成 32 KiB ROM only 卡带：0x147=0x00、无 RAM；每个字节为地址低 8 位异或高 8 位，便于识别。
fn rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    for (i, b) in rom.iter_mut().enumerate() {
        *b = (i as u8) ^ ((i >> 8) as u8);
    }
    rom[0x134..0x13B].copy_from_slice(b"BUSTEST");
    rom[0x147] = 0x00;
    rom[0x148] = 0x00;
    rom[0x149] = 0x00;
    rom
}

fn bus() -> Bus {
    Bus::new(cart::from_rom(rom()).expect("synthetic ROM"))
}

#[test]
fn rom_region_reads_cart_and_ignores_writes() {
    let mut b = bus();
    let r = rom();
    for addr in [0x0000u16, 0x0100, 0x3FFF, 0x4000, 0x7FFF] {
        assert_eq!(b.read(addr), r[addr as usize], "ROM {addr:04X}");
        assert_eq!(b.peek(addr), r[addr as usize]);
    }
    b.write(0x2000, 0x05);
    b.write(0x0100, 0x99);
    assert_eq!(b.read(0x0100), r[0x100]);
}

#[test]
fn vram_read_write() {
    let mut b = bus();
    b.write(0x8000, 0x12);
    b.write(0x9FFF, 0x34);
    b.write(0x8ABC, 0x56);
    assert_eq!(b.read(0x8000), 0x12);
    assert_eq!(b.read(0x9FFF), 0x34);
    assert_eq!(b.peek(0x8ABC), 0x56);
}

#[test]
fn cart_ram_absent_reads_ff() {
    let mut b = bus();
    b.write(0xA000, 0x12);
    assert_eq!(b.read(0xA000), 0xFF);
    assert_eq!(b.read(0xBFFF), 0xFF);
}

#[test]
fn wram_and_echo_mirror() {
    let mut b = bus();
    b.write(0xC000, 0x11);
    b.write(0xDFFF, 0x22);
    b.write(0xD000, 0x33);
    assert_eq!(b.read(0xC000), 0x11);
    assert_eq!(b.read(0xDFFF), 0x22);
    assert_eq!(b.read(0xD000), 0x33);
    // E000–FDFF 读写都落到 C000–DDFF。
    assert_eq!(b.read(0xE000), 0x11);
    assert_eq!(b.read(0xF000), 0x33);
    b.write(0xE123, 0x44);
    assert_eq!(b.read(0xC123), 0x44);
    b.write(0xFDFF, 0x55);
    assert_eq!(b.read(0xDDFF), 0x55);
    b.write(0xDDFE, 0x66);
    assert_eq!(b.peek(0xFDFE), 0x66);
    // DE00–DFFF 没有镜像（FE00 起是 OAM）。
    b.write(0xDE00, 0x77);
    assert_ne!(b.read(0xFE00), 0x77);
}

#[test]
fn oam_read_write() {
    let mut b = bus();
    b.write(0xFE00, 0xA1);
    b.write(0xFE9F, 0xA2);
    assert_eq!(b.read(0xFE00), 0xA1);
    assert_eq!(b.read(0xFE9F), 0xA2);
}

#[test]
fn unusable_area_reads_zero_and_ignores_writes() {
    let mut b = bus();
    for addr in [0xFEA0u16, 0xFEC0, 0xFEFF] {
        b.write(addr, 0x5A);
        assert_eq!(b.read(addr), 0x00, "{addr:04X}");
        assert_eq!(b.peek(addr), 0x00);
    }
    // 写入不得溢出到 OAM 或 I/O。
    assert_eq!(b.read(0xFE9F), 0x00);
}

#[test]
fn unused_io_reads_ff() {
    let mut b = bus();
    let unused = [0xFF03u16, 0xFF08, 0xFF09, 0xFF0A, 0xFF0B, 0xFF0C, 0xFF0D, 0xFF0E, 0xFF4C, 0xFF4D, 0xFF4F, 0xFF50, 0xFF51, 0xFF56, 0xFF68, 0xFF70, 0xFF7F];
    for addr in unused {
        assert_eq!(b.read(addr), 0xFF, "{addr:04X} before write");
        b.write(addr, 0x00);
        assert_eq!(b.read(addr), 0xFF, "{addr:04X} after write");
    }
}

#[test]
fn if_upper_bits_read_one() {
    let mut b = bus();
    b.write(0xFF0F, 0x00);
    assert_eq!(b.read(0xFF0F), 0xE0);
    b.write(0xFF0F, 0x1F);
    assert_eq!(b.read(0xFF0F), 0xFF);
    b.write(0xFF0F, 0x04);
    assert_eq!(b.peek(0xFF0F), 0xE4);
}

#[test]
fn ie_full_byte() {
    let mut b = bus();
    assert_eq!(b.read(0xFFFF), 0x00);
    b.write(0xFFFF, 0x1F);
    assert_eq!(b.read(0xFFFF), 0x1F);
    b.write(0xFFFF, 0xFF);
    assert_eq!(b.read(0xFFFF), 0xFF);
}

#[test]
fn hram_read_write() {
    let mut b = bus();
    b.write(0xFF80, 0x01);
    b.write(0xFFFE, 0x02);
    assert_eq!(b.read(0xFF80), 0x01);
    assert_eq!(b.read(0xFFFE), 0x02);
    // HRAM 写入不影响 IE。
    assert_eq!(b.read(0xFFFF), 0x00);
}

#[test]
fn apu_registers_read_unused_and_write_only_bits_as_one() {
    // Pan Docs Audio_Registers：每个寄存器读出时，只写位与未使用位为 1。
    let masks: [(u16, u8); 31] = [
        (0xFF10, 0x80), (0xFF11, 0x3F), (0xFF12, 0x00), (0xFF13, 0xFF), (0xFF14, 0xBF),
        (0xFF15, 0xFF),
        (0xFF16, 0x3F), (0xFF17, 0x00), (0xFF18, 0xFF), (0xFF19, 0xBF),
        (0xFF1A, 0x7F), (0xFF1B, 0xFF), (0xFF1C, 0x9F), (0xFF1D, 0xFF), (0xFF1E, 0xBF),
        (0xFF1F, 0xFF),
        (0xFF20, 0xFF), (0xFF21, 0x00), (0xFF22, 0x00), (0xFF23, 0xBF),
        (0xFF24, 0x00), (0xFF25, 0x00),
        (0xFF27, 0xFF), (0xFF28, 0xFF), (0xFF29, 0xFF), (0xFF2A, 0xFF), (0xFF2B, 0xFF),
        (0xFF2C, 0xFF), (0xFF2D, 0xFF), (0xFF2E, 0xFF), (0xFF2F, 0xFF),
    ];
    let mut b = bus();
    for (addr, mask) in masks {
        b.write(addr, 0x00);
        assert_eq!(b.read(addr), mask, "{addr:04X} after writing 00");
        b.write(addr, 0xFF);
        assert_eq!(b.read(addr), 0xFF, "{addr:04X} after writing FF");
        b.write(addr, 0x5A);
        assert_eq!(b.read(addr), 0x5A | mask, "{addr:04X} after writing 5A");
    }
}

#[test]
fn nr52_status_bits_are_read_only() {
    let mut b = bus();
    assert_eq!(b.read(0xFF26), 0xF1, "power-up: APU on, CH1 on");
    b.write(0xFF26, 0x80);
    assert_eq!(b.read(0xFF26), 0xF1, "writing bit7=1 keeps channel status");
    b.write(0xFF26, 0x8E);
    assert_eq!(b.read(0xFF26), 0xF1, "status bits cannot be written");
    b.write(0xFF26, 0x00);
    assert_eq!(b.read(0xFF26), 0x70, "APU off clears channel status; bits 6-4 read 1");
    b.write(0xFF26, 0x8F);
    assert_eq!(b.read(0xFF26), 0xF0);
}

#[test]
fn wave_ram_is_plain_storage() {
    let mut b = bus();
    for (i, addr) in (0xFF30u16..=0xFF3F).enumerate() {
        b.write(addr, (i as u8).wrapping_mul(0x11) ^ 0x0F);
    }
    for (i, addr) in (0xFF30u16..=0xFF3F).enumerate() {
        assert_eq!(b.read(addr), (i as u8).wrapping_mul(0x11) ^ 0x0F, "{addr:04X}");
    }
}

#[test]
fn ppu_and_dma_registers_dispatch() {
    let mut b = bus();
    // LCDC、BGP 的开机值来自 Ppu::new；写入后能读回（ppu 自行处理只读位）。
    assert_eq!(b.peek(0xFF40), 0x91);
    assert_eq!(b.peek(0xFF47), 0xFC);
    b.write(0xFF42, 0x12); // SCY
    assert_eq!(b.read(0xFF42), 0x12);
    b.write(0xFF4B, 0x07); // WX
    assert_eq!(b.read(0xFF4B), 0x07);
    // FF46 归 dma。
    assert_eq!(b.peek(0xFF46), 0xFF);
}

#[test]
fn power_up_io_values() {
    let b = bus();
    let expect: [(u16, u8); 33] = [
        (0xFF00, 0xCF),
        (0xFF01, 0x00),
        (0xFF02, 0x7E),
        (0xFF04, 0xAB),
        (0xFF05, 0x00),
        (0xFF06, 0x00),
        (0xFF07, 0xF8),
        (0xFF0F, 0xE1),
        (0xFF10, 0x80),
        (0xFF11, 0xBF),
        (0xFF12, 0xF3),
        (0xFF13, 0xFF),
        (0xFF14, 0xBF),
        (0xFF15, 0xFF),
        (0xFF16, 0x3F),
        (0xFF17, 0x00),
        (0xFF18, 0xFF),
        (0xFF19, 0xBF),
        (0xFF1A, 0x7F),
        (0xFF1B, 0xFF),
        (0xFF1C, 0x9F),
        (0xFF1D, 0xFF),
        (0xFF1E, 0xBF),
        (0xFF1F, 0xFF),
        (0xFF20, 0xFF),
        (0xFF21, 0x00),
        (0xFF22, 0x00),
        (0xFF23, 0xBF),
        (0xFF24, 0x77),
        (0xFF25, 0xF3),
        (0xFF26, 0xF1),
        (0xFF27, 0xFF),
        (0xFFFF, 0x00),
    ];
    for (addr, v) in expect {
        assert_eq!(b.peek(addr), v, "power-up {addr:04X}");
    }
}

#[test]
fn every_access_is_one_mcycle_and_peek_is_free() {
    let mut b = bus();
    assert_eq!(b.mcycles(), 0);
    b.peek(0xC000);
    b.peek(0xFF04);
    assert_eq!(b.mcycles(), 0);
    b.read(0xC000);
    assert_eq!(b.mcycles(), 1);
    b.write(0xC000, 1);
    assert_eq!(b.mcycles(), 2);
    b.idle();
    assert_eq!(b.mcycles(), 3);
    // pending/ack 不耗时。
    b.pending();
    b.ack(0);
    assert_eq!(b.mcycles(), 3);
}

#[test]
fn pending_and_ack_via_cpubus() {
    let mut b = bus();
    // 开机 IF=E1（VBlank 已请求）、IE=00：没有可分派的中断。
    assert_eq!(b.pending(), 0);
    b.write(0xFFFF, 0x01);
    assert_eq!(b.pending(), 0x01);
    b.write(0xFF0F, 0x1F);
    b.write(0xFFFF, 0xFF);
    assert_eq!(b.pending(), 0x1F);
    b.ack(0);
    b.ack(3);
    assert_eq!(b.pending(), 0x16);
    assert_eq!(b.read(0xFF0F), 0xF6);
}

#[test]
fn bus_snapshot_roundtrip() {
    use gb_core::{Snapshot, StateReader, StateWriter};
    let mut a = bus();
    a.write(0xC000, 0xAA);
    a.write(0xDFFF, 0xBB);
    a.write(0xFF80, 0xCC);
    a.write(0xFFFE, 0xDD);
    a.write(0xFF30, 0xEE);
    a.idle();
    let mut w = StateWriter::new();
    a.save(&mut w);
    let data = w.into_inner();

    let mut b = bus();
    let mut r = StateReader::new(&data);
    b.load(&mut r).unwrap();
    assert_eq!(r.remaining(), 0);
    assert_eq!(b.mcycles(), a.mcycles());
    for addr in [0xC000u16, 0xDFFF, 0xE000, 0xFF80, 0xFFFE, 0xFF30, 0xFF26] {
        assert_eq!(b.peek(addr), a.peek(addr), "{addr:04X}");
    }

    // 截断的数据返回错误，且不改动状态。
    let mut c = bus();
    let mut r = StateReader::new(&data[..data.len() - 3]);
    assert!(c.load(&mut r).is_err());
    assert_eq!(c.peek(0xC000), 0x00);
    assert_eq!(c.mcycles(), 0);
}

/// OAM DMA 由 Bus 驱动：从源页逐字节搬到 OAM，源页 ≥ 0xE0 时读 WRAM 镜像。
#[test]
fn oam_dma_copies_from_wram_and_echo() {
    let mut b = bus();
    for i in 0..0xA0u16 {
        b.write(0xC100 + i, i as u8 ^ 0x5A);
    }
    b.write(0xFF46, 0xC1);
    for _ in 0..200 {
        b.idle();
    }
    for i in 0..0xA0u16 {
        assert_eq!(b.peek(0xFE00 + i), i as u8 ^ 0x5A, "OAM {i}");
    }
    // 源页 ≥ 0xE0 时按 WRAM 镜像。
    for i in 0..0xA0u16 {
        b.write(0xC000 + i, !(i as u8));
    }
    b.write(0xFF46, 0xE0);
    for _ in 0..200 {
        b.idle();
    }
    for i in 0..0xA0u16 {
        assert_eq!(b.peek(0xFE00 + i), !(i as u8), "OAM {i} (echo)");
    }
}
