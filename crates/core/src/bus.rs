//! 总线与内存映射（sys 通道，T13；PLAN §3.2、§3.3、T13）。
//!
//! 归属：WRAM、HRAM、APU 寄存器存储由本模块持有；VRAM/OAM 转发给 `Ppu`；卡带 ROM/RAM 转发给 `Cart`；
//! IF/IE 在 `interrupts.rs`。每个 M-cycle 先推进外设 4 T（timer → serial → joypad → ppu → dma），
//! 再执行访问（D002）。这个顺序只在 `Bus::tick` 一处实现。
//!
//! 内存映射（`peek` 与 `read` 共用，见 `Bus::peek`）：
//!
//! | 地址        | 映射                                                        |
//! | ----------- | ----------------------------------------------------------- |
//! | 0000–7FFF   | 卡带 ROM（写入 = MBC 寄存器）                               |
//! | 8000–9FFF   | VRAM（ppu）                                                 |
//! | A000–BFFF   | 卡带 RAM                                                    |
//! | C000–DFFF   | WRAM                                                        |
//! | E000–FDFF   | 镜像 C000–DDFF                                              |
//! | FE00–FE9F   | OAM（ppu）；OAM DMA 进行中读 0xFF、写入忽略                 |
//! | FEA0–FEFF   | 读 0x00（OAM DMA 进行中读 0xFF），写入忽略                  |
//! | FF00–FF7F   | I/O，见 `read_io` / `write_io`                              |
//! | FF80–FFFE   | HRAM                                                        |
//! | FFFF        | IE                                                          |
//!
//! 本文件除 `Machine` 用到的方法外都由 sys 通道自定。

use crate::cart::Cart;
use crate::cpu::CpuBus;
use crate::dma::Dma;
use crate::interrupts::Interrupts;
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::serial::Serial;
use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::timer::Timer;
use crate::{Frame, Peripheral};

/// APU 寄存器与波形 RAM（FF10–FF3F）的字节数。
const APU_LEN: usize = 0x30;

/// NR52（FF26）在 APU 存储中的下标。
const NR52: usize = 0x16;

/// FF10–FF2F 的读掩码：读出值 = 存储值 | 掩码。只写位（长度计时器、周期、触发位）与未使用的位读 1
/// （Pan Docs Audio_Registers 各寄存器的位图；FF15、FF1F、FF27–FF2F 整个未使用）。
/// 波形 RAM（FF30–FF3F）可正常读写，不加掩码。
const APU_READ_OR: [u8; 0x20] = [
    0x80, 0x3F, 0x00, 0xFF, 0xBF, // NR10 NR11 NR12 NR13 NR14
    0xFF, // FF15
    0x3F, 0x00, 0xFF, 0xBF, // NR21 NR22 NR23 NR24
    0x7F, 0xFF, 0x9F, 0xFF, 0xBF, // NR30 NR31 NR32 NR33 NR34
    0xFF, // FF1F
    0xFF, 0x00, 0x00, 0xBF, // NR41 NR42 NR43 NR44
    0x00, 0x00, 0x70, // NR50 NR51 NR52（bit6–4 未使用；bit3–0 为只读的声道状态）
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // FF27–FF2F
];

/// APU 区（FF10–FF3F）的开机值（Power_Up_Sequence 的 DMG 列；表中未列出的地址与未使用的地址读 0xFF）。
/// 波形 RAM（FF30–FF3F）开机内容不确定，取 0x00。
fn apu_power_up() -> [u8; APU_LEN] {
    let mut apu = [0xFF; APU_LEN];
    let table: [(u16, u8); 21] = [
        (0xFF10, 0x80), // NR10
        (0xFF11, 0xBF), // NR11
        (0xFF12, 0xF3), // NR12
        (0xFF13, 0xFF), // NR13
        (0xFF14, 0xBF), // NR14
        (0xFF16, 0x3F), // NR21
        (0xFF17, 0x00), // NR22
        (0xFF18, 0xFF), // NR23
        (0xFF19, 0xBF), // NR24
        (0xFF1A, 0x7F), // NR30
        (0xFF1B, 0xFF), // NR31
        (0xFF1C, 0x9F), // NR32
        (0xFF1D, 0xFF), // NR33
        (0xFF1E, 0xBF), // NR34
        (0xFF20, 0xFF), // NR41
        (0xFF21, 0x00), // NR42
        (0xFF22, 0x00), // NR43
        (0xFF23, 0xBF), // NR44
        (0xFF24, 0x77), // NR50
        (0xFF25, 0xF3), // NR51
        (0xFF26, 0xF1), // NR52
    ];
    for (addr, v) in table {
        apu[(addr - 0xFF10) as usize] = v;
    }
    for b in &mut apu[0x20..] {
        *b = 0x00;
    }
    apu
}

