"""Error paths of the ctypes layer and the backend that need no running machine."""

from __future__ import annotations

import ctypes
import gc

import pytest

from conftest import make_rom
from gbemu import native
from gbemu.native import GbEmuError, GbEmuLibraryError


def test_gbemu_lib_unset_is_a_clear_error(monkeypatch):
    monkeypatch.delenv("GBEMU_LIB", raising=False)
    with pytest.raises(GbEmuLibraryError, match="GBEMU_LIB is not set"):
        native.load_library()
    from gbemu.backend import GbEmuEmulator

    with pytest.raises(GbEmuLibraryError, match="GBEMU_LIB is not set"):
        GbEmuEmulator()


def test_gbemu_lib_missing_file(monkeypatch, tmp_path):
    monkeypatch.setenv("GBEMU_LIB", str(tmp_path / "nope.dylib"))
    with pytest.raises(GbEmuLibraryError, match="does not exist"):
        native.load_library()


def test_all_signatures_are_typed(lib_env):
    lib = native.load_library()
    for name, (restype, argtypes) in native._SIGNATURES.items():
        fn = getattr(lib, name)
        assert fn.restype == restype, name
        assert fn.argtypes == argtypes, name
    assert ctypes.sizeof(native.GbRegs) == 14


@pytest.mark.parametrize(
    "rom, pattern",
    [
        (b"", "bad ROM size"),
        (b"\x00" * 16, "bad ROM size"),
        (make_rom(cart_type=0xFF), "unsupported cartridge type 0xFF"),
    ],
)
def test_gb_new_rejects_bad_rom(lib_env, rom, pattern):
    lib = native.load_library()
    buf = (ctypes.c_uint8 * max(1, len(rom))).from_buffer_copy(rom or b"\x00")
    assert not lib.gb_new(buf, len(rom))  # null handle
    assert pattern in native.last_error(lib)
    with pytest.raises(GbEmuError, match=pattern):
        native.new(lib, rom)


def test_null_handle_calls_fail_cleanly(lib_env):
    lib = native.load_library()
    with pytest.raises(GbEmuError, match="null handle") as ei:
        native.run_frames(lib, None, 1)
    assert ei.value.code == native.GB_ERR_ARG
    with pytest.raises(GbEmuError, match="null handle"):
        native.peek(lib, None, 0xC000)
    with pytest.raises(GbEmuError, match="null handle"):
        native.save_state(lib, None)
    lib.gb_free(None)  # no-op


def test_panic_is_converted_to_error(lib_env):
    lib = native.load_library()
    with pytest.raises(GbEmuError, match="selftest panic") as ei:
        native.selftest_panic(lib)
    assert ei.value.code == native.GB_ERR_PANIC


def test_backend_bad_rom_raises(lib_env, tmp_path):
    from gbemu.backend import GbEmuEmulator

    bad = tmp_path / "bad.gb"
    bad.write_bytes(b"\x00" * 100)
    e = GbEmuEmulator()
    with pytest.raises(GbEmuError, match="bad ROM size"):
        e.load(str(bad))
    with pytest.raises(FileNotFoundError):
        e.load(str(tmp_path / "missing.gb"))
    e.close()


def test_unknown_button_raises_value_error(emu):
    for bad in ("x", "", "turbo", None, 3):
        with pytest.raises(ValueError):
            emu.press(bad, 8)
    assert emu.frame_count == 0  # nothing ran


def test_frames_argument_is_validated(emu):
    with pytest.raises(ValueError):
        emu.tick(-1)
    with pytest.raises(ValueError):
        emu.press("a", -1)
    with pytest.raises(ValueError):
        emu.tick(1.5)
    emu.tick(0)  # no-op, no native call
    assert emu.frame_count == 0


def test_read_range_empty_needs_no_native_call(emu):
    assert emu.read_range(0xC000, 0) == b""


def test_close_is_idempotent(emu):
    emu.close()
    emu.close()
    with pytest.raises(GbEmuError, match="not loaded"):
        emu.tick(1)
    with pytest.raises(GbEmuError, match="not loaded"):
        emu.read_u8(0xC000)
    emu.__del__()  # still harmless after close


def test_close_before_load_and_del_without_close(lib_env, rom_file):
    from gbemu.backend import GbEmuEmulator

    e = GbEmuEmulator()
    e.close()
    e.close()
    e2 = GbEmuEmulator()
    e2.load(str(rom_file))
    e2.load(str(rom_file))  # reload frees the previous handle
    del e2  # __del__ frees the native handle (the /games/new path)
    gc.collect()


def test_backend_reports_info(emu):
    info = emu.get_info()
    assert info["backend"] == "GbEmuEmulator"
    assert info["platform"] == "GB"
    assert info["frame_count"] == 0
    assert info["rom_path"].endswith("synthetic.gb")
