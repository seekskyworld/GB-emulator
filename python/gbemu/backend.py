"""pokemon-agent ``Emulator`` backend on top of the gb-core C ABI (PLAN §3.8).

pokemon-agent's server runs every emulator call through ``_run_sync`` (a
thread pool), so the monitor's ``/state`` and ``/screenshot`` and the driver's
``/action`` can enter the same instance concurrently.  The native instance is
not thread safe: **every native call happens while holding ``self._lock``**.

``/games/new`` replaces the emulator without calling ``close()`` on the old
one, so ``__del__`` also frees the native handle.  ``close()`` is idempotent.
"""

from __future__ import annotations

import os
import threading
from pathlib import Path
from typing import Dict, Optional

from PIL import Image
from pokemon_agent.emulator import Emulator

from gbemu import native
from gbemu.native import GbEmuError

# Bit positions of Machine::set_buttons (gb_core::buttons): 1 = pressed.
BUTTON_BITS: Dict[str, int] = {
    "a": 1 << 0,
    "b": 1 << 1,
    "select": 1 << 2,
    "start": 1 << 3,
    "right": 1 << 4,
    "left": 1 << 5,
    "up": 1 << 6,
    "down": 1 << 7,
}


def button_bit(button: str) -> int:
    """Mask bit for a pokemon-agent button name; unknown names raise ``ValueError``."""
    if not isinstance(button, str):
        raise ValueError(f"Unknown button {button!r}. Valid: {Emulator.BUTTONS}")
    bit = BUTTON_BITS.get(button.lower())
    if bit is None:
        raise ValueError(f"Unknown button '{button}'. Valid: {Emulator.BUTTONS}")
    return bit


def _frames(frames) -> int:
    if isinstance(frames, bool) or not isinstance(frames, int):
        raise ValueError(f"frames must be an int, got {frames!r}")
    if frames < 0:
        raise ValueError(f"frames must be >= 0, got {frames}")
    return frames


class GbEmuEmulator(Emulator):
    """Game Boy (DMG) backend for ``.gb``/``.gbc`` ROMs, backed by ``GBEMU_LIB``."""

    def __init__(self) -> None:
        super().__init__()
        self._lock = threading.Lock()
        self._handle: Optional[int] = None
        self._buttons = 0
        self._state_size_hint = 1 << 18
        # Fails fast (with a clear message) when GBEMU_LIB is missing.
        self._lib = native.load_library()

    # -- lifecycle ----------------------------------------------------------

    def load(self, rom_path: str) -> None:
        rom_path = str(Path(rom_path).expanduser().resolve())
        if not os.path.isfile(rom_path):
            raise FileNotFoundError(f"ROM not found: {rom_path}")
        with open(rom_path, "rb") as f:
            rom = f.read()
        with self._lock:
            handle = native.new(self._lib, rom)  # raises GbEmuError on a bad ROM
            old, self._handle = self._handle, handle
            if old:
                native.free(self._lib, old)
            self._buttons = 0
            self.rom_path = rom_path
            self.frame_count = 0

    def close(self) -> None:
        lock = getattr(self, "_lock", None)
        if lock is None:  # __init__ failed before the lock existed
            return
        with lock:
            self._free_locked()

    def __del__(self) -> None:
        # No lock here: when __del__ runs nobody else holds a reference.
        try:
            self._free_locked()
        except Exception:  # noqa: BLE001 - never raise from a finaliser
            pass

    def _free_locked(self) -> None:
        handle = getattr(self, "_handle", None)
        if handle:
            self._handle = None
            native.free(self._lib, handle)

    def _h(self) -> int:
        """The live native handle (caller holds the lock)."""
        if not self._handle:
            raise GbEmuError("emulator is not loaded (call load() first) or already closed")
        return self._handle

    # -- input --------------------------------------------------------------

    def press(self, button: str, frames: int = 1) -> None:
        bit = button_bit(button)
        frames = _frames(frames)
        with self._lock:
            h = self._h()
            self._buttons |= bit
            native.set_buttons(self._lib, h, self._buttons)
            try:
                self._tick_locked(h, frames)
            finally:
                self._buttons &= ~bit
                native.set_buttons(self._lib, h, self._buttons)

    def release_all(self) -> None:
        with self._lock:
            self._buttons = 0
            native.set_buttons(self._lib, self._h(), 0)

    # -- timing -------------------------------------------------------------

    def tick(self, frames: int = 1) -> None:
        frames = _frames(frames)
        with self._lock:
            self._tick_locked(self._h(), frames)

    def _tick_locked(self, h: int, frames: int) -> None:
        if frames:
            native.run_frames(self._lib, h, frames)
            self.frame_count += frames

    # -- video --------------------------------------------------------------

    def get_screen(self) -> Image.Image:
        with self._lock:
            buf = native.frame(self._lib, self._h())
        return Image.frombytes("L", (native.FRAME_W, native.FRAME_H), buf).convert("RGB")

    # -- memory (side-effect-free peeks) ------------------------------------

    def read_u8(self, addr: int) -> int:
        with self._lock:
            return native.peek(self._lib, self._h(), addr & 0xFFFF)

    def read_u16(self, addr: int) -> int:
        return int.from_bytes(self.read_range(addr, 2), "little")

    def read_u32(self, addr: int) -> int:
        return int.from_bytes(self.read_range(addr, 4), "little")

    def read_range(self, addr: int, size: int) -> bytes:
        if size <= 0:
            return b""
        with self._lock:
            return native.peek_range(self._lib, self._h(), addr & 0xFFFF, size)

    # -- save / load --------------------------------------------------------

    def save_state(self, path: str) -> None:
        path = str(Path(path).expanduser().resolve())
        with self._lock:
            data = native.save_state(self._lib, self._h(), self._state_size_hint)
        self._state_size_hint = max(self._state_size_hint, len(data))
        tmp = path + ".tmp"
        with open(tmp, "wb") as f:
            f.write(data)
        os.replace(tmp, path)

    def load_state(self, path: str) -> None:
        path = str(Path(path).expanduser().resolve())
        with open(path, "rb") as f:
            data = f.read()
        with self._lock:
            native.load_state(self._lib, self._h(), data)

    # -- extras (not part of the pokemon-agent interface) --------------------

    def regs(self) -> dict:
        with self._lock:
            return native.regs(self._lib, self._h())

    def take_serial(self) -> bytes:
        with self._lock:
            return native.take_serial(self._lib, self._h())

    def get_info(self) -> Dict:
        info = super().get_info()
        info["platform"] = "GB"
        info["library"] = native.loaded_library_path()
        return info


__all__ = ["GbEmuEmulator", "GbEmuError", "BUTTON_BITS", "button_bit"]
