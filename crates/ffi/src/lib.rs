//! C ABI（PLAN §3.8），供 Python `ctypes` 后端（`python/gbemu/native.py`）加载。
//!
//! 约定：
//! - 句柄 `*mut Gb` 由 `gb_new` 创建、`gb_free` 释放；**实例不是线程安全的**，调用方必须加锁。
//! - 每个入口都用 `catch_unwind` 包住：核心 panic 时返回 `GB_ERR_PANIC`，并把信息记入
//!   线程局部的 last_error（`gb_last_error` 读取）。发生过 panic 的实例被标记为 poisoned，
//!   之后除 `gb_free` 外的调用都返回 `GB_ERR_POISONED`（机器状态可能已不一致）。
//! - 返回 `i32` 的入口：`>= 0` 成功（`gb_peek` 返回字节值 0..=255），负数为错误码。
//! - 返回 `i64` 的入口：`>= 0` 为写入的字节数；错误码在 `[-16, -1]`；`gb_save_state`
//!   在 `cap` 不足时返回 `-(所需长度)`（存档总大于 16 字节，因此与错误码不重叠）。
//! - 错误码之外的详细原因一律通过 `gb_last_error` 取得。

use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;

use gb_core::{Machine, FRAME_PIXELS};

/// 成功。
pub const GB_OK: i32 = 0;
/// 句柄或指针为 null、参数非法。
pub const GB_ERR_ARG: i32 = -1;
/// 核心 panic（已被捕获）。
pub const GB_ERR_PANIC: i32 = -2;
/// 读档失败（`StateError`）。
pub const GB_ERR_STATE: i32 = -3;
/// 实例此前发生过 panic，拒绝继续使用。
pub const GB_ERR_POISONED: i32 = -4;
/// 错误码的下界：`[-16, -1]` 都是错误码。
pub const GB_ERR_MIN: i32 = -16;

/// 不透明句柄。
pub struct Gb {
    machine: Machine,
    /// 串口输出中尚未被 `gb_take_serial` 取走的部分（`cap` 不足时保留余量）。
    serial_pending: Vec<u8>,
    poisoned: Option<String>,
}

/// `gb_regs` 的输出，与 Python 侧 `GbRegs(ctypes.Structure)` 一一对应。
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct GbRegs {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: u8,
    pub halted: u8,
}

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
    static PANIC_LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

static HOOK: Once = Once::new();

/// 包一层 panic hook：记录 panic 位置（供 last_error 使用），然后照常调用原 hook（打印到 stderr）。
fn install_hook() {
    HOOK.call_once(|| {
        let prev = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if let Some(loc) = info.location() {
                let s = format!("{}:{}", loc.file(), loc.line());
                PANIC_LOCATION.with(|p| *p.borrow_mut() = Some(s));
            }
            prev(info);
        }));
    });
}

fn set_error(msg: impl Into<String>) {
    let msg = msg.into();
    LAST_ERROR.with(|e| *e.borrow_mut() = msg);
}

fn clear_error() {
    LAST_ERROR.with(|e| e.borrow_mut().clear());
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    let msg = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    };
    match PANIC_LOCATION.with(|p| p.borrow_mut().take()) {
        Some(loc) => format!("panic at {loc}: {msg}"),
        None => format!("panic: {msg}"),
    }
}

/// 在 `catch_unwind` 中执行 `f`；panic 时记录 last_error 并返回 `None`。
fn guard<R>(what: &str, f: impl FnOnce() -> R) -> Option<R> {
    install_hook();
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => Some(r),
        Err(payload) => {
            set_error(format!("{what}: {}", panic_message(&*payload)));
            None
        }
    }
}

/// 取得可用的实例引用；null 或 poisoned 时记录错误并返回错误码。
///
/// # Safety
/// `gb` 必须为 null 或 `gb_new` 返回且尚未释放的指针。
unsafe fn instance<'a>(gb: *mut Gb, what: &str) -> Result<&'a mut Gb, i32> {
    if gb.is_null() {
        set_error(format!("{what}: null handle"));
        return Err(GB_ERR_ARG);
    }
    let g = &mut *gb;
    if let Some(p) = &g.poisoned {
        set_error(format!("{what}: instance poisoned by an earlier panic ({p})"));
        return Err(GB_ERR_POISONED);
    }
    Ok(g)
}

