//! PPU（ppu 通道，T40–T42；W4 精确时序）：逐扫描线渲染加双缓冲（PLAN §3.5）。
//!
//! 冻结部分（D001）：`Ppu::new`、VRAM/OAM 访问方法、`dma_write_oam`、`frame`、`Peripheral` 实现
//! （`tick_mcycle` 返回 VBlank/STAT 中断位；`read_reg`/`write_reg` 处理 FF40–FF45、FF47–FF4B；
//! FF46 归 `dma.rs`）、`Snapshot`。VRAM 与 OAM 数组由本模块持有，Bus 只做转发。
//!
//! # 时序模型（Pan Docs `Rendering.md`、`STAT.md`、`Accessing_VRAM_and_OAM.md`）
//!
//! 每行 456 dot，共 154 行；`tick_mcycle` 每次推进 4 dot，只在到达下一个事件点时做事。
//! 下面的 dot 都是"M-cycle 采样网格"上的位置：Bus 先推进外设再访问，所以第 k 次 tick 之后的访问
//! 看到的是 dot 4k 的状态；硬件上发生在两个采样点之间的变化，记在它之后的第一个采样点上。
//! 以 LY 变为新值的那个采样点为本行 dot 0，可见行（LY 0–143）依次是：
//!
//! | dot | 事件 |
//! | --- | --- |
//! | 0 | LY 更新；STAT 的 LYC=LY 位暂时读 0，mode 位仍是上一行的；OAM 读封锁 |
//! | 4 | mode 2 开始：mode 2 中断源、LYC 比较（中断与 STAT 位）、STAT 读 mode 2；OAM 写封锁 |
//! | 80 | VRAM 读封锁；OAM 写短暂放开 |
//! | 84 | mode 3 开始：OAM scan、STAT 读 mode 3，VRAM/OAM 读写全部封锁 |
//! | 253+p | STAT 读 mode 0，解除封锁 |
//! | 256+p | 内部进入 mode 0：mode 0 中断源，画完本行 |
//!
//! 其中 p 是 mode 3 的额外长度（`Rendering.md` "Mode 3 length"）：SCX%8、窗口 6 dot、每个精灵 6–11 dot。
//! 非 4 的倍数的事件在之后第一个采样点生效。VBlank 行在 dot 4 做 LYC 比较；LY=144 在 dot 4 进入 mode 1、
//! 请求 VBlank 中断，并且 mode 2 中断源在这 1 个 M-cycle 内也有效（与 VBlank 同时请求 STAT 中断）。
//!
//! # 渲染时机
//!
//! 进入 mode 3 时做 OAM scan 并开始本行；mode 3 内第 x 个像素视为在 mode 3 开始后 `12 + SCX%8 + x` dot 输出
//! （开头两次 tile fetch 共 12 dot，再丢弃 SCX%8 个像素）。像素在以下时刻提交到后缓冲：
//! - 进入 mode 0 时，把本行剩余像素一次画完（通常就是整行一次画完）；
//! - mode 3 期间 CPU 写 PPU 寄存器、VRAM 或 OAM（含 DMA 写入）时，先把"已经输出"的像素画完，再让写入生效。
//!
//! 因此 mode 2 期间（以及 mode 3 开头的取 tile 期间）对寄存器的修改作用于整行，mode 0/1 的修改作用于下一行；
//! 绝大多数行只渲染一次，tick 内没有逐 dot 的工作。
//!
//! # STAT 中断
//!
//! 内部 STAT 中断线 = (LYC=LY 且 bit6) OR (mode 0 且 bit3) OR (mode 1 且 bit4) OR (mode 2 且 bit5)，
//! 只在其从 0 变为 1 时请求一次；某个源保持为高时，其他源再变高不会再次触发（`STAT.md`、`Interrupt_Sources.md`）。
//!
//! # LCD 开关（`LCDC.md`、`STAT.md`）
//!
//! 关闭：LY=0、mode=0，前缓冲置为全白一次，此后 tick 不推进 PPU；LYC=LY 标志与中断线保持关闭前的值
//! （比较电路不工作），VRAM/OAM 不封锁。
//! 开启：从 LY=0 开始，立即重新比较 LYC。这一行不做 OAM scan：dot 0–79 读 mode 0、不封锁，
//! dot 80 起是 mode 3，之后各事件比普通行早 4 dot，整行 452 dot。

