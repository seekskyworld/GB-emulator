#!/usr/bin/env python3
"""tools/playtest.py — G5 实玩监控（PLAN §4.4 第 1–6 条）。

    GBEMU_LIB=/abs/target/release/libgb_ffi.dylib \\
    .venv/bin/python tools/playtest.py --minutes 30 --driver agent --port 8765

1. 新建 ``reports/playtest/<时间戳>/``，把 ``GBEMU_LIB`` 复制（固定）到
   ``<run>/data/gbemu-lib/``，用 ``--data-dir <run>/data`` 启动
   ``python -m gbemu.launch serve``；
2. 只由本脚本调用一次 ``POST /games/new``（之后由 ``/gbemu/stats`` 核对次数）；
3. 每 10 秒采样 ``/health``、``/state``、``/screenshot``（加 ``/gbemu/stats``）；
4. 硬性判定（失败即退出码 1）与告警（只记录）；
5. 结束后写 ``summary.md``、``summary.json``、``samples.jsonl``、``contact-sheet.png``。

退出码：0 通过；1 硬性判定失败；2 启动失败；3 本次实玩无效（scripted 兜底累计超过 3 分钟）。

``pokemon-agent`` 的服务端是被动推进的：驾驶思考期间帧数不动属于正常，所以
"每次 /action 之后帧数都增加" 由 ``gbemu.launch`` 注入的 ``/gbemu/stats`` 逐次核对，
而不是看帧数是否持续增长。
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import io
import json
import math
import os
import shutil
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

REPO = Path(__file__).resolve().parent.parent
TOOLS = REPO / "tools"

SAMPLE_INTERVAL = 10.0
MONO_LIMIT_S = 60.0           # 单色画面持续超过 60 秒 → 失败
COORD_STALL_WARN_S = 600.0    # 10 分钟坐标无变化 → 告警
ACTION_GAP_WARN_S = 120.0     # 连续 2 分钟没有 /action → 告警
SCRIPTED_LIMIT_S = 180.0      # scripted 兜底累计超过 3 分钟 → 无效

EXIT_PASS, EXIT_FAIL, EXIT_SETUP, EXIT_INVALID = 0, 1, 2, 3


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------


def http(method: str, url: str, body: Optional[dict] = None, timeout: float = 10.0
         ) -> Tuple[Optional[int], bytes, str]:
    """Return (status, body, error). status is None when the request itself failed."""
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read(), ""
    except urllib.error.HTTPError as e:
        try:
            payload = e.read()
        except Exception:  # noqa: BLE001
            payload = b""
        return e.code, payload, f"HTTP {e.code}"
    except Exception as e:  # noqa: BLE001 - connection refused, timeout, ...
        return None, b"", f"{type(e).__name__}: {e}"


def get_json(url: str, timeout: float = 10.0) -> Tuple[Optional[int], Any, str]:
    status, payload, err = http("GET", url, timeout=timeout)
    if status is None:
        return None, None, err
    try:
        return status, json.loads(payload.decode("utf-8")), err
    except Exception as e:  # noqa: BLE001
        return status, None, err or f"bad JSON: {e}"


# ---------------------------------------------------------------------------
# Pure checks (unit-tested in python/tests/test_playtest.py)
# ---------------------------------------------------------------------------


def _is_int(v, lo: int = 0, hi: int = 255) -> bool:
    return isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi


def validate_state(js: Any) -> Tuple[List[str], Dict[str, Any]]:
    """Check the fields PLAN §4.4 samples (map_id, x, y, dialog, battle, party).

    Returns (problems, summary). An empty problem list means the state is valid.
    """
    problems: List[str] = []
    summary: Dict[str, Any] = {}
    if not isinstance(js, dict):
        return ["state is not a JSON object"], summary
    for key in ("player", "map", "dialog", "battle", "party"):
        if js.get(f"{key}_error"):
            problems.append(f"{key}_error: {str(js[f'{key}_error']).splitlines()[0][:160]}")
    m = js.get("map")
    if isinstance(m, dict) and _is_int(m.get("map_id")):
        summary["map_id"] = m["map_id"]
        summary["map_name"] = str(m.get("map_name", ""))
    else:
        problems.append("map.map_id missing or not 0..255")
    p = js.get("player")
    pos = p.get("position") if isinstance(p, dict) else None
    if isinstance(pos, dict) and _is_int(pos.get("x")) and _is_int(pos.get("y")):
        summary["x"], summary["y"] = pos["x"], pos["y"]
        summary["player_name"] = str(p.get("name", ""))
    else:
        problems.append("player.position.x/y missing or not 0..255")
    d = js.get("dialog")
    if isinstance(d, dict) and isinstance(d.get("active"), bool):
        summary["dialog"] = d["active"]
    else:
        problems.append("dialog.active missing or not bool")
    b = js.get("battle")
    if isinstance(b, dict) and isinstance(b.get("in_battle"), bool):
        summary["battle"] = b["in_battle"]
    else:
        problems.append("battle.in_battle missing or not bool")
    party = js.get("party")
    if isinstance(party, list) and len(party) <= 6:
        summary["party"] = [
            f"{mon.get('species', '?')} L{mon.get('level', '?')}" for mon in party if isinstance(mon, dict)
        ]
    else:
        problems.append("party missing or longer than 6")
    if js.get("collision_error"):
        summary["collision_error"] = str(js["collision_error"])[:160]
    return problems, summary


def is_mono(png: bytes) -> Optional[bool]:
    """True if the PNG is a single colour; None if it cannot be decoded."""
    try:
        from PIL import Image

        img = Image.open(io.BytesIO(png))
        img.load()
    except Exception:  # noqa: BLE001
        return None
    extrema = img.convert("L").getextrema()
    return extrema[0] == extrema[1]


def png_size(png: bytes) -> Optional[Tuple[int, int]]:
    try:
        from PIL import Image

        return Image.open(io.BytesIO(png)).size
    except Exception:  # noqa: BLE001
        return None


def longest_run(samples: List[dict], pred) -> Tuple[float, Optional[float], Optional[float]]:
    """Longest span (seconds, start, end) over consecutive samples satisfying pred."""
    best: Tuple[float, Optional[float], Optional[float]] = (0.0, None, None)
    start = None
    for s in samples:
        if pred(s):
            if start is None:
                start = s["t"]
            span = s["t"] - start
            if best[1] is None or span > best[0]:
                best = (span, start, s["t"])
        else:
            start = None
    return best


def position_key(s: dict):
    if s.get("map_id") is None or s.get("x") is None:
        return None
    return (s["map_id"], s["x"], s["y"])


def coordinate_stalls(samples: List[dict], limit: float = COORD_STALL_WARN_S) -> List[Tuple[float, float]]:
    """Intervals (start, end) where (map, x, y) stayed unchanged for >= limit seconds."""
    out = []
    cur_key = None
    since = None
    last_t = None
    for s in samples:
        k = position_key(s)
        if k is None:
            continue
        if k != cur_key:
            if cur_key is not None and last_t - since >= limit:
                out.append((since, last_t))
            cur_key, since = k, s["t"]
        last_t = s["t"]
    if cur_key is not None and last_t is not None and last_t - since >= limit:
        out.append((since, last_t))
    return out


def action_gaps(samples: List[dict], limit: float = ACTION_GAP_WARN_S) -> List[Tuple[float, float]]:
    """Intervals where the server-side /action count did not move for >= limit seconds."""
    out = []
    since = None
    count = None
    last_t = None
    for s in samples:
        c = s.get("actions_total")
        if c is None:
            continue
        if c != count:
            if count is not None and last_t - since >= limit:
                out.append((since, last_t))
            count, since = c, s["t"]
        last_t = s["t"]
    if count is not None and last_t is not None and last_t - since >= limit:
        out.append((since, last_t))
    return out


def trajectory(samples: List[dict]) -> List[dict]:
    """Samples where (map, x, y) changed (first sample included)."""
    out = []
    prev = None
    for s in samples:
        k = position_key(s)
        if k is not None and k != prev:
            out.append({"t": s["t"], "map_id": k[0], "map_name": s.get("map_name", ""), "x": k[1], "y": k[2]})
            prev = k
    return out


def read_driver_rounds(path: Path) -> List[dict]:
    if not path.is_file():
        return []
    rounds = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if line:
            try:
                rounds.append(json.loads(line))
            except json.JSONDecodeError:
                rounds.append({"kind": "unparsable", "raw": line[:200]})
    return rounds


def evaluate(samples: List[dict], stats: Optional[dict], driver: str,
             rounds: Optional[List[dict]] = None, server_exit: Optional[int] = None) -> dict:
    """Apply the hard checks and warnings of PLAN §4.4 item 4/5."""
    rounds = rounds or []
    hard: List[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        hard.append({"name": name, "ok": bool(ok), "detail": detail})

    n = len(samples)
    check("samples taken", n > 0, f"{n} samples")
    dead = [s["t"] for s in samples if not s.get("server_alive", True)]
    check("server process alive throughout", not dead and server_exit is None,
          f"died at t={dead[0]:.0f}s" if dead else (f"exit code {server_exit}" if server_exit is not None else ""))
    bad_health = [s["t"] for s in samples if not s.get("health_ok")]
    check("/health always responds", not bad_health,
          f"{len(bad_health)} failures, first t={bad_health[0]:.0f}s" if bad_health else "")
    bad_state = [s for s in samples if not s.get("state_ok")]
    check("/state always 200 with valid fields", not bad_state,
          (f"{len(bad_state)} bad, first t={bad_state[0]['t']:.0f}s: "
           f"{bad_state[0].get('state_status')} {'; '.join(bad_state[0].get('state_problems', []))[:200]}")
          if bad_state else "")
    bad_shot = [s for s in samples if not s.get("screenshot_ok")]
    check("/screenshot always a 160x144 PNG", not bad_shot,
          f"{len(bad_shot)} bad, first t={bad_shot[0]['t']:.0f}s" if bad_shot else "")

    if stats is None:
        check("frames advance after every /action", False, "/gbemu/stats unavailable (not launched via gbemu.launch?)")
        check("/games/new called exactly once", False, "/gbemu/stats unavailable")
    else:
        ok_actions = stats.get("actions_ok", 0)
        stuck = stats.get("actions_without_frame_advance", 0)
        check("frames advance after every /action", ok_actions > 0 and stuck == 0,
              f"{ok_actions} ok /action, {stuck} without frame advance, {stats.get('actions_failed', 0)} failed")
        gn = stats.get("games_new_calls")
        check("/games/new called exactly once", gn == 1, f"{gn} calls")

    keys = {position_key(s) for s in samples if position_key(s) is not None}
    check("map or coordinates changed at least once", len(keys) >= 2, f"{len(keys)} distinct (map,x,y)")
    span, a, b = longest_run(samples, lambda s: s.get("mono") is True)
    check(f"no single-colour screen for more than {MONO_LIMIT_S:.0f}s", span <= MONO_LIMIT_S,
          f"longest {span:.0f}s" + (f" (t={a:.0f}..{b:.0f}s)" if a is not None and span > 0 else ""))

    warnings: List[str] = []
    for s0, s1 in coordinate_stalls(samples):
        warnings.append(f"coordinates unchanged for {s1 - s0:.0f}s (t={s0:.0f}..{s1:.0f}s)")
    for s0, s1 in action_gaps(samples):
        warnings.append(f"no /action for {s1 - s0:.0f}s (t={s0:.0f}..{s1:.0f}s)")
    mono_n = sum(1 for s in samples if s.get("mono") is True)
    if mono_n:
        warnings.append(f"{mono_n} single-colour screenshot(s)")
    failed = [r for r in rounds if r.get("kind") == "agent" and not r.get("ok", False)]
    if failed:
        warnings.append(f"{len(failed)} failed driver round(s): " +
                        ", ".join(f"#{r.get('round')} rc={r.get('rc')}" + (" timeout" if r.get("timeout") else "")
                                  for r in failed[:10]))
    scripted_s = sum(float(r.get("seconds", 0)) for r in rounds if r.get("kind") == "scripted")
    if scripted_s:
        warnings.append(f"scripted fallback drove {scripted_s:.0f}s")
    collision_errors = sum(1 for s in samples if s.get("collision_error"))
    if collision_errors:
        warnings.append(f"{collision_errors} sample(s) with collision_error in /state")

    invalid = driver == "agent" and scripted_s > SCRIPTED_LIMIT_S
    hard_ok = all(h["ok"] for h in hard)
    if invalid:
        verdict, code = "INVALID", EXIT_INVALID
    elif hard_ok:
        verdict, code = "PASS", EXIT_PASS
    else:
        verdict, code = "FAIL", EXIT_FAIL
    return {
        "verdict": verdict,
        "exit_code": code,
        "hard": hard,
        "warnings": warnings,
        "scripted_seconds": scripted_s,
        "agent_rounds": sum(1 for r in rounds if r.get("kind") == "agent"),
        "failed_rounds": len(failed),
        "invalid_reason": (f"scripted fallback {scripted_s:.0f}s > {SCRIPTED_LIMIT_S:.0f}s" if invalid else ""),
    }


def contact_sheet_picks(samples: List[dict]) -> List[dict]:
    """One sample with a screenshot per minute mark (closest to m*60 s), plus the last one."""
    shots = [s for s in samples if s.get("shot")]
    if not shots:
        return []
    end = shots[-1]["t"]
    picks = []
    for m in range(int(math.floor(end / 60.0)) + 1):
        target = m * 60.0
        best = min(shots, key=lambda s: abs(s["t"] - target))
        if not picks or best is not picks[-1]:
            picks.append(best)
    if shots[-1] is not picks[-1]:
        picks.append(shots[-1])
    return picks


def make_contact_sheet(run_dir: Path, samples: List[dict], cols: int = 6) -> Optional[Path]:
    from PIL import Image, ImageDraw

    picks = contact_sheet_picks(samples)
    if not picks:
        return None
    tile_w, tile_h, label_h = 160, 144, 14
    rows = (len(picks) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * tile_w, rows * (tile_h + label_h)), (40, 40, 40))
    draw = ImageDraw.Draw(sheet)
    for i, s in enumerate(picks):
        x0, y0 = (i % cols) * tile_w, (i // cols) * (tile_h + label_h)
        try:
            img = Image.open(run_dir / s["shot"]).convert("RGB")
            if img.size != (tile_w, tile_h):
                img = img.resize((tile_w, tile_h))
            sheet.paste(img, (x0, y0 + label_h))
        except Exception:  # noqa: BLE001
            draw.rectangle([x0, y0 + label_h, x0 + tile_w - 1, y0 + label_h + tile_h - 1], outline=(255, 0, 0))
        pos = f"m{s.get('map_id', '?')} ({s.get('x', '?')},{s.get('y', '?')})"
        draw.text((x0 + 2, y0 + 1), f"{s['t'] / 60:5.1f}m {pos}", fill=(255, 255, 255))
    out = run_dir / "contact-sheet.png"
    sheet.save(out)
    return out


def _fmt_t(t: float) -> str:
    return f"{int(t // 60):02d}:{int(t % 60):02d}"


def render_summary(meta: dict, result: dict, samples: List[dict], rounds: List[dict]) -> str:
    lines = [f"# Playtest {meta['run_id']}", ""]
    lines.append(f"**Verdict: {result['verdict']}** (exit code {result['exit_code']})"
                 + (f" — {result['invalid_reason']}" if result.get("invalid_reason") else ""))
    lines.append("")
    lines.append("## Run")
    for k in ("driver", "minutes", "port", "url", "rom", "rom_sha1", "gbemu_lib", "gbemu_lib_sha256",
              "git_head", "data_dir", "started", "ended", "server_exit"):
        if k in meta:
            lines.append(f"- {k}: `{meta[k]}`")
    lines.append("")
    lines.append("## Hard checks")
    lines.append("")
    lines.append("| check | result | detail |")
    lines.append("| --- | --- | --- |")
    for h in result["hard"]:
        lines.append(f"| {h['name']} | {'ok' if h['ok'] else '**FAIL**'} | {h['detail']} |")
    lines.append("")
    lines.append("## Warnings")
    lines.append("")
    lines.extend([f"- {w}" for w in result["warnings"]] or ["- none"])
    lines.append("")
    lines.append("## Timeline (one sample per minute)")
    lines.append("")
    lines.append("| t | map | x,y | dialog | battle | party | actions | frames | mono |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    picked_t = -1e9
    for s in samples:
        if s["t"] - picked_t < 60 and s is not samples[-1]:
            continue
        picked_t = s["t"]
        lines.append(
            f"| {_fmt_t(s['t'])} | {s.get('map_id', '?')} {s.get('map_name', '')} | "
            f"{s.get('x', '?')},{s.get('y', '?')} | {s.get('dialog', '?')} | {s.get('battle', '?')} | "
            f"{len(s.get('party') or [])} | {s.get('actions_total', '?')} | {s.get('frame_count', '?')} | "
            f"{s.get('mono', '?')} |"
        )
    lines.append("")
    lines.append("## Coordinate trajectory")
    lines.append("")
    traj = trajectory(samples)
    for p in traj[:200]:
        lines.append(f"- {_fmt_t(p['t'])} map {p['map_id']} {p['map_name']} ({p['x']},{p['y']})")
    if len(traj) > 200:
        lines.append(f"- … {len(traj) - 200} more in samples.jsonl")
    if not traj:
        lines.append("- none")
    lines.append("")
    lines.append("## Driver")
    lines.append("")
    lines.append(f"- agent rounds: {result['agent_rounds']}, failed: {result['failed_rounds']}, "
                 f"scripted fallback: {result['scripted_seconds']:.0f}s")
    if rounds:
        lines.append("- rounds: `driver-rounds.jsonl`; session ids: `driver-sessions.txt`")
    lines.append("")
    lines.append("## Files")
    lines.append("")
    lines.append("- `samples.jsonl` (every sample), `summary.json`, `server.log`, `driver.log`, "
                 "`shots/` (every screenshot), `contact-sheet.png` (one per minute)")
    lines.append("")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Process management
# ---------------------------------------------------------------------------


def sha(path: Path, algo: str) -> str:
    h = hashlib.new(algo)
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def git_head() -> str:
    try:
        head = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--short", "HEAD"],
                              capture_output=True, text=True, timeout=10).stdout.strip()
        dirty = subprocess.run(["git", "-C", str(REPO), "status", "--porcelain", "--", "crates", "python"],
                               capture_output=True, text=True, timeout=10).stdout.strip()
        return head + ("+dirty" if dirty else "")
    except Exception:  # noqa: BLE001
        return "unknown"


def stop_process(proc: Optional[subprocess.Popen], name: str, grace: float = 10.0) -> Optional[int]:
    if proc is None:
        return None
    if proc.poll() is None:
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGKILL):
            try:
                os.killpg(proc.pid, sig)
            except ProcessLookupError:
                break
            try:
                proc.wait(timeout=grace)
                break
            except subprocess.TimeoutExpired:
                print(f"[playtest] {name} did not stop on {sig.name}", file=sys.stderr)
    return proc.returncode


def wait_ready(url: str, proc: Optional[subprocess.Popen], timeout: float) -> Tuple[bool, str]:
    deadline = time.time() + timeout
    last = ""
    while time.time() < deadline:
        if proc is not None and proc.poll() is not None:
            return False, f"server exited with code {proc.returncode}"
        status, js, err = get_json(url + "/health", timeout=3)
        if status == 200 and isinstance(js, dict) and js.get("emulator_ready"):
            return True, ""
        last = err or f"health={js}"
        time.sleep(0.5)
    return False, f"not ready after {timeout:.0f}s ({last})"


def take_sample(url: str, t: float, idx: int, run_dir: Path, proc: Optional[subprocess.Popen]) -> dict:
    s: Dict[str, Any] = {"t": round(t, 1), "wall": _dt.datetime.now().isoformat(timespec="seconds")}
    s["server_alive"] = proc is None or proc.poll() is None
    t0 = time.time()
    status, js, err = get_json(url + "/health", timeout=10)
    s["health_ok"] = status == 200 and isinstance(js, dict) and js.get("status") == "ok"
    s["health_ms"] = round((time.time() - t0) * 1000)
    if not s["health_ok"]:
        s["health_error"] = err or str(js)[:200]
    status, js, err = get_json(url + "/state", timeout=20)
    s["state_status"] = status
    if status == 200:
        problems, summ = validate_state(js)
        s.update(summ)
        s["state_problems"] = problems
        s["state_ok"] = not problems
    else:
        detail = js.get("detail") if isinstance(js, dict) else None
        s["state_problems"] = [str(detail or err)[:300]]
        s["state_ok"] = False
    status, png, err = http("GET", url + "/screenshot", timeout=20)
    size = png_size(png) if status == 200 else None
    s["screenshot_ok"] = status == 200 and size == (160, 144)
    if status == 200 and png:
        shot = Path("shots") / f"s{idx:04d}.png"
        (run_dir / shot).write_bytes(png)
        s["shot"] = str(shot)
        s["mono"] = is_mono(png)
    else:
        s["screenshot_error"] = err or f"status {status}"
    status, st, err = get_json(url + "/gbemu/stats", timeout=10)
    if status == 200 and isinstance(st, dict):
        for k in ("frame_count", "actions_total", "actions_ok", "actions_without_frame_advance",
                  "games_new_calls", "last_action_time", "pyboy_loaded", "backend"):
            s[k] = st.get(k)
    return s


def driver_command(args, url: str, run_dir: Path, seconds: float) -> Optional[List[str]]:
    if args.driver == "script":
        return [args.python, str(TOOLS / "scripted_driver.py"), "--url", url, "--seconds", str(int(seconds)),
                "--log", str(run_dir / "driver-actions.jsonl")]
    if args.driver == "agent":
        return ["bash", str(TOOLS / "drive_loop.sh")]
    return None


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="G5 playtest monitor (PLAN §4.4)")
    ap.add_argument("--minutes", type=float, default=30.0, help="driving time (default 30)")
    ap.add_argument("--driver", choices=["script", "agent", "none"], default="script")
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--rom", default=os.environ.get("GBEMU_POKERED", str(REPO / "roms" / "pokered.gb")),
                    help="ROM (default $GBEMU_POKERED or <repo>/roms/pokered.gb)")
    ap.add_argument("--lib", default=os.environ.get("GBEMU_LIB"),
                    help="libgb_ffi.dylib to pin (default $GBEMU_LIB)")
    ap.add_argument("--out-root", default=str(REPO / "reports" / "playtest"))
    ap.add_argument("--run-id", default=None, help="run directory name (default: timestamp)")
    ap.add_argument("--interval", type=float, default=SAMPLE_INTERVAL, help="sampling period in seconds")
    ap.add_argument("--python", default=sys.executable, help="interpreter for the server and scripted driver")
    ap.add_argument("--launcher", default="gbemu.launch", help="module started with `-m … serve` (default gbemu.launch)")
    ap.add_argument("--attach", default=None, metavar="URL",
                    help="debug: monitor an already running server instead of starting one")
    ap.add_argument("--ready-timeout", type=float, default=120.0)
    args = ap.parse_args(argv)

    run_id = args.run_id or _dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    run_dir = Path(args.out_root).resolve() / run_id
    data_dir = run_dir / "data"
    (run_dir / "shots").mkdir(parents=True, exist_ok=True)
    data_dir.mkdir(parents=True, exist_ok=True)
    url = (args.attach or f"http://localhost:{args.port}").rstrip("/")
    meta: Dict[str, Any] = {"run_id": run_id, "driver": args.driver, "minutes": args.minutes, "port": args.port,
                            "url": url, "data_dir": str(data_dir), "git_head": git_head(),
                            "started": _dt.datetime.now().isoformat(timespec="seconds")}

    def setup_fail(msg: str) -> int:
        print(f"ERROR {msg}", file=sys.stderr)
        meta["setup_error"] = msg
        (run_dir / "summary.md").write_text(f"# Playtest {run_id}\n\n**Verdict: SETUP ERROR** — {msg}\n")
        (run_dir / "summary.json").write_text(json.dumps({"meta": meta, "verdict": "SETUP_ERROR"}, indent=2))
        return EXIT_SETUP

    server = None
    env = dict(os.environ)
    env["PYTHONPATH"] = os.pathsep.join([str(REPO / "python")] + [p for p in [env.get("PYTHONPATH")] if p])
    if args.attach is None:
        rom = Path(args.rom).expanduser().resolve()
        if not rom.is_file():
            return setup_fail(f"ROM not found: {rom}")
        if not args.lib:
            return setup_fail("GBEMU_LIB is not set (or pass --lib)")
        lib = Path(args.lib).expanduser().resolve()
        if not lib.is_file():
            return setup_fail(f"GBEMU_LIB not found: {lib}")
        pinned = data_dir / "gbemu-lib" / lib.name
        pinned.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(lib, pinned)
        env["GBEMU_LIB"] = str(pinned)
        meta.update(rom=str(rom), rom_sha1=sha(rom, "sha1"), gbemu_lib=f"{lib} (pinned copy {pinned})",
                    gbemu_lib_sha256=sha(pinned, "sha256"))
        status, _, _ = http("GET", url + "/health", timeout=2)
        if status is not None:
            return setup_fail(f"something is already listening on port {args.port}")
        cmd = [args.python, "-m", args.launcher, "serve", "--rom", str(rom), "--port", str(args.port),
               "--data-dir", str(data_dir)]
        server_log = open(run_dir / "server.log", "wb")
        server = subprocess.Popen(cmd, cwd=str(REPO), env=env, stdout=server_log, stderr=subprocess.STDOUT,
                                  start_new_session=True)
        meta["server_cmd"] = " ".join(cmd)
    ok, why = wait_ready(url, server, args.ready_timeout)
    if not ok:
        stop_process(server, "server")
        return setup_fail(f"server not ready: {why} (see server.log)")

    status, payload, err = http("POST", url + "/games/new", {"name": f"playtest-{run_id}"}, timeout=180)
    meta["games_new"] = status
    if status != 200:
        stop_process(server, "server")
        return setup_fail(f"/games/new failed: {status} {err} {payload[:300]!r}")

    seconds = args.minutes * 60.0
    denv = dict(env)
    denv.update(PORT=str(args.port), URL=url, RUN_DIR=str(run_dir), MINUTES=str(args.minutes),
                SECONDS_TOTAL=str(int(seconds)), PYTHON=args.python)
    dcmd = driver_command(args, url, run_dir, seconds)
    driver = None
    if dcmd:
        driver_log = open(run_dir / "driver.log", "wb")
        driver = subprocess.Popen(dcmd, cwd=str(REPO), env=denv, stdout=driver_log, stderr=subprocess.STDOUT,
                                  start_new_session=True)
        meta["driver_cmd"] = " ".join(dcmd)

    samples: List[dict] = []
    samples_f = open(run_dir / "samples.jsonl", "w", encoding="utf-8")
    t_start = time.time()
    idx = 0
    print(f"[playtest] {run_dir} driver={args.driver} minutes={args.minutes}", flush=True)
    try:
        while True:
            t = time.time() - t_start
            s = take_sample(url, t, idx, run_dir, server)
            samples.append(s)
            samples_f.write(json.dumps(s) + "\n")
            samples_f.flush()
            idx += 1
            print(f"[playtest] t={_fmt_t(t)} map={s.get('map_id')} xy=({s.get('x')},{s.get('y')}) "
                  f"actions={s.get('actions_total')} frames={s.get('frame_count')} "
                  f"state_ok={s.get('state_ok')} mono={s.get('mono')}", flush=True)
            if not s["server_alive"]:
                print("[playtest] server died; stopping", flush=True)
                break
            if t >= seconds:
                break
            next_t = t_start + idx * args.interval
            time.sleep(max(0.0, min(next_t, t_start + seconds) - time.time()))
    except KeyboardInterrupt:
        print("[playtest] interrupted", flush=True)
    finally:
        samples_f.close()
        stop_process(driver, "driver", grace=15)
        status, st, _ = get_json(url + "/gbemu/stats", timeout=10)
        final_stats = st if status == 200 and isinstance(st, dict) else None
        server_alive_at_end = server is None or server.poll() is None
        server_exit = None if server_alive_at_end else server.returncode
        stop_process(server, "server")

    meta["ended"] = _dt.datetime.now().isoformat(timespec="seconds")
    meta["server_exit"] = server_exit
    rounds = read_driver_rounds(run_dir / "driver-rounds.jsonl")
    result = evaluate(samples, final_stats, args.driver, rounds, server_exit)
    make_contact_sheet(run_dir, samples)
    (run_dir / "summary.md").write_text(render_summary(meta, result, samples, rounds), encoding="utf-8")
    (run_dir / "summary.json").write_text(json.dumps({"meta": meta, "result": result, "final_stats": final_stats},
                                                     indent=2, default=str), encoding="utf-8")
    print(f"[playtest] {result['verdict']} — {run_dir / 'summary.md'}")
    for h in result["hard"]:
        if not h["ok"]:
            print(f"ERROR {h['name']}: {h['detail']}")
    for w in result["warnings"]:
        print(f"WARN {w}")
    return result["exit_code"]


if __name__ == "__main__":
    sys.exit(main())