pub struct Bus {
    pub(crate) cart: Box<dyn Cart>,
    pub(crate) wram: Box<[u8; 0x2000]>,
    pub(crate) hram: [u8; 0x7F],
    /// FF10–FF3F：APU 寄存器只做带读掩码的存储（不发声，PLAN §1.1）。NR52 的 bit3–0 是声道状态：
    /// 开机为 CH1 on（0x1），写入不能改动，关闭 APU（bit7=0）时清零；因为不发声，触发声道不会置位。
    pub(crate) apu: [u8; APU_LEN],
    pub(crate) ints: Interrupts,
    pub(crate) timer: Timer,
    pub(crate) serial: Serial,
    pub(crate) joypad: Joypad,
    pub(crate) ppu: Ppu,
    pub(crate) dma: Dma,
    /// 开机以来的 M-cycle 数。
    pub(crate) mcycles: u64,
}

impl Bus {
    /// DMG 开机状态（不执行 boot ROM，Power_Up_Sequence 的 DMG 列）。各外设在自己的 `new` 中给出开机值
    /// （P1=CF、SB=00、SC=7E、DIV=AB、TIMA=00、TMA=00、TAC=F8、IF=E1、IE=00；PPU 寄存器在 `Ppu::new`）。
    pub fn new(cart: Box<dyn Cart>) -> Self {
        Self {
            cart,
            wram: Box::new([0; 0x2000]),
            hram: [0; 0x7F],
            apu: apu_power_up(),
            ints: Interrupts::new(),
            timer: Timer::new(),
            serial: Serial::new(),
            joypad: Joypad::new(),
            ppu: Ppu::new(),
            dma: Dma::new(),
            mcycles: 0,
        }
    }

    pub fn mcycles(&self) -> u64 {
        self.mcycles
    }

    pub fn frame(&self) -> &Frame {
        self.ppu.frame()
    }

    /// bit0..7 = A B Select Start Right Left Up Down，1 = 按下（见 `crate::buttons`）。
    pub fn set_buttons(&mut self, mask: u8) {
        self.joypad.set_buttons(mask);
    }

    /// 取走自上次调用以来串口发出的字节。
    pub fn take_serial(&mut self) -> Vec<u8> {
        self.serial.take_output()
    }