mod render;

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::{irq, Frame, Peripheral, FRAME_PIXELS, SCREEN_H, SCREEN_W};

/// 每行的 dot 数。
const LINE_DOTS: u16 = 456;
/// LCD 开启后第 0 行的 dot 数。
const FIRST_LINE_DOTS: u16 = LINE_DOTS - 4;
/// 每帧的行数。
const LINES: u8 = 154;
/// mode 2 开始（以及 LYC 比较、VBlank 开始）的 dot。
const MODE2_DOT: u16 = 4;
/// VRAM 读封锁开始的 dot。
const VRAM_LOCK_DOT: u16 = 80;
/// 普通行 mode 3 开始的 dot。
const MODE3_DOT: u16 = 84;
/// LCD 开启后第 0 行 mode 3 开始的 dot。
const FIRST_LINE_MODE3_DOT: u16 = 80;
/// mode 3 开始到 STAT 读出 mode 0 的最短 dot 数。
const MODE3_VISIBLE_DOTS: u16 = 169;
/// STAT 读出 mode 0 到内部进入 mode 0 的 dot 数。
const MODE0_LAG_DOTS: u16 = 3;
/// mode 3 开始到第 0 个像素输出的 dot 数（两次 tile fetch）。
const FIRST_PIXEL_DOTS: u16 = 12;
/// 进入 LY=144 后 mode 2 中断源保持有效的 dot 数（1 个 M-cycle）。
const VBLANK_MODE2_DOTS: u16 = 4;
/// 每行最多选中的精灵数。
const MAX_LINE_OBJS: usize = 10;

/// 访问封锁位。
const LOCK_OAM_R: u8 = 1 << 0;
const LOCK_OAM_W: u8 = 1 << 1;
const LOCK_VRAM_R: u8 = 1 << 2;
const LOCK_VRAM_W: u8 = 1 << 3;
const LOCK_ALL: u8 = LOCK_OAM_R | LOCK_OAM_W | LOCK_VRAM_R | LOCK_VRAM_W;

/// 开机（PC=0x0100）时 PPU 在第 153 行内的 dot 位置。
///
/// Power_Up_Sequence.md 的 DMG 列给出 LY=0x00、STAT=0x85（mode 1、LYC=LY），即仍处于最后一个
/// VBlank 行、LY 已读作 0。Pan Docs 没有给出行内的具体位置，这里取距第 0 行开始 56 dot，
/// 依据是一份测试 ROM 文档中的开机时序说明（D034）。
const BOOT_DOT: u16 = LINE_DOTS - 56;

/// PPU 内部所处的阶段（决定 STAT 中断源与渲染）。STAT 读到的 mode 位单独记在 `stat_mode`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// LCD 刚开启后的第 0 行、mode 3 之前：不做 OAM scan，不提供 mode 中断源。
    LcdOnLine0,
    /// mode 2。
    OamScan,
    /// mode 3。
    Drawing,
    /// mode 0。
    HBlank,
    /// mode 1。
    VBlank,
}

impl Phase {
    fn to_u8(self) -> u8 {
        match self {
            Phase::LcdOnLine0 => 0,
            Phase::OamScan => 1,
            Phase::Drawing => 2,
            Phase::HBlank => 3,
            Phase::VBlank => 4,
        }
    }

    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Phase::LcdOnLine0,
            1 => Phase::OamScan,
            2 => Phase::Drawing,
            3 => Phase::HBlank,
            4 => Phase::VBlank,
            _ => return None,
        })
    }
}

/// 下一个事件（见顶部的时序表）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    /// 可见行 dot 4。
    OamScan,
    /// 可见行 dot 80。
    VramLock,
    /// mode 3 开始。
    Drawing,
    /// STAT 读出 mode 0、解除封锁。
    Mode0Visible,
    /// 内部进入 mode 0。
    HBlank,
    /// VBlank 行 dot 4。
    VBlankLine,
    /// LY=144 dot 8：mode 2 中断源结束。
    VBlankMode2End,
    /// 行尾。
    LineEnd,
}