/// 对实例执行 `f`（`catch_unwind` 保护）；panic 时把实例标记为 poisoned。
///
/// # Safety
/// 同 `instance`。
unsafe fn with_gb<R>(gb: *mut Gb, what: &str, f: impl FnOnce(&mut Gb) -> R) -> Result<R, i32> {
    clear_error();
    let g = instance(gb, what)?;
    match guard(what, || f(g)) {
        Some(r) => Ok(r),
        None => {
            // guard 已写入 last_error；实例此后拒绝使用。
            let msg = LAST_ERROR.with(|e| e.borrow().clone());
            (*gb).poisoned = Some(msg);
            Err(GB_ERR_PANIC)
        }
    }
}

fn code(r: Result<(), i32>) -> i32 {
    match r {
        Ok(()) => GB_OK,
        Err(c) => c,
    }
}

/// 从 ROM 字节创建实例；失败（ROM 非法、panic）返回 null，原因见 `gb_last_error`。
///
/// # Safety
/// `ptr` 必须指向至少 `len` 个可读字节（`len == 0` 时可为 null）。
#[no_mangle]
pub unsafe extern "C" fn gb_new(ptr: *const u8, len: usize) -> *mut Gb {
    clear_error();
    if ptr.is_null() && len != 0 {
        set_error("gb_new: null ROM pointer");
        return std::ptr::null_mut();
    }
    let rom = if len == 0 { Vec::new() } else { std::slice::from_raw_parts(ptr, len).to_vec() };
    match guard("gb_new", || Machine::new(rom)) {
        Some(Ok(machine)) => {
            Box::into_raw(Box::new(Gb { machine, serial_pending: Vec::new(), poisoned: None }))
        }
        Some(Err(e)) => {
            set_error(format!("gb_new: {e}"));
            std::ptr::null_mut()
        }
        None => std::ptr::null_mut(),
    }
}

/// 释放实例；`gb` 为 null 时什么也不做。同一指针只能释放一次（Python 侧负责置空）。
///
/// # Safety
/// `gb` 必须为 null 或 `gb_new` 返回且尚未释放的指针。
#[no_mangle]
pub unsafe extern "C" fn gb_free(gb: *mut Gb) {
    if gb.is_null() {
        return;
    }
    let b = Box::from_raw(gb);
    // Drop 本身不应 panic；万一 panic 也不能越过 FFI 边界。
    let _ = guard("gb_free", move || drop(b));
}

/// 推进 `n` 帧（每帧精确 70224 T，与 LCD 开关无关）。
///
/// # Safety
/// `gb` 同 `gb_free`。
#[no_mangle]
pub unsafe extern "C" fn gb_run_frames(gb: *mut Gb, n: u32) -> i32 {
    code(with_gb(gb, "gb_run_frames", |g| {
        for _ in 0..n {
            g.machine.run_frame();
        }
    }))
}

/// 设置按键：bit0..7 = A B Select Start Right Left Up Down，1 = 按下。
///
/// # Safety
/// `gb` 同 `gb_free`。
#[no_mangle]
pub unsafe extern "C" fn gb_set_buttons(gb: *mut Gb, mask: u8) -> i32 {
    code(with_gb(gb, "gb_set_buttons", |g| g.machine.set_buttons(mask)))
}

/// 把最近一个完整帧（23040 字节灰度，0xFF/0xAA/0x55/0x00）复制到 `out`。
///
/// # Safety
/// `gb` 同 `gb_free`；`out` 必须指向至少 23040 个可写字节。
#[no_mangle]
pub unsafe extern "C" fn gb_frame(gb: *mut Gb, out: *mut u8) -> i32 {
    if out.is_null() {
        set_error("gb_frame: null output buffer");
        return GB_ERR_ARG;
    }
    code(with_gb(gb, "gb_frame", |g| {
        let frame = g.machine.frame();
        std::ptr::copy_nonoverlapping(frame.as_ptr(), out, FRAME_PIXELS);
    }))
}

