"""ctypes bindings for the ``gb-ffi`` C ABI (PLAN §3.8).

The shared library path is taken **only** from the ``GBEMU_LIB`` environment
variable (no search path, no fallback), so the build that is being exercised
is always explicit, e.g.::

    export GBEMU_LIB=/abs/path/target/release/libgb_ffi.dylib

Every function here is a thin, *unlocked* wrapper: a native instance is not
thread safe and callers (``gbemu.backend.GbEmuEmulator``) must serialise all
calls on one instance with a lock.  Errors are raised as :class:`GbEmuError`
carrying the text of ``gb_last_error``.
"""

from __future__ import annotations

import ctypes
import os
import threading
from ctypes import POINTER, c_int32, c_int64, c_size_t, c_uint8, c_uint16, c_uint32, c_void_p
from typing import Optional

ENV_LIB = "GBEMU_LIB"

FRAME_W = 160
FRAME_H = 144
FRAME_PIXELS = FRAME_W * FRAME_H

# Error codes (mirror crates/ffi/src/lib.rs).
GB_OK = 0
GB_ERR_ARG = -1
GB_ERR_PANIC = -2
GB_ERR_STATE = -3
GB_ERR_POISONED = -4
GB_ERR_MIN = -16

_ERR_NAMES = {
    GB_ERR_ARG: "invalid argument",
    GB_ERR_PANIC: "core panic",
    GB_ERR_STATE: "save state rejected",
    GB_ERR_POISONED: "instance poisoned",
}


class GbEmuError(RuntimeError):
    """A native call failed; ``code`` is the gb-ffi error code (or ``None``)."""

    def __init__(self, message: str, code: Optional[int] = None) -> None:
        super().__init__(message)
        self.code = code


class GbEmuLibraryError(GbEmuError):
    """``GBEMU_LIB`` is unset, points at nothing, or is not a gb-ffi library."""


class GbRegs(ctypes.Structure):
    """Mirror of ``#[repr(C)] struct GbRegs`` (14 bytes, align 2)."""

    _fields_ = [
        ("a", c_uint8),
        ("f", c_uint8),
        ("b", c_uint8),
        ("c", c_uint8),
        ("d", c_uint8),
        ("e", c_uint8),
        ("h", c_uint8),
        ("l", c_uint8),
        ("sp", c_uint16),
        ("pc", c_uint16),
        ("ime", c_uint8),
        ("halted", c_uint8),
    ]

    def to_dict(self) -> dict:
        d = {name: int(getattr(self, name)) for name, _ in self._fields_}
        d["ime"] = bool(d["ime"])
        d["halted"] = bool(d["halted"])
        return d


GbHandle = c_void_p
_u8p = POINTER(c_uint8)

# name -> (restype, argtypes)
_SIGNATURES = {
    "gb_new": (GbHandle, [_u8p, c_size_t]),
    "gb_free": (None, [GbHandle]),
    "gb_run_frames": (c_int32, [GbHandle, c_uint32]),
    "gb_set_buttons": (c_int32, [GbHandle, c_uint8]),
    "gb_frame": (c_int32, [GbHandle, _u8p]),
    "gb_peek": (c_int32, [GbHandle, c_uint16]),
    "gb_peek_range": (c_int32, [GbHandle, c_uint16, c_size_t, _u8p]),
    "gb_save_state": (c_int64, [GbHandle, _u8p, c_size_t]),
    "gb_load_state": (c_int32, [GbHandle, _u8p, c_size_t]),
    "gb_regs": (c_int32, [GbHandle, POINTER(GbRegs)]),
    "gb_take_serial": (c_int64, [GbHandle, _u8p, c_size_t]),
    "gb_last_error": (c_int64, [_u8p, c_size_t]),
    "gb_selftest_panic": (c_int32, []),
}

_lib: Optional[ctypes.CDLL] = None
_lib_path: Optional[str] = None
_lib_lock = threading.Lock()


def library_path() -> str:
    """Return the absolute library path from ``GBEMU_LIB`` or raise a clear error."""
    path = os.environ.get(ENV_LIB, "").strip()
    if not path:
        raise GbEmuLibraryError(
            f"{ENV_LIB} is not set. Build the C ABI with "
            "`cargo build -p gb-ffi --release` and export "
            f"{ENV_LIB}=/absolute/path/to/target/release/libgb_ffi.dylib"
        )
    path = os.path.abspath(os.path.expanduser(path))
    if not os.path.isfile(path):
        raise GbEmuLibraryError(f"{ENV_LIB}={path} does not exist (build gb-ffi first)")
    return path


def load_library() -> ctypes.CDLL:
    """Load (once per process) and type the gb-ffi library named by ``GBEMU_LIB``.

    A second call with a different ``GBEMU_LIB`` is an error: one process uses
    exactly one build.
    """
    global _lib, _lib_path
    path = library_path()
    with _lib_lock:
        if _lib is not None:
            if path != _lib_path:
                raise GbEmuLibraryError(
                    f"gb-ffi already loaded from {_lib_path}; refusing to switch to {path}"
                )
            return _lib
        try:
            lib = ctypes.CDLL(path)
        except OSError as exc:
            raise GbEmuLibraryError(f"cannot load {ENV_LIB}={path}: {exc}") from exc
        for name, (restype, argtypes) in _SIGNATURES.items():
            try:
                fn = getattr(lib, name)
            except AttributeError as exc:
                raise GbEmuLibraryError(f"{path} does not export {name}; not a gb-ffi build?") from exc
            fn.restype = restype
            fn.argtypes = argtypes
        _lib, _lib_path = lib, path
        return lib