impl Ev {
    fn to_u8(self) -> u8 {
        match self {
            Ev::OamScan => 0,
            Ev::VramLock => 1,
            Ev::Drawing => 2,
            Ev::Mode0Visible => 3,
            Ev::HBlank => 4,
            Ev::VBlankLine => 5,
            Ev::VBlankMode2End => 6,
            Ev::LineEnd => 7,
        }
    }

    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Ev::OamScan,
            1 => Ev::VramLock,
            2 => Ev::Drawing,
            3 => Ev::Mode0Visible,
            4 => Ev::HBlank,
            5 => Ev::VBlankLine,
            6 => Ev::VBlankMode2End,
            7 => Ev::LineEnd,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct Ppu {
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    /// 最近一个完整帧（进入 VBlank 时由后缓冲交换而来）。
    front: Box<Frame>,
    /// 正在绘制的帧。
    back: Box<Frame>,

    // ---- 寄存器 ----
    lcdc: u8,
    /// STAT 的中断选择位（bit3–6），其余位由状态合成。
    stat_sel: u8,
    scy: u8,
    scx: u8,
    /// LY 寄存器的值。正常运行时等于 `line`；只有开机状态（第 153 行、LY 已读作 0）两者不同。
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,

    // ---- 时序 ----
    /// 当前扫描线 0..=153（调度用）。
    line: u8,
    /// 行内 dot。
    dot: u16,
    /// 下一个事件及其 dot。
    ev: Ev,
    next_event: u16,
    /// LCD 开启后的第 0 行（行长 452 dot，没有 mode 2）。
    first_line: bool,
    phase: Phase,
    /// STAT bit1–0 读到的 mode。
    stat_mode: u8,
    /// 内部 LYC=LY 比较结果（驱动 STAT 中断线）。LCD 关闭期间保持不变。
    lyc_match: bool,
    /// STAT bit2 读到的值（行首 dot 0–3 为 0）。LCD 关闭期间保持不变。
    lyc_flag: bool,
    /// 当前的 VRAM/OAM 访问封锁（`LOCK_*`）。
    locks: u8,
    /// 进入 LY=144 的第一个 M-cycle：mode 2 中断源仍有效。
    vblank_mode2: bool,
    /// 内部 STAT 中断线的当前电平（上升沿触发）。
    stat_line: bool,
    /// 寄存器写入引起、尚未交给 Bus 的中断位（`take_irq` 或下一次 `tick_mcycle` 返回）。
    irq_pending: u8,

    // ---- 窗口 ----
    /// 窗口 Y 条件：本帧内某行开始时 WY==LY 后为真，VBlank 时清除。
    win_y_hit: bool,
    /// 窗口内部行计数器：本帧已实际绘制窗口的行数。
    win_line: u8,
    /// 当前行是否已绘制过窗口像素。
    win_drawn: bool,

    // ---- 当前行的渲染进度 ----
    /// 当前行第 0 个像素输出的 dot。
    first_pixel_dot: u16,
    /// 当前行已提交到后缓冲的像素数（mode 3 期间有效）。
    drawn_x: u8,
    /// OAM scan 选中的精灵个数（≤10）。
    obj_count: u8,
    /// 选中的精灵 OAM 序号，已按 DMG 绘制优先级排序（X 小者在前，X 相同时序号小者在前）。
    objs: [u8; MAX_LINE_OBJS],
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    /// DMG 开机状态（Pan Docs Power_Up_Sequence 的硬件寄存器表，DMG 列），前缓冲为全白。
    pub fn new() -> Self {
        let mut p = Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            front: Box::new([0xFF; FRAME_PIXELS]),
            back: Box::new([0xFF; FRAME_PIXELS]),
            lcdc: 0x91,
            stat_sel: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            // OBP0/OBP1 开机值未初始化（Power_Up_Sequence 脚注），取常见的 0xFF。
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            line: LINES - 1,
            dot: BOOT_DOT,
            ev: Ev::LineEnd,
            next_event: LINE_DOTS,
            first_line: false,
            phase: Phase::VBlank,
            stat_mode: 1,
            lyc_match: false,
            lyc_flag: false,
            locks: 0,
            vblank_mode2: false,
            stat_line: false,
            irq_pending: 0,
            win_y_hit: false,
            win_line: 0,
            win_drawn: false,
            first_pixel_dot: MODE3_DOT + FIRST_PIXEL_DOTS,
            drawn_x: 0,
            obj_count: 0,
            objs: [0; MAX_LINE_OBJS],
        };
        p.lyc_match = p.ly == p.lyc;
        p.lyc_flag = p.lyc_match;
        p.stat_line = p.stat_signal();
        p
    }

