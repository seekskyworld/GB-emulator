"""T53b: backend behaviour on a whole machine (marked ``machine``; skipped until
``Machine`` can run, see conftest.running_emu)."""

from __future__ import annotations

import os
import threading
from pathlib import Path

import pytest

pytestmark = pytest.mark.machine


def test_tick_then_screen_is_160x144(running_emu):
    running_emu.tick(60)
    img = running_emu.get_screen()
    assert img.size == (160, 144)
    assert img.mode == "RGB"
    assert set(img.convert("L").getdata()) <= {0x00, 0x55, 0xAA, 0xFF}


def test_press_advances_frame_count(running_emu):
    before = running_emu.frame_count
    running_emu.press("start", 8)
    assert running_emu.frame_count == before + 8
    running_emu.release_all()


def test_read_range_length_and_consistency(running_emu):
    running_emu.tick(5)
    data = running_emu.read_range(0xC000, 16)
    assert len(data) == 16
    assert data[0] == running_emu.read_u8(0xC000)
    assert running_emu.read_u16(0xC000) == data[0] | (data[1] << 8)
    assert running_emu.read_u32(0xC000) == int.from_bytes(data[:4], "little")
    # reads are side-effect free and wrap at 16 bits
    assert running_emu.read_range(0xC000, 16) == data
    assert len(running_emu.read_range(0xFFFF, 2)) == 2
    assert running_emu.read_u8(0x1C000) == running_emu.read_u8(0xC000)


def test_save_load_roundtrip(running_emu, tmp_path):
    running_emu.tick(10)
    path = tmp_path / "s.state"
    running_emu.save_state(str(path))
    wram = running_emu.read_range(0xC000, 0x2000)
    running_emu.tick(30)
    after_a = (running_emu.read_range(0xC000, 0x2000), running_emu.get_screen().tobytes())
    running_emu.load_state(str(path))
    assert running_emu.read_range(0xC000, 0x2000) == wram
    running_emu.tick(30)
    after_b = (running_emu.read_range(0xC000, 0x2000), running_emu.get_screen().tobytes())
    assert after_a == after_b  # deterministic replay from the state


def test_load_state_rejects_garbage(running_emu, tmp_path):
    from gbemu.native import GbEmuError

    p = tmp_path / "garbage.state"
    p.write_bytes(b"not a state")
    with pytest.raises(GbEmuError):
        running_emu.load_state(str(p))
    running_emu.tick(1)  # instance still usable


def test_two_threads_screen_and_tick(running_emu):
    errors = []

    def screens():
        try:
            for _ in range(100):
                assert running_emu.get_screen().size == (160, 144)
        except Exception as exc:  # noqa: BLE001
            errors.append(exc)

    def ticks():
        try:
            for _ in range(100):
                running_emu.tick(1)
        except Exception as exc:  # noqa: BLE001
            errors.append(exc)

    start = running_emu.frame_count
    ts = [threading.Thread(target=screens), threading.Thread(target=ticks)]
    for t in ts:
        t.start()
    for t in ts:
        t.join(timeout=120)
    assert not errors, errors
    assert running_emu.frame_count == start + 100


def test_pokered_boots(lib_env):
    """Optional: the real game ROM (GBEMU_POKERED) boots and draws something."""
    rom = os.environ.get("GBEMU_POKERED")
    if not rom or not Path(rom).is_file():
        pytest.skip("GBEMU_POKERED not set")
    from gbemu.backend import GbEmuEmulator
    from gbemu.native import GbEmuError

    e = GbEmuEmulator()
    try:
        e.load(rom)
        try:
            e.tick(600)
        except GbEmuError as exc:
            if os.environ.get("GBEMU_REQUIRE_MACHINE") == "1":
                raise
            pytest.skip(f"whole machine not available yet: {exc}")
        img = e.get_screen()
        assert img.size == (160, 144)
        lo, hi = img.convert("L").getextrema()
        assert lo != hi, "screen is a single colour after 10 s of pokered"
    finally:
        e.close()