def loaded_library_path() -> Optional[str]:
    return _lib_path


# ---------------------------------------------------------------------------
# Thin wrappers (unlocked; one native call each unless noted)
# ---------------------------------------------------------------------------


def last_error(lib: ctypes.CDLL) -> str:
    """Text of the most recent error on *this thread* (empty if none)."""
    buf = (c_uint8 * 1024)()
    n = lib.gb_last_error(buf, len(buf))
    if n > len(buf):
        buf = (c_uint8 * n)()
        n = lib.gb_last_error(buf, len(buf))
    return bytes(buf[: max(0, min(n, len(buf)))]).decode("utf-8", "replace")


def _raise(lib: ctypes.CDLL, what: str, code: int) -> None:
    msg = last_error(lib) or _ERR_NAMES.get(code, f"error {code}")
    raise GbEmuError(f"{what} failed ({_ERR_NAMES.get(code, code)}): {msg}", code)


def _check(lib: ctypes.CDLL, what: str, rc: int) -> int:
    if rc < 0:
        _raise(lib, what, rc)
    return rc


def _bytes_ptr(data: bytes):
    """A ``uint8_t*`` view of *data* (kept alive by the returned buffer)."""
    buf = (c_uint8 * len(data)).from_buffer_copy(data) if data else (c_uint8 * 1)()
    return buf


def new(lib: ctypes.CDLL, rom: bytes) -> int:
    buf = _bytes_ptr(rom)
    handle = lib.gb_new(buf, len(rom))
    if not handle:
        raise GbEmuError(f"gb_new rejected the ROM: {last_error(lib) or 'unknown error'}")
    return handle


def free(lib: ctypes.CDLL, handle: int) -> None:
    lib.gb_free(handle)


def run_frames(lib: ctypes.CDLL, handle: int, n: int) -> None:
    _check(lib, "gb_run_frames", lib.gb_run_frames(handle, n))


def set_buttons(lib: ctypes.CDLL, handle: int, mask: int) -> None:
    _check(lib, "gb_set_buttons", lib.gb_set_buttons(handle, mask & 0xFF))


def frame(lib: ctypes.CDLL, handle: int) -> bytes:
    buf = (c_uint8 * FRAME_PIXELS)()
    _check(lib, "gb_frame", lib.gb_frame(handle, buf))
    return bytes(buf)


def peek(lib: ctypes.CDLL, handle: int, addr: int) -> int:
    return _check(lib, "gb_peek", lib.gb_peek(handle, addr & 0xFFFF))


def peek_range(lib: ctypes.CDLL, handle: int, addr: int, size: int) -> bytes:
    if size <= 0:
        return b""
    buf = (c_uint8 * size)()
    _check(lib, "gb_peek_range", lib.gb_peek_range(handle, addr & 0xFFFF, size, buf))
    return bytes(buf)


def save_state(lib: ctypes.CDLL, handle: int, hint: int = 1 << 18) -> bytes:
    """Serialise the machine (may call the native side twice to size the buffer)."""
    cap = max(hint, 64)
    for _ in range(3):
        buf = (c_uint8 * cap)()
        n = lib.gb_save_state(handle, buf, cap)
        if n >= 0:
            return bytes(buf[:n])
        if n >= GB_ERR_MIN:
            _raise(lib, "gb_save_state", int(n))
        cap = -n
    raise GbEmuError("gb_save_state: state size kept changing between calls")


def load_state(lib: ctypes.CDLL, handle: int, data: bytes) -> None:
    buf = _bytes_ptr(data)
    _check(lib, "gb_load_state", lib.gb_load_state(handle, buf, len(data)))


def regs(lib: ctypes.CDLL, handle: int) -> dict:
    out = GbRegs()
    _check(lib, "gb_regs", lib.gb_regs(handle, ctypes.byref(out)))
    return out.to_dict()


def take_serial(lib: ctypes.CDLL, handle: int) -> bytes:
    chunks = []
    while True:
        buf = (c_uint8 * 4096)()
        n = lib.gb_take_serial(handle, buf, len(buf))
        if n < 0:
            _raise(lib, "gb_take_serial", int(n))
        chunks.append(bytes(buf[:n]))
        if n < len(buf):
            return b"".join(chunks)


def selftest_panic(lib: ctypes.CDLL) -> None:
    """Trigger a caught panic inside the library; always raises GbEmuError."""
    _raise(lib, "gb_selftest_panic", lib.gb_selftest_panic())


__all__ = [
    "ENV_LIB",
    "FRAME_W",
    "FRAME_H",
    "FRAME_PIXELS",
    "GbEmuError",
    "GbEmuLibraryError",
    "GbRegs",
    "library_path",
    "load_library",
    "loaded_library_path",
    "last_error",
]