    /// 调试读 VRAM（`Machine::peek`、OAM DMA 取源）：不受访问封锁影响（新增方法，非冻结接口）。
    pub fn peek_vram(&self, addr: u16) -> u8 {
        self.vram[(addr & 0x1FFF) as usize]
    }

    /// 调试读 OAM（`Machine::peek`）：不受访问封锁影响（新增方法，非冻结接口）。
    pub fn peek_oam(&self, addr: u16) -> u8 {
        self.oam[(addr as usize).wrapping_sub(0xFE00) % 0xA0]
    }

    /// CPU 读 VRAM，`addr` 为绝对地址 0x8000..=0x9FFF。mode 3 期间读到 0xFF（`Accessing_VRAM_and_OAM.md`）。无副作用。
    pub fn read_vram(&self, addr: u16) -> u8 {
        if self.locks & LOCK_VRAM_R != 0 {
            return 0xFF;
        }
        self.vram[(addr & 0x1FFF) as usize]
    }

    /// CPU 写 VRAM，`addr` 为绝对地址 0x8000..=0x9FFF。mode 3 期间写入被忽略。
    pub fn write_vram(&mut self, addr: u16, v: u8) {
        if self.locks & LOCK_VRAM_W != 0 {
            return;
        }
        self.catch_up();
        self.vram[(addr & 0x1FFF) as usize] = v;
    }

    /// CPU 读 OAM，`addr` 为绝对地址 0xFE00..=0xFE9F。mode 2/3 期间读到 0xFF（`Accessing_VRAM_and_OAM.md`）。无副作用。
    pub fn read_oam(&self, addr: u16) -> u8 {
        if self.locks & LOCK_OAM_R != 0 {
            return 0xFF;
        }
        self.oam[(addr as usize).wrapping_sub(0xFE00) % 0xA0]
    }

    /// CPU 写 OAM，`addr` 为绝对地址 0xFE00..=0xFE9F。mode 2/3 期间写入被忽略。
    pub fn write_oam(&mut self, addr: u16, v: u8) {
        if self.locks & LOCK_OAM_W != 0 {
            return;
        }
        self.catch_up();
        self.oam[(addr as usize).wrapping_sub(0xFE00) % 0xA0] = v;
    }

    /// OAM DMA 写入第 `index`（0..160）个字节；由 Bus 驱动，不受 PPU 模式封锁影响。
    pub fn dma_write_oam(&mut self, index: u8, v: u8) {
        self.catch_up();
        self.oam[index as usize % 0xA0] = v;
    }

    /// 最近一个完整帧（前缓冲），值为 0xFF/0xAA/0x55/0x00。
    pub fn frame(&self) -> &Frame {
        &self.front
    }

    /// 取走寄存器写入（STAT、LYC、LCDC 开启）刚刚引起的中断位并清零（新增方法，非冻结接口）。
    ///
    /// Bus 不调用它也不会丢中断：这些位会在下一次 `tick_mcycle` 的返回值里交出。
    /// Bus 若在写 PPU 寄存器之后立即调用，中断就能在写入的同一个 M-cycle 进入 IF。
    pub fn take_irq(&mut self) -> u8 {
        std::mem::take(&mut self.irq_pending)
    }

    fn lcd_on(&self) -> bool {
        self.lcdc & 0x80 != 0
    }

    /// 内部 STAT 中断线此刻应有的电平。
    fn stat_signal(&self) -> bool {
        let s = self.stat_sel;
        let mode_src = match self.phase {
            Phase::HBlank => s & 0x08,
            Phase::VBlank => (s & 0x10) | if self.vblank_mode2 { s & 0x20 } else { 0 },
            Phase::OamScan => s & 0x20,
            Phase::Drawing | Phase::LcdOnLine0 => 0,
        };
        mode_src != 0 || (s & 0x40 != 0 && self.lyc_match)
    }