/// 无副作用读 1 字节：返回 0..=255，错误时返回负的错误码。
///
/// # Safety
/// `gb` 同 `gb_free`。
#[no_mangle]
pub unsafe extern "C" fn gb_peek(gb: *mut Gb, addr: u16) -> i32 {
    match with_gb(gb, "gb_peek", |g| g.machine.peek(addr)) {
        Ok(v) => v as i32,
        Err(c) => c,
    }
}

/// 无副作用读 `len` 字节到 `out`，地址按 16 位回绕（0xFFFF 之后是 0x0000）。
///
/// # Safety
/// `gb` 同 `gb_free`；`out` 必须指向至少 `len` 个可写字节（`len == 0` 时可为 null）。
#[no_mangle]
pub unsafe extern "C" fn gb_peek_range(gb: *mut Gb, addr: u16, len: usize, out: *mut u8) -> i32 {
    if out.is_null() && len != 0 {
        set_error("gb_peek_range: null output buffer");
        return GB_ERR_ARG;
    }
    code(with_gb(gb, "gb_peek_range", |g| {
        if len == 0 {
            return;
        }
        let dst = std::slice::from_raw_parts_mut(out, len);
        for (i, b) in dst.iter_mut().enumerate() {
            *b = g.machine.peek(addr.wrapping_add(i as u16));
        }
    }))
}

/// 存档到 `out`：成功返回写入的字节数；`cap` 不足时不写入，返回 `-(所需长度)`；错误返回错误码。
///
/// # Safety
/// `gb` 同 `gb_free`；`out` 必须指向至少 `cap` 个可写字节（`cap == 0` 时可为 null）。
#[no_mangle]
pub unsafe extern "C" fn gb_save_state(gb: *mut Gb, out: *mut u8, cap: usize) -> i64 {
    match with_gb(gb, "gb_save_state", |g| g.machine.save_state()) {
        Ok(data) => {
            if data.len() > cap {
                return -(data.len() as i64);
            }
            if out.is_null() {
                set_error("gb_save_state: null output buffer");
                return GB_ERR_ARG as i64;
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), out, data.len());
            data.len() as i64
        }
        Err(c) => c as i64,
    }
}

/// 读档；失败（格式错误、ROM 指纹不符）返回 `GB_ERR_STATE`，机器状态保持不变。
///
/// # Safety
/// `gb` 同 `gb_free`；`ptr` 必须指向至少 `len` 个可读字节（`len == 0` 时可为 null）。
#[no_mangle]
pub unsafe extern "C" fn gb_load_state(gb: *mut Gb, ptr: *const u8, len: usize) -> i32 {
    if ptr.is_null() && len != 0 {
        set_error("gb_load_state: null input buffer");
        return GB_ERR_ARG;
    }
    let data: &[u8] = if len == 0 { &[] } else { std::slice::from_raw_parts(ptr, len) };
    match with_gb(gb, "gb_load_state", |g| g.machine.load_state(data)) {
        Ok(Ok(())) => GB_OK,
        Ok(Err(e)) => {
            set_error(format!("gb_load_state: {e}"));
            GB_ERR_STATE
        }
        Err(c) => c,
    }
}

/// 读寄存器快照到 `out`。
///
/// # Safety
/// `gb` 同 `gb_free`；`out` 必须指向一个可写的 `GbRegs`。
#[no_mangle]
pub unsafe extern "C" fn gb_regs(gb: *mut Gb, out: *mut GbRegs) -> i32 {
    if out.is_null() {
        set_error("gb_regs: null output");
        return GB_ERR_ARG;
    }
    code(with_gb(gb, "gb_regs", |g| {
        let r = g.machine.regs();
        *out = GbRegs {
            a: r.a,
            f: r.f,
            b: r.b,
            c: r.c,
            d: r.d,
            e: r.e,
            h: r.h,
            l: r.l,
            sp: r.sp,
            pc: r.pc,
            ime: r.ime as u8,
            halted: r.halted as u8,
        };
    }))
}

