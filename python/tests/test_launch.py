"""T54: gbemu.launch (way 1) — offline parts, plus the machine end-to-end check."""

from __future__ import annotations

import io
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from helpers import PYDIR, REPO, Server, env_with_paths, free_port, get, post, wait_health


def test_install_routes_gb_to_backend_and_blocks_pyboy(lib_env, tmp_path):
    from gbemu import launch
    from gbemu.backend import GbEmuEmulator
    from gbemu.native import GbEmuError

    launch.install()
    import pokemon_agent.emulator as pe

    assert pe._EXT_MAP[".gb"] is GbEmuEmulator
    assert pe._EXT_MAP[".gbc"] is GbEmuEmulator
    bad = tmp_path / "bad.gb"
    bad.write_bytes(b"\x00" * 64)
    with pytest.raises(GbEmuError, match="bad ROM size"):
        pe.create_emulator(str(bad))
    with pytest.raises(ImportError, match="forbidden"):
        import pyboy  # noqa: F401
    assert "pyboy" not in sys.modules


def test_launch_help_runs():
    r = subprocess.run([sys.executable, "-m", "gbemu.launch", "serve", "--help"], env=env_with_paths(),
                       capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, r.stderr
    assert "--data-dir" in r.stdout and "--rom" in r.stdout


def test_launch_without_gbemu_lib_fails_clearly(tmp_path):
    env = env_with_paths()
    env.pop("GBEMU_LIB", None)
    rom = tmp_path / "x.gb"
    rom.write_bytes(b"\x00" * 0x8000)
    r = subprocess.run([sys.executable, "-m", "gbemu.launch", "serve", "--rom", str(rom), "--port", "1",
                        "--data-dir", str(tmp_path / "d")], env=env, capture_output=True, text=True, timeout=60)
    assert r.returncode != 0
    assert "GBEMU_LIB is not set" in r.stderr


def test_fake_backend_through_launcher(tmp_path):
    """Real pokemon-agent server + launcher instrumentation, fake emulator."""
    port = free_port()
    url = f"http://127.0.0.1:{port}"
    rom = tmp_path / "fake.gb"
    rom.write_bytes(b"\x00" * 0x8000)
    args = [str(Path(__file__).with_name("fake_server.py")), "serve", "--rom", str(rom), "--port", str(port),
            "--data-dir", str(tmp_path / "data")]
    with Server(args, env_with_paths(), tmp_path / "server.log") as srv:
        wait_health(url, srv.proc)
        status, ctype, body = get(url + "/state")
        state = json.loads(body)
        assert status == 200 and state["map"]["map_id"] == 38
        status, ctype, png = get(url + "/screenshot")
        from PIL import Image

        assert ctype == "image/png" and Image.open(io.BytesIO(png)).size == (160, 144)
        status, js = post(url + "/games/new", {"name": "t"})
        assert status == 200
        status, js = post(url + "/action", {"actions": ["walk_right", "press_a"]})
        assert status == 200 and js["actions_executed"] == 2
        status, js = post(url + "/action", {"actions": ["press_turbo"]})
        assert status == 400
        _, _, body = get(url + "/gbemu/stats")
        stats = json.loads(body)
    assert stats["backend"] == "FakeEmulator"
    assert stats["pyboy_loaded"] is False
    assert stats["games_new_calls"] == 1
    assert stats["actions_total"] == 2 and stats["actions_ok"] == 1 and stats["actions_failed"] == 1
    assert stats["actions_without_frame_advance"] == 0
    assert stats["recent_actions"][0]["delta"] == 40  # walk = 8+12, press = 8+12


def test_frozen_backend_is_detected(tmp_path):
    port = free_port()
    url = f"http://127.0.0.1:{port}"
    rom = tmp_path / "fake.gb"
    rom.write_bytes(b"\x00" * 0x8000)
    args = [str(Path(__file__).with_name("fake_server.py")), "serve", "--rom", str(rom), "--port", str(port),
            "--data-dir", str(tmp_path / "data")]
    with Server(args, env_with_paths({"FAKE_FROZEN": "1"}), tmp_path / "server.log") as srv:
        wait_health(url, srv.proc)
        post(url + "/action", {"actions": ["wait_10"]})
        stats = json.loads(get(url + "/gbemu/stats")[2])
    assert stats["actions_without_frame_advance"] == 1


@pytest.mark.machine
def test_pokered_server_end_to_end(lib_env, tmp_path):
    """T54 completion: real launcher + real backend + pokered."""
    rom = os.environ.get("GBEMU_POKERED")
    if not rom or not Path(rom).is_file():
        pytest.skip("GBEMU_POKERED not set")
    port = free_port()
    url = f"http://127.0.0.1:{port}"
    args = ["-m", "gbemu.launch", "serve", "--rom", rom, "--port", str(port), "--data-dir", str(tmp_path / "data")]
    with Server(args, env_with_paths(), tmp_path / "server.log") as srv:
        wait_health(url, srv.proc, timeout=120)
        status, ctype, body = get(url + "/state", timeout=60)
        assert status == 200 and ctype.startswith("application/json")
        json.loads(body)
        status, ctype, png = get(url + "/screenshot", timeout=60)
        from PIL import Image

        assert status == 200 and ctype == "image/png" and Image.open(io.BytesIO(png)).size == (160, 144)
        status, js = post(url + "/action", {"actions": ["press_start", "wait_30"]}, timeout=120)
        assert status == 200, js
        stats = json.loads(get(url + "/gbemu/stats")[2])
    assert stats["backend"] == "GbEmuEmulator"
    assert stats["pyboy_loaded"] is False
    assert stats["actions_without_frame_advance"] == 0
