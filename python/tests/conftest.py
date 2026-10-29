"""Shared fixtures for the gbemu tests.

Run from the worktree with the worktree's package first on the path::

    PYTHONPATH=$W/python GBEMU_LIB=$W/target/release/libgb_ffi.dylib \
        .venv/bin/python -m pytest $W/python/tests [-k "not machine"]

Tests marked ``machine`` need a working whole machine (CPU, bus, PPU...).  They
skip while ``Machine::run_frame`` still panics, unless ``GBEMU_REQUIRE_MACHINE=1``
(set it for T53b so a broken machine fails instead of skipping).
"""

from __future__ import annotations

import os
from pathlib import Path

import pytest

import gbemu


def pytest_configure(config):
    config.addinivalue_line(
        "markers",
        "machine: needs a whole working Machine (skipped until it stops panicking, "
        "unless GBEMU_REQUIRE_MACHINE=1)",
    )


def pytest_report_header(config):
    return [
        f"gbemu package: {Path(gbemu.__file__).parent}",
        f"GBEMU_LIB: {os.environ.get('GBEMU_LIB', '<unset>')}",
    ]


def make_rom(cart_type: int = 0x00, size: int = 0x8000, code: bytes | None = None) -> bytes:
    """A synthetic DMG ROM: entry point jumps to 0x150, which by default runs
    ``DI; LD HL,0xC000; loop: INC (HL); JR loop`` (WRAM 0xC000 keeps changing)."""
    rom = bytearray(size)
    rom[0x100:0x104] = bytes([0x00, 0xC3, 0x50, 0x01])  # NOP; JP 0x0150
    rom[0x134:0x13F] = b"GBEMU-TEST"
    rom[0x147] = cart_type
    rom[0x148] = 0x00  # 32 KiB
    rom[0x149] = 0x00  # no RAM
    if code is None:
        code = bytes([0xF3, 0x21, 0x00, 0xC0, 0x34, 0x18, 0xFD])
    rom[0x150 : 0x150 + len(code)] = code
    chk = 0
    for b in rom[0x134:0x14D]:
        chk = (chk - b - 1) & 0xFF
    rom[0x14D] = chk
    total = (sum(rom) - rom[0x14E] - rom[0x14F]) & 0xFFFF
    rom[0x14E], rom[0x14F] = total >> 8, total & 0xFF
    return bytes(rom)


@pytest.fixture
def lib_env():
    if not os.environ.get("GBEMU_LIB"):
        pytest.fail("GBEMU_LIB must point at target/release/libgb_ffi.dylib for these tests")
    return os.environ["GBEMU_LIB"]


@pytest.fixture
def rom_file(tmp_path) -> Path:
    p = tmp_path / "synthetic.gb"
    p.write_bytes(make_rom())
    return p


@pytest.fixture
def emu(lib_env, rom_file):
    """A loaded emulator (loading works even while the machine cannot run)."""
    from gbemu.backend import GbEmuEmulator

    e = GbEmuEmulator()
    e.load(str(rom_file))
    yield e
    e.close()


@pytest.fixture
def running_emu(emu):
    """A loaded emulator whose machine can actually run frames; else skip."""
    from gbemu.native import GbEmuError

    try:
        emu.tick(1)
        emu.get_screen()
    except GbEmuError as exc:
        if os.environ.get("GBEMU_REQUIRE_MACHINE") == "1":
            raise
        pytest.skip(f"whole machine not available yet: {exc}")
    # Start every machine test from a fresh instance.
    emu.load(emu.rom_path)
    return emu


def skip_if_unimplemented(exc: Exception) -> None:
    """Turn a core ``todo!()`` panic into a skip (unless GBEMU_REQUIRE_MACHINE=1)."""
    if "not yet implemented" in str(exc) and os.environ.get("GBEMU_REQUIRE_MACHINE") != "1":
        pytest.skip(f"core part not implemented yet: {exc}")
