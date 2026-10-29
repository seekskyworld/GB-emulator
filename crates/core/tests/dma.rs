//! OAM DMA 状态机单测（core 通道，D022；Pan Docs `OAM_DMA_Transfer.md`）。
//! 约定：`write_reg` 发生在 M-cycle W（Bus 先 tick 再访问），之后每次 `tick_mcycle` 是 W+1、W+2……

use gb_core::dma::Dma;
use gb_core::state::{Snapshot, StateError, StateReader, StateWriter};

/// 记录 n 个 M-cycle 的 (tick 返回值, 该周期 oam_blocked)。
fn ticks(dma: &mut Dma, n: usize) -> Vec<(Option<(u16, u8)>, bool)> {
    (0..n)
        .map(|_| {
            let t = dma.tick_mcycle();
            (t, dma.oam_blocked())
        })
        .collect()
}

#[test]
fn power_up_state_is_idle() {
    let mut dma = Dma::new();
    assert_eq!(dma.read_reg(), 0xFF);
    assert!(!dma.oam_blocked());
    assert!(ticks(&mut dma, 500).iter().all(|&(t, b)| t.is_none() && !b));
}

#[test]
fn transfer_sequence_delay_and_length() {
    let mut dma = Dma::new();
    dma.write_reg(0xC1);
    assert_eq!(dma.read_reg(), 0xC1);
    assert!(!dma.oam_blocked(), "write cycle itself is not blocked");
    let t = ticks(&mut dma, 1 + 160 + 5);
    // W+1：启动延迟。
    assert_eq!(t[0], (None, false));
    // W+2 … W+161：每个 M-cycle 一字节，源 C100+i → OAM i。
    for i in 0..160usize {
        assert_eq!(t[1 + i], (Some((0xC100 + i as u16, i as u8)), true), "M-cycle W+{}", i + 2);
    }
    // W+162 起结束。
    assert!(t[161..].iter().all(|&(x, b)| x.is_none() && !b));
    assert_eq!(t.iter().filter(|x| x.0.is_some()).count(), 160);
    assert_eq!(t.iter().filter(|x| x.1).count(), 160);
    assert_eq!(dma.read_reg(), 0xC1, "FF46 keeps the last written value");
}

#[test]
fn every_source_page_maps_linearly() {
    for page in [0x00u8, 0x80, 0xC0, 0xDF, 0xFE] {
        let mut dma = Dma::new();
        dma.write_reg(page);
        let t = ticks(&mut dma, 161);
        let srcs: Vec<u16> = t.iter().filter_map(|x| x.0.map(|(s, _)| s)).collect();
        let want: Vec<u16> = (0..160).map(|i| (page as u16) << 8 | i).collect();
        assert_eq!(srcs, want, "page {page:02X}");
    }
}

#[test]
fn rewrite_during_transfer_restarts() {
    let mut dma = Dma::new();
    dma.write_reg(0xC0);
    let t = ticks(&mut dma, 1 + 50);
    assert_eq!(t[50], (Some((0xC031, 49)), true));
    // 第 50 字节之前重写 FF46：新传输的启动延迟期间旧传输照常再搬 1 字节（OAM 保持封锁），
    // 之后从第 0 字节、新的源页重新开始，完整的 160 字节。
    dma.write_reg(0xD0);
    assert_eq!(dma.read_reg(), 0xD0);
    let t = ticks(&mut dma, 1 + 160 + 2);
    assert_eq!(t[0], (Some((0xC032, 50)), true));
    for i in 0..160usize {
        assert_eq!(t[1 + i], (Some((0xD000 + i as u16, i as u8)), true));
    }
    assert_eq!(t[161], (None, false));
    assert_eq!(t[162], (None, false));
}

#[test]
fn rewrite_in_last_cycle_and_after_end() {
    // 在最后一字节所在周期之后写：旧传输已经结束，延迟周期不封锁。
    let mut dma = Dma::new();
    dma.write_reg(0x12);
    ticks(&mut dma, 161);
    dma.write_reg(0x34);
    let t = ticks(&mut dma, 2);
    assert_eq!(t[0], (None, false));
    assert_eq!(t[1], (Some((0x3400, 0)), true));

    // 在搬第 158 字节的周期里写：延迟周期里旧传输搬完最后一个字节（159），随后新传输开始。
    let mut dma = Dma::new();
    dma.write_reg(0x12);
    let t = ticks(&mut dma, 160);
    assert_eq!(t[159], (Some((0x129E, 158)), true));
    dma.write_reg(0x56);
    let t = ticks(&mut dma, 2);
    assert_eq!(t[0], (Some((0x129F, 159)), true));
    assert_eq!(t[1], (Some((0x5600, 0)), true));
}

fn save(dma: &Dma) -> Vec<u8> {
    let mut w = StateWriter::new();
    dma.save(&mut w);
    w.into_inner()
}

#[test]
fn snapshot_roundtrip_mid_transfer_and_pending_start() {
    // 三种状态：传输中、刚写完 FF46（启动延迟未过）、传输中又写了 FF46。
    let mut states = Vec::new();
    let mut a = Dma::new();
    a.write_reg(0xC0);
    ticks(&mut a, 70);
    states.push(a.clone());
    let mut b = Dma::new();
    b.write_reg(0x9A);
    states.push(b);
    let mut c = a.clone();
    c.write_reg(0x80);
    states.push(c);
    states.push(Dma::new());

    for dma in states {
        let data = save(&dma);
        assert_eq!(&data[..4], b"DMA ");
        let mut restored = Dma::new();
        let mut r = StateReader::new(&data);
        restored.load(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(restored, dma);
        assert_eq!(restored.oam_blocked(), dma.oam_blocked());
        let mut orig = dma.clone();
        assert_eq!(ticks(&mut restored, 400), ticks(&mut orig, 400));
    }
}

#[test]
fn snapshot_rejects_corrupt_data() {
    let mut dma = Dma::new();
    dma.write_reg(0xC0);
    ticks(&mut dma, 10);
    let data = save(&dma);
    let fresh = Dma::new();

    let mut x = fresh.clone();
    let mut bad = data.clone();
    bad[0] = b'X';
    assert!(matches!(x.load(&mut StateReader::new(&bad)), Err(StateError::Corrupt(_))));
    assert_eq!(x, fresh);

    let mut x = fresh.clone();
    assert_eq!(x.load(&mut StateReader::new(&data[..data.len() - 1])), Err(StateError::Truncated));
    assert_eq!(x, fresh);

    let mut x = fresh.clone();
    let mut bad = data.clone();
    bad[6] = 161; // index：tag(4) reg page index
    assert!(matches!(x.load(&mut StateReader::new(&bad)), Err(StateError::Corrupt(_))));
    assert_eq!(x, fresh);
}