    /// 重新计算 STAT 中断线；上升沿时返回 STAT 中断位。
    fn update_stat(&mut self) -> u8 {
        let sig = self.stat_signal();
        let rising = sig && !self.stat_line;
        self.stat_line = sig;
        if rising {
            irq::STAT
        } else {
            0
        }
    }

    fn schedule(&mut self, ev: Ev, dot: u16) {
        self.ev = ev;
        self.next_event = dot;
    }

    fn line_dots(&self) -> u16 {
        if self.first_line {
            FIRST_LINE_DOTS
        } else {
            LINE_DOTS
        }
    }

    /// 处理所有已到期的事件，返回新请求的中断位。
    fn run_events(&mut self) -> u8 {
        let mut req = 0;
        while self.dot >= self.next_event {
            req |= self.event();
        }
        req
    }

    fn event(&mut self) -> u8 {
        match self.ev {
            Ev::LineEnd => {
                self.dot -= self.line_dots();
                self.first_line = false;
                let next = if self.line + 1 >= LINES {
                    0
                } else {
                    self.line + 1
                };
                self.start_line(next);
                return 0;
            }
            Ev::OamScan => {
                self.phase = Phase::OamScan;
                self.stat_mode = 2;
                self.compare_lyc();
                self.locks |= LOCK_OAM_W;
                if self.wy == self.line {
                    self.win_y_hit = true;
                }
                self.schedule(Ev::VramLock, VRAM_LOCK_DOT);
            }
            Ev::VramLock => {
                self.locks = LOCK_OAM_R | LOCK_VRAM_R;
                self.schedule(Ev::Drawing, MODE3_DOT);
            }
            Ev::Drawing => self.enter_drawing(),
            Ev::Mode0Visible => {
                self.stat_mode = 0;
                self.locks = 0;
                self.schedule(Ev::HBlank, self.next_event + MODE0_LAG_DOTS);
            }
            Ev::HBlank => self.enter_hblank(),
            Ev::VBlankLine => {
                self.compare_lyc();
                if self.line as usize == SCREEN_H {
                    self.phase = Phase::VBlank;
                    self.stat_mode = 1;
                    self.vblank_mode2 = true;
                    std::mem::swap(&mut self.front, &mut self.back);
                    self.win_y_hit = false;
                    self.win_line = 0;
                    self.schedule(Ev::VBlankMode2End, MODE2_DOT + VBLANK_MODE2_DOTS);
                    return irq::VBLANK | self.update_stat();
                }
                self.schedule(Ev::LineEnd, LINE_DOTS);
            }
            Ev::VBlankMode2End => {
                self.vblank_mode2 = false;
                self.schedule(Ev::LineEnd, LINE_DOTS);
            }
        }
        self.update_stat()
    }

    /// 行首（dot 0）：LY 更新，STAT 的 LYC=LY 位在 dot 4 比较之前读 0；可见行封锁 OAM 读。
    fn start_line(&mut self, line: u8) {
        self.line = line;
        self.ly = line;
        self.lyc_flag = false;
        if (line as usize) < SCREEN_H {
            self.locks = LOCK_OAM_R;
            self.schedule(Ev::OamScan, MODE2_DOT);
        } else {
            self.schedule(Ev::VBlankLine, MODE2_DOT);
        }
    }

    /// LYC 比较：同时更新中断用的比较结果与 STAT bit2。
    fn compare_lyc(&mut self) {
        self.lyc_match = self.ly == self.lyc;
        self.lyc_flag = self.lyc_match;
    }

    /// 行首到 dot 4 之间（尚未比较 LYC）。
    fn before_lyc_compare(&self) -> bool {
        matches!(self.ev, Ev::OamScan | Ev::VBlankLine)
    }

