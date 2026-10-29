//! 整机 OAM DMA 测试（core 通道，D022）：小 ROM 把 DMA 例程拷进 HRAM 并执行（Pan Docs `OAM_DMA_Transfer.md`
//! 的 "Best practices" 例程，外加一次传输中的 OAM 读），然后用 `Machine::peek` 检查 OAM 与 FF46。

use gb_core::Machine;

/// 源数据（ROM 第 0x20 页）：OAM 第 i 字节应为它。
fn pattern(i: usize) -> u8 {
    (i as u8).wrapping_mul(7) ^ 0x5A
}

fn build_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x147] = 0x00; // ROM only
    rom[0x148] = 0x00; // 32 KiB
    rom[0x149] = 0x00; // 无 RAM
    let put = |rom: &mut Vec<u8>, at: usize, bytes: &[u8]| rom[at..at + bytes.len()].copy_from_slice(bytes);
    put(&mut rom, 0x0100, &[0x00, 0xC3, 0x50, 0x01]); // nop; jp $0150

    // HRAM 例程（拷到 FF80）：
    let routine: &[u8] = &[
        0x3E, 0x20, // FF80 ld a, $20
        0xE0, 0x46, // FF82 ldh [$46], a      ; 启动 DMA，源 $2000
        0xFA, 0x00, 0xFE, // FF84 ld a, [$FE00]  ; 传输进行中读 OAM → 0xFF
        0xE0, 0x90, // FF87 ldh [$90], a
        0x3E, 0x28, // FF89 ld a, 40
        0x3D, // FF8B .wait: dec a
        0x20, 0xFD, // FF8C jr nz, .wait
        0xC9, // FF8E ret
    ];
    put(&mut rom, 0x0200, routine);

    put(
        &mut rom,
        0x0150,
        &[
            0x31, 0xFE, 0xFF, // ld sp, $FFFE
            0x0E, 0x80, // ld c, $80
            0x21, 0x00, 0x02, // ld hl, $0200
            0x06, routine.len() as u8, // ld b, len
            0x2A, // .copy: ld a, [hl+]
            0xE2, // ldh [c], a
            0x0C, // inc c
            0x05, // dec b
            0x20, 0xFA, // jr nz, .copy
            0xCD, 0x80, 0xFF, // call $FF80
            0xAF, // xor a
            0xE0, 0x40, // ldh [$40], a   ; 关 LCD，之后读 OAM 不受 PPU 模式影响
            0x40, // ld b, b         ; 结束标记
            0x18, 0xFE, // jr @
        ],
    );
    for i in 0..160 {
        rom[0x2000 + i] = pattern(i);
    }
    rom
}

#[test]
fn hram_routine_copies_rom_page_to_oam() {
    let mut m = Machine::new(build_rom()).expect("machine");
    // 开机时 OAM 与源数据不同，避免"本来就相等"。
    assert!((0..160).any(|i| m.peek(0xFE00 + i as u16) != pattern(i)));
    let mut steps = 0;
    while !m.ld_b_b_hit() {
        m.step();
        steps += 1;
        assert!(steps < 100_000, "LD B,B not reached; regs {:?}", m.regs());
    }
    let oam: Vec<u8> = (0..160).map(|i| m.peek(0xFE00 + i as u16)).collect();
    let want: Vec<u8> = (0..160).map(pattern).collect();
    assert_eq!(oam, want);
    assert_eq!(m.peek(0xFF46), 0x20, "FF46 reads back the last written value");
    assert_eq!(m.peek(0xFF90), 0xFF, "OAM read during the transfer returns 0xFF");
}
