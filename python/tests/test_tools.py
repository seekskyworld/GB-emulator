"""Offline checks of tools/playtest.py, tools/scripted_driver.py and tools/drive_loop.sh.

The servers here are the real pokemon-agent server on the fake backend
(fake_server.py); `claude` is replaced by fake_claude.py.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from helpers import REPO, Server, env_with_paths, free_port, post, wait_health

TOOLS = REPO / "tools"
TESTS = Path(__file__).resolve().parent


def load_tool(name: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


playtest = load_tool("playtest")
scripted = load_tool("scripted_driver")


# ---------------------------------------------------------------------------
# playtest.py pure functions
# ---------------------------------------------------------------------------


def good_state(map_id=38, x=3, y=6, dialog=False):
    return {"metadata": {}, "player": {"name": "RED", "position": {"x": x, "y": y}},
            "map": {"map_id": map_id, "map_name": "Red's House 2F"}, "dialog": {"active": dialog},
            "battle": {"in_battle": False}, "party": [], "bag": [], "flags": {}}


def test_validate_state():
    problems, summ = playtest.validate_state(good_state())
    assert problems == [] and summ["map_id"] == 38 and (summ["x"], summ["y"]) == (3, 6)
    bad = good_state()
    bad["player"] = None
    bad["player_error"] = "GbEmuError: boom\ntrace"
    problems, _ = playtest.validate_state(bad)
    assert any("player_error: GbEmuError: boom" in p for p in problems)
    assert any("position" in p for p in problems)
    problems, _ = playtest.validate_state({"map": {"map_id": 300}})
    assert len(problems) >= 4
    assert playtest.validate_state([1, 2])[0] == ["state is not a JSON object"]


def sample(t, map_id=38, x=3, y=6, mono=False, actions=None, **kw):
    s = {"t": t, "server_alive": True, "health_ok": True, "state_ok": True, "screenshot_ok": True,
         "map_id": map_id, "x": x, "y": y, "mono": mono, "actions_total": actions}
    s.update(kw)
    return s


STATS_OK = {"actions_ok": 10, "actions_failed": 0, "actions_without_frame_advance": 0, "games_new_calls": 1}


def hard(result, name_part):
    return next(h for h in result["hard"] if name_part in h["name"])


def test_evaluate_pass():
    ss = [sample(i * 10, x=3 + i, actions=i) for i in range(12)]
    r = playtest.evaluate(ss, STATS_OK, "script")
    assert r["verdict"] == "PASS" and r["exit_code"] == 0, r
    assert r["warnings"] == []


def test_evaluate_requires_a_position_change():
    ss = [sample(i * 10, actions=i) for i in range(12)]
    r = playtest.evaluate(ss, STATS_OK, "script")
    assert r["verdict"] == "FAIL"
    assert not hard(r, "map or coordinates")["ok"]


def test_evaluate_mono_limit():
    ss = [sample(i * 10, x=i, mono=(1 <= i <= 7), actions=i) for i in range(12)]  # mono t=10..70 → 60 s
    assert playtest.evaluate(ss, STATS_OK, "script")["verdict"] == "PASS"
    ss = [sample(i * 10, x=i, mono=(1 <= i <= 8), actions=i) for i in range(12)]  # t=10..80 → 70 s
    r = playtest.evaluate(ss, STATS_OK, "script")
    assert r["verdict"] == "FAIL" and not hard(r, "single-colour")["ok"]
    assert any("single-colour screenshot" in w for w in r["warnings"])


def test_evaluate_frame_advance_and_games_new():
    ss = [sample(i * 10, x=i, actions=i) for i in range(5)]
    r = playtest.evaluate(ss, dict(STATS_OK, actions_without_frame_advance=1), "script")
    assert not hard(r, "frames advance")["ok"]
    r = playtest.evaluate(ss, dict(STATS_OK, actions_ok=0), "script")
    assert not hard(r, "frames advance")["ok"]
    r = playtest.evaluate(ss, dict(STATS_OK, games_new_calls=2), "script")
    assert not hard(r, "/games/new")["ok"]
    r = playtest.evaluate(ss, None, "script")
    assert r["verdict"] == "FAIL"


def test_evaluate_server_and_endpoint_failures():
    base = [sample(i * 10, x=i, actions=i) for i in range(5)]
    for key, part in [("server_alive", "alive"), ("health_ok", "/health"), ("state_ok", "/state"),
                      ("screenshot_ok", "/screenshot")]:
        ss = [dict(s) for s in base]
        ss[3][key] = False
        r = playtest.evaluate(ss, STATS_OK, "script")
        assert not hard(r, part)["ok"], key
    r = playtest.evaluate(base, STATS_OK, "script", server_exit=1)
    assert not hard(r, "alive")["ok"]
    assert playtest.evaluate([], STATS_OK, "script")["verdict"] == "FAIL"


def test_evaluate_warnings():
    # coordinates frozen for 11 minutes, actions frozen for 3 minutes
    ss = [sample(i * 10, x=(1 if i > 66 else 0), actions=(0 if i < 18 else i)) for i in range(70)]
    r = playtest.evaluate(ss, STATS_OK, "agent", rounds=[
        {"kind": "agent", "round": 1, "ok": False, "rc": 1},
        {"kind": "agent", "round": 2, "ok": True, "rc": 0},
    ])
    assert r["verdict"] == "PASS"
    assert any("coordinates unchanged for 660s" in w for w in r["warnings"]), r["warnings"]
    assert any("no /action for 170s" in w for w in r["warnings"]), r["warnings"]
    assert any("1 failed driver round" in w for w in r["warnings"])


def test_evaluate_invalid_when_scripted_fallback_too_long():
    ss = [sample(i * 10, x=i, actions=i) for i in range(5)]
    rounds = [{"kind": "scripted", "seconds": 60}] * 4
    r = playtest.evaluate(ss, STATS_OK, "agent", rounds=rounds)
    assert r["verdict"] == "INVALID" and r["exit_code"] == 3
    assert playtest.evaluate(ss, STATS_OK, "agent", rounds=rounds[:3])["verdict"] == "PASS"


def test_contact_sheet_picks_one_per_minute():
    ss = [sample(i * 10, shot=f"shots/s{i:04d}.png") for i in range(19)]  # 0..180 s
    picks = playtest.contact_sheet_picks(ss)
    assert [p["t"] for p in picks] == [0, 60, 120, 180]
    assert playtest.contact_sheet_picks([sample(0)]) == []


def test_contact_sheet_and_summary_render(tmp_path):
    from PIL import Image

    (tmp_path / "shots").mkdir()
    ss = []
    for i in range(8):
        p = tmp_path / "shots" / f"s{i:04d}.png"
        Image.new("RGB", (160, 144), (i * 30, 0, 0)).save(p)
        ss.append(sample(i * 30, x=i, actions=i, shot=f"shots/s{i:04d}.png"))
    out = playtest.make_contact_sheet(tmp_path, ss)
    assert out and Image.open(out).size == (6 * 160, 144 + 14)  # picks t=0,60,120,180,210
    r = playtest.evaluate(ss, STATS_OK, "script")
    md = playtest.render_summary({"run_id": "x", "driver": "script"}, r, ss, [])
    assert "**Verdict: PASS**" in md and "## Coordinate trajectory" in md and "| t | map |" in md


def test_mono_detection():
    import io

    from PIL import Image

    def png(img):
        b = io.BytesIO()
        img.save(b, format="PNG")
        return b.getvalue()

    white = Image.new("RGB", (160, 144), (255, 255, 255))
    assert playtest.is_mono(png(white)) is True
    white.putpixel((5, 5), (0, 0, 0))
    assert playtest.is_mono(png(white)) is False
    assert playtest.is_mono(b"not a png") is None


# ---------------------------------------------------------------------------
# scripted_driver.py pure functions
# ---------------------------------------------------------------------------


def test_scripted_names_detection():
    st = good_state()
    st["player"]["rival_name"] = "BLUE"
    assert scripted.names_set(st)
    st["player"]["name"] = "???????????"
    assert not scripted.names_set(st)
    st["player"]["name"], st["player"]["rival_name"] = "NINTEN", "SONY"  # defaults before naming
    assert not scripted.names_set(st)
    st["player"]["name"] = "RED"
    assert not scripted.names_set(st)
    assert not scripted.names_set(None)
    assert scripted.position(good_state(x=4, y=5)) == (38, 4, 5)


# ---------------------------------------------------------------------------
# CLI smoke
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("tool", ["playtest.py", "scripted_driver.py"])
def test_help(tool):
    r = subprocess.run([sys.executable, str(TOOLS / tool), "--help"], capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, r.stderr
    assert "--seconds" in r.stdout or "--minutes" in r.stdout


def test_drive_loop_syntax_and_prompt():
    assert subprocess.run(["bash", "-n", str(TOOLS / "drive_loop.sh")]).returncode == 0
    prompt = (TOOLS / "driver_prompt.md").read_text()
    assert ("服务器已在 `localhost:{{PORT}}` 运行；禁止启动或停止服务器；禁止调用 `/games/new`、"
            "`/games/{id}/load`、`/load`；只用 `/objectives`、`/state`、`/screenshot*`、`/map/ascii`、"
            "`/action`。") in prompt
    assert "{{SKILL}}" in prompt and "E5" in prompt


def test_playtest_setup_error_without_lib(tmp_path):
    env = env_with_paths()
    env.pop("GBEMU_LIB", None)
    rom = tmp_path / "x.gb"
    rom.write_bytes(b"\0" * 0x8000)
    r = subprocess.run([sys.executable, str(TOOLS / "playtest.py"), "--rom", str(rom), "--port", str(free_port()),
                        "--out-root", str(tmp_path / "out"), "--run-id", "r", "--minutes", "0.1"],
                       env=env, capture_output=True, text=True, timeout=60)
    assert r.returncode == 2 and "GBEMU_LIB is not set" in r.stderr
    assert "SETUP ERROR" in (tmp_path / "out" / "r" / "summary.md").read_text()


# ---------------------------------------------------------------------------
# End to end against the fake backend
# ---------------------------------------------------------------------------


def run_playtest(tmp_path, driver, minutes, extra_env=None, run_id="run"):
    rom = tmp_path / "fake.gb"
    rom.write_bytes(b"\0" * 0x8000)
    env = env_with_paths(extra_env or {})
    env.setdefault("GBEMU_LIB", str(REPO / "target" / "release" / "libgb_ffi.dylib"))
    if not Path(env["GBEMU_LIB"]).is_file():
        env["GBEMU_LIB"] = str(rom)  # only copied/pinned; the fake backend never loads it
    cmd = [sys.executable, str(TOOLS / "playtest.py"), "--minutes", str(minutes), "--driver", driver,
           "--port", str(free_port()), "--rom", str(rom), "--launcher", "fake_server", "--interval", "2",
           "--out-root", str(tmp_path / "out"), "--run-id", run_id]
    r = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=300)
    return r, tmp_path / "out" / run_id


def test_playtest_script_driver_end_to_end(tmp_path):
    r, run = run_playtest(tmp_path, "script", 0.4)
    assert r.returncode == 0, r.stdout + r.stderr + (run / "summary.md").read_text()
    for f in ("summary.md", "summary.json", "samples.jsonl", "contact-sheet.png", "server.log", "driver.log",
              "driver-actions.jsonl"):
        assert (run / f).is_file(), f
    assert (run / "data" / "gbemu-lib").is_dir()
    summary = json.loads((run / "summary.json").read_text())
    assert summary["result"]["verdict"] == "PASS"
    assert summary["final_stats"]["games_new_calls"] == 1
    log = (run / "driver.log").read_text()
    assert "opening: names set (RED/BLUE)" in log, log
    assert "roam:" in log


def test_playtest_detects_frozen_frames(tmp_path):
    r, run = run_playtest(tmp_path, "script", 0.15, {"FAKE_FROZEN": "1"})
    assert r.returncode == 1, r.stdout
    assert "ERROR frames advance after every /action" in r.stdout


def test_playtest_agent_driver_with_fake_claude(tmp_path):
    r, run = run_playtest(tmp_path, "agent", 0.3, {
        "CLAUDE_BIN": str(TESTS / "fake_claude.py"), "FAKE_CLAUDE_MODE": "ok", "FAKE_NAMES": "1",
        "FAKE_CLAUDE_LOG": str(tmp_path / "claude.log")})
    assert r.returncode == 0, r.stdout + (run / "driver.log").read_text()
    rows = (run / "driver-sessions.txt").read_text().splitlines()
    assert rows[0].startswith("round\tsession_id")
    assert len(rows) - 1 >= 3, rows
    sid = rows[1].split("\t")[1]
    assert len(sid) == 36 and sid.count("-") == 4
    rounds = [json.loads(line) for line in (run / "driver-rounds.jsonl").read_text().splitlines()]
    assert all(r_["kind"] == "agent" and r_["ok"] and r_["actions"] == 2 for r_ in rounds[:3]), rounds
    summary = json.loads((run / "summary.json").read_text())
    assert summary["result"]["agent_rounds"] >= 3


def test_drive_loop_fallback_after_three_failures(tmp_path):
    port = free_port()
    url = f"http://localhost:{port}"
    rom = tmp_path / "fake.gb"
    rom.write_bytes(b"\0" * 0x8000)
    args = [str(TESTS / "fake_server.py"), "serve", "--rom", str(rom), "--port", str(port),
            "--data-dir", str(tmp_path / "data")]
    run = tmp_path / "run"
    with Server(args, env_with_paths({"FAKE_NAMES": "1"}), tmp_path / "server.log") as srv:
        wait_health(url, srv.proc)
        env = env_with_paths({"PORT": str(port), "RUN_DIR": str(run), "SECONDS_TOTAL": "6",
                              "FALLBACK_SECONDS": "2", "PYTHON": sys.executable,
                              "CLAUDE_BIN": str(TESTS / "fake_claude.py"), "FAKE_CLAUDE_MODE": "fail"})
        r = subprocess.run(["bash", str(TOOLS / "drive_loop.sh")], env=env, capture_output=True, text=True,
                           timeout=120)
    assert r.returncode == 0, r.stdout + r.stderr
    rounds = [json.loads(line) for line in (run / "driver-rounds.jsonl").read_text().splitlines()]
    kinds = [x["kind"] for x in rounds]
    assert kinds[:4] == ["agent", "agent", "agent", "scripted"], rounds
    assert all(not x["ok"] and x["status"] == "exit-1" for x in rounds[:3])
    assert rounds[3]["seconds"] >= 2


def test_drive_loop_round_timeout(tmp_path):
    port = free_port()
    run = tmp_path / "run"
    env = env_with_paths({"PORT": str(port), "RUN_DIR": str(run), "SECONDS_TOTAL": "3", "ROUND_TIMEOUT": "2",
                          "CLAUDE_BIN": str(TESTS / "fake_claude.py"), "FAKE_CLAUDE_MODE": "hang"})
    r = subprocess.run(["bash", str(TOOLS / "drive_loop.sh")], env=env, capture_output=True, text=True, timeout=60)
    assert r.returncode == 0, r.stdout + r.stderr
    first = json.loads((run / "driver-rounds.jsonl").read_text().splitlines()[0])
    assert first["timeout"] is True and first["status"] == "timeout" and first["ok"] is False


def test_drive_loop_records_interrupted_round(tmp_path):
    import signal
    import time

    run = tmp_path / "run"
    env = env_with_paths({"PORT": str(free_port()), "RUN_DIR": str(run), "SECONDS_TOTAL": "60",
                          "CLAUDE_BIN": str(TESTS / "fake_claude.py"), "FAKE_CLAUDE_MODE": "hang"})
    p = subprocess.Popen(["bash", str(TOOLS / "drive_loop.sh")], env=env, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, start_new_session=True)
    time.sleep(3)
    os.killpg(p.pid, signal.SIGINT)
    out = p.communicate(timeout=30)[0].decode()
    rows = (run / "driver-sessions.txt").read_text().splitlines()
    assert len(rows) == 2 and rows[1].endswith("\tinterrupted"), out
    assert len(rows[1].split("\t")[1]) == 36
    rec = json.loads((run / "driver-rounds.jsonl").read_text().splitlines()[0])
    assert rec["status"] == "interrupted" and rec["ok"] is True