    /// 进入 mode 3：OAM scan（按此刻的 LCDC.2），算出本行 mode 3 的额外长度，开始本行的渲染。
    fn enter_drawing(&mut self) {
        let start = self.next_event;
        self.phase = Phase::Drawing;
        self.stat_mode = 3;
        self.locks = LOCK_ALL;
        self.oam_scan();
        self.drawn_x = 0;
        self.win_drawn = false;
        self.first_pixel_dot = start + FIRST_PIXEL_DOTS + (self.scx & 7) as u16;
        let extra = self.mode3_penalty();
        self.schedule(Ev::Mode0Visible, start + MODE3_VISIBLE_DOTS + extra);
    }

    /// 内部进入 mode 0：画完本行剩余像素；本行画过窗口时窗口行计数器加 1。
    fn enter_hblank(&mut self) {
        self.render_to(SCREEN_W);
        if self.win_drawn {
            self.win_line = self.win_line.wrapping_add(1);
        }
        self.phase = Phase::HBlank;
        let end = self.line_dots();
        self.schedule(Ev::LineEnd, end);
    }

    /// mode 3 期间发生 PPU 可见的写入之前调用：把已经输出的像素先画到后缓冲。
    fn catch_up(&mut self) {
        if self.phase == Phase::Drawing && self.lcd_on() {
            let x = self
                .dot
                .saturating_sub(self.first_pixel_dot)
                .min(SCREEN_W as u16);
            self.render_to(x as usize);
        }
    }

    /// 把当前行画到第 `x` 个像素（不含）。
    fn render_to(&mut self, x: usize) {
        let x0 = self.drawn_x as usize;
        if x > x0 {
            self.render_span(x0, x);
            self.drawn_x = x as u8;
        }
    }

    fn turn_off(&mut self) {
        self.line = 0;
        self.ly = 0;
        self.dot = 0;
        self.first_line = false;
        self.phase = Phase::HBlank;
        self.stat_mode = 0;
        self.locks = 0;
        self.schedule(Ev::LineEnd, LINE_DOTS);
        self.vblank_mode2 = false;
        self.drawn_x = 0;
        self.obj_count = 0;
        self.front.fill(0xFF);
        // LYC=LY 标志与 STAT 中断线保持关闭前的值：关闭期间比较电路不工作，重新开启时才再比较。
    }

    fn turn_on(&mut self) {
        self.line = 0;
        self.ly = 0;
        self.dot = 0;
        self.first_line = true;
        self.phase = Phase::LcdOnLine0;
        self.stat_mode = 0;
        self.locks = 0;
        self.schedule(Ev::Drawing, FIRST_LINE_MODE3_DOT);
        self.vblank_mode2 = false;
        self.drawn_x = 0;
        self.obj_count = 0;
        self.win_y_hit = self.wy == 0;
        self.win_line = 0;
        self.win_drawn = false;
        self.compare_lyc();
        self.irq_pending |= self.update_stat();
    }
}

impl Peripheral for Ppu {
    fn tick_mcycle(&mut self) -> u8 {
        let req = std::mem::take(&mut self.irq_pending);
        if !self.lcd_on() {
            return req;
        }
        self.dot += 4;
        if self.dot < self.next_event {
            return req;
        }
        req | self.run_events()
    }

    fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => {
                let mode = if self.lcd_on() { self.stat_mode } else { 0 };
                0x80 | self.stat_sel | ((self.lyc_flag as u8) << 2) | mode
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, addr: u16, v: u8) {
        match addr {
            0xFF40 => {
                let was_on = self.lcd_on();
                let on = v & 0x80 != 0;
                if was_on {
                    self.catch_up();
                }
                self.lcdc = v;
                if was_on && !on {
                    self.turn_off();
                } else if !was_on && on {
                    self.turn_on();
                }
            }
            0xFF41 => {
                self.stat_sel = v & 0x78;
                if self.lcd_on() {
                    self.irq_pending |= self.update_stat();
                }
            }
            0xFF44 => {} // LY 只读
            0xFF45 => {
                self.lyc = v;
                if self.lcd_on() {
                    self.lyc_match = self.ly == v;
                    if !self.before_lyc_compare() {
                        self.lyc_flag = self.lyc_match;
                    }
                    self.irq_pending |= self.update_stat();
                }
            }
            0xFF42 | 0xFF43 | 0xFF47..=0xFF4B => {
                self.catch_up();
                match addr {
                    0xFF42 => self.scy = v,
                    0xFF43 => self.scx = v,
                    0xFF47 => self.bgp = v,
                    0xFF48 => self.obp0 = v,
                    0xFF49 => self.obp1 = v,
                    0xFF4A => self.wy = v,
                    _ => self.wx = v,
                }
            }
            _ => {}
        }
    }
}