/// 取走最多 `cap` 字节的串口输出，返回取走的字节数；未取走的部分留到下次。
///
/// # Safety
/// `gb` 同 `gb_free`；`out` 必须指向至少 `cap` 个可写字节（`cap == 0` 时可为 null）。
#[no_mangle]
pub unsafe extern "C" fn gb_take_serial(gb: *mut Gb, out: *mut u8, cap: usize) -> i64 {
    if out.is_null() && cap != 0 {
        set_error("gb_take_serial: null output buffer");
        return GB_ERR_ARG as i64;
    }
    match with_gb(gb, "gb_take_serial", |g| {
        let fresh = g.machine.take_serial();
        g.serial_pending.extend_from_slice(&fresh);
        let n = g.serial_pending.len().min(cap);
        if n > 0 {
            std::ptr::copy_nonoverlapping(g.serial_pending.as_ptr(), out, n);
            g.serial_pending.drain(..n);
        }
        n as i64
    }) {
        Ok(n) => n,
        Err(c) => c as i64,
    }
}

/// 把当前线程最近一次错误的 UTF-8 文本复制到 `out`（截断到 `cap`，不写 NUL），
/// 返回完整长度；没有错误时返回 0。
///
/// # Safety
/// `out` 必须指向至少 `cap` 个可写字节（`cap == 0` 时可为 null）。
#[no_mangle]
pub unsafe extern "C" fn gb_last_error(out: *mut u8, cap: usize) -> i64 {
    LAST_ERROR.with(|e| {
        let e = e.borrow();
        let n = e.len().min(cap);
        if n > 0 && !out.is_null() {
            std::ptr::copy_nonoverlapping(e.as_ptr(), out, n);
        }
        e.len() as i64
    })
}

/// 诊断用：在 `catch_unwind` 内故意 panic，验证 panic → 错误码 + last_error 的通路。
/// 总是返回 `GB_ERR_PANIC`。
#[no_mangle]
pub extern "C" fn gb_selftest_panic() -> i32 {
    clear_error();
    match guard("gb_selftest_panic", || -> i32 { panic!("gb-ffi selftest panic") }) {
        Some(v) => v,
        None => GB_ERR_PANIC,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error() -> String {
        let mut buf = vec![0u8; 512];
        let n = unsafe { gb_last_error(buf.as_mut_ptr(), buf.len()) } as usize;
        buf.truncate(n.min(512));
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn bad_rom_returns_null_with_message() {
        let rom = [0u8; 16];
        let gb = unsafe { gb_new(rom.as_ptr(), rom.len()) };
        assert!(gb.is_null());
        assert!(last_error().contains("bad ROM size"), "{}", last_error());
        let gb = unsafe { gb_new(std::ptr::null(), 0) };
        assert!(gb.is_null());
    }

    #[test]
    fn null_handle_is_an_error_not_a_crash() {
        let mut buf = [0u8; FRAME_PIXELS];
        unsafe {
            assert_eq!(gb_run_frames(std::ptr::null_mut(), 1), GB_ERR_ARG);
            assert_eq!(gb_frame(std::ptr::null_mut(), buf.as_mut_ptr()), GB_ERR_ARG);
            assert_eq!(gb_peek(std::ptr::null_mut(), 0), GB_ERR_ARG);
            assert_eq!(gb_save_state(std::ptr::null_mut(), buf.as_mut_ptr(), 0), GB_ERR_ARG as i64);
            gb_free(std::ptr::null_mut());
        }
        assert!(last_error().contains("null handle"));
    }

    #[test]
    fn panic_becomes_error_code() {
        assert_eq!(gb_selftest_panic(), GB_ERR_PANIC);
        let e = last_error();
        assert!(e.contains("selftest panic"), "{e}");
    }

    #[test]
    fn last_error_reports_full_length_when_truncated() {
        gb_selftest_panic();
        let mut small = [0u8; 4];
        let n = unsafe { gb_last_error(small.as_mut_ptr(), small.len()) };
        assert!(n > 4);
    }

    #[test]
    fn regs_layout_is_stable() {
        // Python 侧的 ctypes.Structure 依赖这个布局：8 个 u8、2 个 u16、2 个 u8。
        assert_eq!(std::mem::size_of::<GbRegs>(), 14);
        assert_eq!(std::mem::align_of::<GbRegs>(), 2);
    }
}
