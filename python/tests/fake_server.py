"""A pure-Python stand-in for the native backend, served through gbemu.launch.

Used by the offline tests of gbemu.launch, tools/playtest.py and
tools/scripted_driver.py:

    python fake_server.py serve --rom x.gb --port P --data-dir D

The fake emulator writes the pokered WRAM fields that pokemon-agent reads
(map id, x/y, names) so the monitor sees movement.  Knobs (environment):
``FAKE_MONO=1`` (every screen one colour), ``FAKE_FROZEN=1`` (tick does not
advance frame_count), ``FAKE_NAMES=1`` (names already set at boot).
"""

from __future__ import annotations

import os
import sys
import threading

from PIL import Image
from pokemon_agent.emulator import Emulator

# pokemon_agent.memory.red addresses (read by the server's state builder)
MAP_ID, MAP_Y, MAP_X = 0xD35E, 0xD361, 0xD362
PLAYER_NAME, RIVAL_NAME = 0xD158, 0xD34A
LETTERS = {c: 0x80 + i for i, c in enumerate("ABCDEFGHIJKLMNOPQRSTUVWXYZ")}


def encode(name: str) -> bytes:
    return bytes(LETTERS[c] for c in name) + b"\x50"


class FakeEmulator(Emulator):
    def __init__(self) -> None:
        super().__init__()
        self._lock = threading.Lock()
        self.mem = bytearray(0x10000)
        self.presses = 0
        self.mono = os.environ.get("FAKE_MONO") == "1"
        self.frozen = os.environ.get("FAKE_FROZEN") == "1"

    def load(self, rom_path: str) -> None:
        self.rom_path = rom_path
        self.frame_count = 0
        self.mem[PLAYER_NAME] = 0x50
        self.mem[RIVAL_NAME] = 0x50
        if os.environ.get("FAKE_NAMES") == "1":
            self._set_names()
        self.mem[MAP_ID], self.mem[MAP_X], self.mem[MAP_Y] = 38, 3, 6

    def _set_names(self) -> None:
        self.mem[PLAYER_NAME:PLAYER_NAME + 4] = encode("RED")
        self.mem[RIVAL_NAME:RIVAL_NAME + 5] = encode("BLUE")

    def close(self) -> None:
        pass

    def press(self, button: str, frames: int = 1) -> None:
        button = button.lower()
        if button not in self.BUTTONS:
            raise ValueError(f"Unknown button '{button}'")
        with self._lock:
            self.presses += 1
            if self.presses == 40:
                self._set_names()
            dx, dy = {"left": (-1, 0), "right": (1, 0), "up": (0, -1), "down": (0, 1)}.get(button, (0, 0))
            self.mem[MAP_X] = (self.mem[MAP_X] + dx) % 20
            self.mem[MAP_Y] = (self.mem[MAP_Y] + dy) % 20
            if self.presses % 50 == 0:
                self.mem[MAP_ID] = (self.mem[MAP_ID] + 1) % 0xF8
        self.tick(frames)

    def release_all(self) -> None:
        pass

    def tick(self, frames: int = 1) -> None:
        with self._lock:
            if not self.frozen:
                self.frame_count += frames

    def get_screen(self) -> Image.Image:
        if self.mono:
            return Image.new("RGB", (160, 144), (255, 255, 255))
        img = Image.new("L", (160, 144), 0xFF)
        with self._lock:
            x, y = self.mem[MAP_X], self.mem[MAP_Y]
        for i in range(16):
            for j in range(16):
                img.putpixel(((x * 8 + i) % 160, (y * 7 + j) % 144), 0x00)
        return img.convert("RGB")

    def read_u8(self, addr: int) -> int:
        return self.mem[addr & 0xFFFF]

    def read_u16(self, addr: int) -> int:
        return self.read_u8(addr) | (self.read_u8(addr + 1) << 8)

    def read_u32(self, addr: int) -> int:
        return int.from_bytes(self.read_range(addr, 4), "little")

    def read_range(self, addr: int, size: int) -> bytes:
        return bytes(self.mem[(addr + i) & 0xFFFF] for i in range(max(0, size)))

    def save_state(self, path: str) -> None:
        with open(path, "wb") as f:
            f.write(bytes(self.mem))

    def load_state(self, path: str) -> None:
        with open(path, "rb") as f:
            self.mem[:] = f.read()


def main() -> None:
    from gbemu import launch

    launch.install()
    import pokemon_agent.cli as pa_cli
    import pokemon_agent.emulator as pa_emulator

    pa_emulator._EXT_MAP[".gb"] = FakeEmulator
    launch.instrument()
    assert "pyboy" not in sys.modules
    sys.argv = ["pokemon-agent", *sys.argv[1:]]
    pa_cli.main()


if __name__ == "__main__":
    main()