impl Snapshot for Ppu {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"PPU ");
        w.bytes(&self.vram);
        w.bytes(&self.oam);
        w.bytes(&self.front[..]);
        w.bytes(&self.back[..]);
        for v in [
            self.lcdc,
            self.stat_sel,
            self.scy,
            self.scx,
            self.ly,
            self.lyc,
            self.bgp,
            self.obp0,
            self.obp1,
            self.wy,
            self.wx,
        ] {
            w.u8(v);
        }
        w.u8(self.line);
        w.u16(self.dot);
        w.u8(self.ev.to_u8());
        w.u16(self.next_event);
        w.bool(self.first_line);
        w.u8(self.phase.to_u8());
        w.u8(self.stat_mode);
        w.bool(self.lyc_match);
        w.bool(self.lyc_flag);
        w.u8(self.locks);
        w.bool(self.vblank_mode2);
        w.bool(self.stat_line);
        w.u8(self.irq_pending);
        w.bool(self.win_y_hit);
        w.u8(self.win_line);
        w.bool(self.win_drawn);
        w.u16(self.first_pixel_dot);
        w.u8(self.drawn_x);
        w.u8(self.obj_count);
        w.bytes(&self.objs);
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"PPU ")?;
        // 先读进副本，全部校验通过后再整体替换，失败时 self 保持不变。
        let mut p = self.clone();
        r.bytes_into(&mut p.vram)?;
        r.bytes_into(&mut p.oam)?;
        r.bytes_into(&mut p.front[..])?;
        r.bytes_into(&mut p.back[..])?;
        p.lcdc = r.u8()?;
        p.stat_sel = r.u8()? & 0x78;
        p.scy = r.u8()?;
        p.scx = r.u8()?;
        p.ly = r.u8()?;
        p.lyc = r.u8()?;
        p.bgp = r.u8()?;
        p.obp0 = r.u8()?;
        p.obp1 = r.u8()?;
        p.wy = r.u8()?;
        p.wx = r.u8()?;
        p.line = r.u8()?;
        p.dot = r.u16()?;
        p.ev = Ev::from_u8(r.u8()?).ok_or(StateError::Corrupt("ppu event"))?;
        p.next_event = r.u16()?;
        p.first_line = r.bool()?;
        p.phase = Phase::from_u8(r.u8()?).ok_or(StateError::Corrupt("ppu phase"))?;
        p.stat_mode = r.u8()?;
        p.lyc_match = r.bool()?;
        p.lyc_flag = r.bool()?;
        p.locks = r.u8()?;
        p.vblank_mode2 = r.bool()?;
        p.stat_line = r.bool()?;
        p.irq_pending = r.u8()? & 0x1F;
        p.win_y_hit = r.bool()?;
        p.win_line = r.u8()?;
        p.win_drawn = r.bool()?;
        p.first_pixel_dot = r.u16()?;
        p.drawn_x = r.u8()?;
        p.obj_count = r.u8()?;
        r.bytes_into(&mut p.objs)?;
        if p.line >= LINES || p.ly >= LINES {
            return Err(StateError::Corrupt("ppu line"));
        }
        if p.dot >= LINE_DOTS || p.next_event == 0 || p.next_event > LINE_DOTS {
            return Err(StateError::Corrupt("ppu dot"));
        }
        if p.stat_mode > 3 || p.locks & !LOCK_ALL != 0 {
            return Err(StateError::Corrupt("ppu stat"));
        }
        if p.drawn_x as usize > SCREEN_W
            || p.obj_count as usize > MAX_LINE_OBJS
            || p.objs.iter().any(|&i| i >= 40)
        {
            return Err(StateError::Corrupt("ppu line render state"));
        }
        *self = p;
        Ok(())
    }
}