    /// 无副作用读，与 `read` 共用映射但不推进时钟。
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cart.read_rom(addr),
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            0xC000..=0xFDFF => self.wram[(addr & 0x1FFF) as usize],
            0xFE00..=0xFE9F => {
                if self.dma.oam_blocked() {
                    0xFF
                } else {
                    self.ppu.read_oam(addr)
                }
            }
            0xFEA0..=0xFEFF => {
                if self.dma.oam_blocked() {
                    0xFF
                } else {
                    0x00
                }
            }
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ints.read_ie(),
        }
    }

    /// 执行一次写访问（不推进时钟）；`CpuBus::write` 在推进外设之后调用它。
    fn poke(&mut self, addr: u16, v: u8) {
        match addr {
            0x0000..=0x7FFF => self.cart.write_rom(addr, v),
            0x8000..=0x9FFF => self.ppu.write_vram(addr, v),
            0xA000..=0xBFFF => self.cart.write_ram(addr, v),
            0xC000..=0xFDFF => self.wram[(addr & 0x1FFF) as usize] = v,
            0xFE00..=0xFE9F => {
                if !self.dma.oam_blocked() {
                    self.ppu.write_oam(addr, v);
                }
            }
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => self.write_io(addr, v),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = v,
            0xFFFF => self.ints.write_ie(v),
        }
    }

    fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => self.joypad.read_reg(addr),
            0xFF01..=0xFF02 => self.serial.read_reg(addr),
            0xFF04..=0xFF07 => self.timer.read_reg(addr),
            0xFF0F => self.ints.read_if(),
            0xFF10..=0xFF2F => {
                let i = (addr - 0xFF10) as usize;
                self.apu[i] | APU_READ_OR[i]
            }
            0xFF30..=0xFF3F => self.apu[(addr - 0xFF10) as usize],
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.ppu.read_reg(addr),
            0xFF46 => self.dma.read_reg(),
            _ => 0xFF,
        }
    }

    fn write_io(&mut self, addr: u16, v: u8) {
        match addr {
            0xFF00 => self.joypad.write_reg(addr, v),
            0xFF01..=0xFF02 => self.serial.write_reg(addr, v),
            0xFF04..=0xFF07 => self.timer.write_reg(addr, v),
            0xFF0F => self.ints.write_if(v),
            0xFF26 => {
                // bit7 可写；bit3–0 只读，关闭 APU 时清零。
                let status = if v & 0x80 != 0 { self.apu[NR52] & 0x0F } else { 0 };
                self.apu[NR52] = (v & 0x80) | status;
            }
            0xFF10..=0xFF3F => self.apu[(addr - 0xFF10) as usize] = v,
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.ppu.write_reg(addr, v),
            0xFF46 => self.dma.write_reg(v),
            _ => {}
        }
    }

    /// OAM DMA 的源读取：无副作用；源地址 ≥ 0xE000 时按 WRAM 镜像处理（DMG 上 E0–FF 页读到 WRAM）。
    fn dma_source(&self, src: u16) -> u8 {
        if src >= 0xE000 {
            self.wram[(src & 0x1FFF) as usize]
        } else {
            self.peek(src)
        }
    }

    /// 推进 1 个 M-cycle（D002：先推进外设，再由调用方执行访问）。全机唯一的时钟推进点。
    fn tick(&mut self) {
        let mut req = self.timer.tick_mcycle();
        req |= self.serial.tick_mcycle();
        req |= self.joypad.tick_mcycle();
        req |= self.ppu.tick_mcycle();
        self.ints.request(req);
        if let Some((src, idx)) = self.dma.tick_mcycle() {
            let v = self.dma_source(src);
            self.ppu.dma_write_oam(idx, v);
        }
        self.mcycles += 1;
    }
}

impl CpuBus for Bus {
    fn read(&mut self, addr: u16) -> u8 {
        self.tick();
        self.peek(addr)
    }

    fn write(&mut self, addr: u16, v: u8) {
        self.tick();
        self.poke(addr, v);
    }

    fn idle(&mut self) {
        self.tick();
    }

    fn pending(&self) -> u8 {
        self.ints.pending()
    }

    fn ack(&mut self, bit: u8) {
        self.ints.ack(bit);
    }

    fn stop(&mut self) {
        self.timer.reset_div();
    }
}

/// Bus 自身持有的状态：WRAM、HRAM、APU 存储、M-cycle 计数。
/// 子模块（IF/IE、定时器、串口、Joypad、PPU、DMA、卡带）由 `Machine::save_state` 逐个写出。
impl Snapshot for Bus {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"BUS ");
        w.bytes(&self.wram[..]);
        w.bytes(&self.hram);
        w.bytes(&self.apu);
        w.u64(self.mcycles);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"BUS ")?;
        let mut wram = Box::new([0u8; 0x2000]);
        let mut hram = [0u8; 0x7F];
        let mut apu = [0u8; APU_LEN];
        r.bytes_into(&mut wram[..])?;
        r.bytes_into(&mut hram)?;
        r.bytes_into(&mut apu)?;
        let mcycles = r.u64()?;
        self.wram = wram;
        self.hram = hram;
        self.apu = apu;
        self.mcycles = mcycles;
        Ok(())
    }
}
