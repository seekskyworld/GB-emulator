//! 运行辅助：静默 panic 捕获、带 PC 轨迹的逐帧推进。

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;

use gb_core::{Machine, FRAME_T};

/// PC 轨迹长度。
pub const TRACE_LEN: usize = 32;

/// 末尾 `TRACE_LEN` 条 PC 的环形缓冲（记录的是每次 `step` 之前的 PC）。
#[derive(Debug, Clone)]
pub struct PcTrace {
    buf: [u16; TRACE_LEN],
    len: usize,
    next: usize,
}

impl Default for PcTrace {
    fn default() -> Self {
        Self { buf: [0; TRACE_LEN], len: 0, next: 0 }
    }
}

impl PcTrace {
    #[inline]
    pub fn push(&mut self, pc: u16) {
        self.buf[self.next] = pc;
        self.next = (self.next + 1) % TRACE_LEN;
        if self.len < TRACE_LEN {
            self.len += 1;
        }
    }
    /// 从旧到新。
    pub fn to_vec(&self) -> Vec<u16> {
        let start = (self.next + TRACE_LEN - self.len) % TRACE_LEN;
        (0..self.len).map(|i| self.buf[(start + i) % TRACE_LEN]).collect()
    }
    pub fn format(&self) -> String {
        let v = self.to_vec();
        if v.is_empty() {
            return "(empty)".to_string();
        }
        v.iter().map(|pc| format!("{pc:04X}")).collect::<Vec<_>>().join(" ")
    }
}

/// 按 `Machine::run_frame` 的同一语义（每帧加 70224 T 的预算，执行到预算不大于 0，余量累计）
/// 逐条 `step` 推进一帧，同时记录 PC 轨迹。`step` 返回 0 时报错而不是死循环。
pub struct FrameStepper {
    budget: i64,
}

impl Default for FrameStepper {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameStepper {
    pub fn new() -> Self {
        Self { budget: 0 }
    }

    pub fn run_frame(&mut self, m: &mut Machine, trace: &mut PcTrace) -> Result<(), String> {
        self.budget += FRAME_T as i64;
        while self.budget > 0 {
            trace.push(m.regs().pc);
            let t = m.step();
            if t == 0 {
                return Err(format!("Machine::step returned 0 T at PC={:04X}", m.regs().pc));
            }
            self.budget -= t as i64;
        }
        Ok(())
    }

    /// 同上，但每条指令后检查 `ld_b_b_hit`；命中时立即返回 `Ok(true)`（本帧剩余预算保留）。
    pub fn run_frame_until_ld_b_b(
        &mut self,
        m: &mut Machine,
        trace: &mut PcTrace,
    ) -> Result<bool, String> {
        if self.budget <= 0 {
            self.budget += FRAME_T as i64;
        }
        while self.budget > 0 {
            trace.push(m.regs().pc);
            let t = m.step();
            if t == 0 {
                return Err(format!("Machine::step returned 0 T at PC={:04X}", m.regs().pc));
            }
            self.budget -= t as i64;
            if m.ld_b_b_hit() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

static HOOK: Once = Once::new();

fn payload_str(p: &(dyn Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// 安装一次全局 panic hook：在 `catch` 内发生的 panic 不打印，只记录"消息 at 文件:行"；
/// 其余 panic 交给默认 hook。
pub fn install_quiet_hook() {
    HOOK.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if QUIET.with(Cell::get) {
                let msg = payload_str(info.payload());
                let full = match info.location() {
                    Some(l) => format!("{msg} at {}:{}", l.file(), l.line()),
                    None => msg,
                };
                LAST_PANIC.with(|c| *c.borrow_mut() = Some(full));
            } else {
                default(info);
            }
        }));
    });
}

/// 运行 `f`，把 panic 转成 `Err(消息)`，且不往 stderr 刷屏。
pub fn catch<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    install_quiet_hook();
    let prev = QUIET.with(|q| q.replace(true));
    LAST_PANIC.with(|c| c.borrow_mut().take());
    let r = panic::catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(prev));
    r.map_err(|p| LAST_PANIC.with(|c| c.borrow_mut().take()).unwrap_or_else(|| payload_str(&*p)))
}
