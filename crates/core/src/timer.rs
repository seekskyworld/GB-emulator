//! 定时器（sys 通道，T15；PLAN §3.4；Pan Docs Timer_and_Divider_Registers、Timer_Obscure_Behaviour）。
//!
//! 模型：
//! - 16 位系统计数器每 T 加 1（本模块每个 M-cycle 加 4），DIV（FF04）是它的高 8 位；写 DIV 或执行 STOP
//!   清零整个计数器。
//! - TIMA 在"所选位 AND TAC.enable"信号的**下降沿**加 1（TAC 低 2 位 00→bit9、01→bit3、10→bit5、11→bit7）。
//!   因此写 DIV、改 TAC（换位或关闭）都可能让 TIMA 额外加 1（DMG 行为）。
//! - TIMA 溢出的那个 M-cycle（下文"周期 A"）TIMA 读出 0x00；下一个 M-cycle（"周期 B"）开头装入 TMA 并
//!   请求定时器中断（IF bit2）。
//! - 周期 A 内写 TIMA：取消这次重载与中断，写入值保留。
//! - 周期 B 内写 TIMA：被忽略（TIMA 仍等于 TMA）；周期 B 内写 TMA：新值同时写进 TIMA。
//!
//! 与 D002 的关系：Bus 在每个 M-cycle 先调用 `tick_mcycle`，再执行 CPU 的访问。所以 `tick_mcycle` 把
//! 本周期的状态推进到位，随后同一周期的 `read_reg`/`write_reg` 看到的是"周期 A/B 内"的状态。

use crate::state::{Snapshot, StateError, StateReader, StateWriter};
use crate::{irq, Peripheral};

/// TAC 低 2 位选择的系统计数器位（T-cycle 计数器中的位掩码）。
const TAC_BIT: [u16; 4] = [1 << 9, 1 << 3, 1 << 5, 1 << 7];

/// 开机（PC=0x0100 时）的系统计数器值：DIV 读出 0xAB（Power_Up_Sequence）。
///
/// 低 8 位（DIV 内部相位）不是实机测量值，而是依据一份测试 ROM 文档对开机 DIV 相位的说明推出的（D034）：
/// 从 PC=0x0100 起第 14 个 M-cycle 的访问恰好在 DIV 加 1 之后读到 0xAC。按 D002（每个 M-cycle 先推进 4 T 再访问），第 k 个访问看到 `BOOT + 4k`，
/// 所以 `BOOT = 0xAC00 - 14*4 = 0xABC8`。若 T30 全局改为"先访问再推进"，此值要改为 0xABCC。
pub const BOOT_COUNTER: u16 = 0xABC8;

/// TIMA 溢出后的重载流水线。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Reload {
    #[default]
    Idle,
    /// 周期 A：TIMA 刚溢出为 0x00，下一个 M-cycle 装入 TMA 并请求中断。
    Overflowed,
    /// 周期 B：本 M-cycle 开头已装入 TMA（持续到本周期的访问结束）。
    Reloading,
}

impl Reload {
    fn to_u8(self) -> u8 {
        match self {
            Reload::Idle => 0,
            Reload::Overflowed => 1,
            Reload::Reloading => 2,
        }
    }

    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Reload::Idle),
            1 => Some(Reload::Overflowed),
            2 => Some(Reload::Reloading),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    /// 16 位系统计数器（T-cycle）；DIV = 高 8 位。
    counter: u16,
    tima: u8,
    tma: u8,
    /// TAC 的低 3 位（bit2 = enable，bit1–0 = clock select）。
    tac: u8,
    reload: Reload,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    /// DMG 开机：DIV=AB、TIMA=00、TMA=00、TAC=F8（读出值）。
    pub fn new() -> Self {
        Self { counter: BOOT_COUNTER, tima: 0x00, tma: 0x00, tac: 0x00, reload: Reload::Idle }
    }

    /// 写 DIV 或执行 STOP：清零系统计数器（可能触发一次 TIMA 加 1）。
    pub fn reset_div(&mut self) {
        self.set_counter(0);
    }

    /// 系统计数器的完整 16 位值（调试与测试用）。
    pub fn counter(&self) -> u16 {
        self.counter
    }

    /// 送往下降沿检测器的信号：所选位 AND TAC.enable。
    fn signal(&self) -> bool {
        self.tac & 0x04 != 0 && self.counter & TAC_BIT[(self.tac & 0x03) as usize] != 0
    }

    /// 改变系统计数器；所选信号出现下降沿时 TIMA 加 1。
    fn set_counter(&mut self, value: u16) {
        let before = self.signal();
        self.counter = value;
        if before && !self.signal() {
            self.increment();
        }
    }

    fn increment(&mut self) {
        let (v, overflow) = self.tima.overflowing_add(1);
        self.tima = v;
        if overflow {
            self.reload = Reload::Overflowed;
        }
    }
}

impl Peripheral for Timer {
    fn tick_mcycle(&mut self) -> u8 {
        let mut req = 0;
        match self.reload {
            Reload::Overflowed => {
                self.tima = self.tma;
                self.reload = Reload::Reloading;
                req = irq::TIMER;
            }
            Reload::Reloading => self.reload = Reload::Idle,
            Reload::Idle => {}
        }
        // 计数器低 2 位在整个运行中保持不变，所选位（≥ bit3）在 +4 之内最多翻转一次。
        self.set_counter(self.counter.wrapping_add(4));
        req
    }

    fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => 0xF8 | self.tac,
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, addr: u16, v: u8) {
        match addr {
            0xFF04 => self.reset_div(),
            0xFF05 => match self.reload {
                // 周期 A：写入取消重载与中断。
                Reload::Overflowed => {
                    self.tima = v;
                    self.reload = Reload::Idle;
                }
                // 周期 B：TIMA 持续装载 TMA，写入被覆盖。
                Reload::Reloading => {}
                Reload::Idle => self.tima = v,
            },
            0xFF06 => {
                self.tma = v;
                if self.reload == Reload::Reloading {
                    self.tima = v;
                }
            }
            0xFF07 => {
                let before = self.signal();
                self.tac = v & 0x07;
                if before && !self.signal() {
                    self.increment();
                }
            }
            _ => {}
        }
    }
}

impl Snapshot for Timer {
    fn save(&self, w: &mut StateWriter) {
        w.tag(b"TIMR");
        w.u16(self.counter);
        w.u8(self.tima);
        w.u8(self.tma);
        w.u8(self.tac);
        w.u8(self.reload.to_u8());
    }

    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError> {
        r.tag(b"TIMR")?;
        let counter = r.u16()?;
        let tima = r.u8()?;
        let tma = r.u8()?;
        let tac = r.u8()?;
        let reload = Reload::from_u8(r.u8()?).ok_or(StateError::Corrupt("timer: reload state"))?;
        if tac & !0x07 != 0 {
            return Err(StateError::Corrupt("timer: TAC upper bits"));
        }
        *self = Self { counter, tima, tma, tac, reload };
        Ok(())
    }
}
