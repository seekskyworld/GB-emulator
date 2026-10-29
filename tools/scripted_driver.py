#!/usr/bin/env python3
"""tools/scripted_driver.py — 不用 LLM 的回归驾驶脚本（PLAN §4.4 `--driver script`）。

只通过 pokemon-agent 的 HTTP 接口（``POST /action``，其响应里的 ``state_after``）驱动：

1. 开场：标题 → NEW GAME → 跳过对话 → 选择预设名字（主角、劲敌都选列表里的第一个预设名）；
2. 之后随机行走（带方向惯性，撞墙就换方向），定期按 A/B；对话中按 A，战斗中以 A 为主。

    python tools/scripted_driver.py --url http://localhost:8765 --seconds 120

``--intro auto``（默认）在主角与劲敌名字都已设置时跳过开场，所以
``drive_loop.sh`` 的兜底可以在游戏中途直接调用。本脚本从不调用 ``/games/new``、``/load``。

开场按键序列的安全性（不读游戏内部菜单状态，只靠顺序）：
- 阶段 1 只按 START：片头可被跳过，标题画面进入主菜单，主菜单上 START 要么确认 NEW GAME 要么被忽略；
- 阶段 2a 只按 A：在主菜单上确认 NEW GAME（光标默认在 NEW GAME）；对话中 A 只是翻页；
- 阶段 2b 按 DOWN 再按 A：对话框不响应方向键，起名菜单里 DOWN 把光标从 NEW NAME 移到第一个预设名。
  DOWN 只在主菜单上有害（会移到 OPTION），而主菜单已在 2a 结束前离开。
"""

from __future__ import annotations

import argparse
import http.client
import json
import random
import re
import sys
import time
import urllib.parse
from typing import Any, List, Optional, Tuple

DIRECTIONS = ["up", "down", "left", "right"]
# 卡死恢复（reports/scout/oaks-lab-stall.md）：游戏自身的竞态会让 PrepareOAMData 写越界到 wTileMap，
# 主角被隐藏、四个方向都判为碰撞而自锁；START 再 B 会让游戏重建 wTileMap。服务端没有读任意地址的
# 端点，所以只用"在大地图上（不在战斗/对话中）连续 STALL_MOVES 次走动坐标都不变"作为触发条件，
# 并要求这期间四个方向都试过（避免在死胡同、墙角里误触发：那里总有一个方向能走）。
STALL_MOVES = 8
RECOVER_TILEMAP = ["press_start", "wait_30", "press_b", "wait_30"]
NAME_RE = re.compile(r"^[A-Z]{1,7}$")
# 新游戏开始时 WRAM 里已经写着这两个默认名字（实测：标题画面之后、起名之前 /state 就读到它们），
# 所以"名字已设置"必须排除它们。
DEFAULT_NAMES = ("NINTEN", "SONY")


class Client:
    def __init__(self, url: str, timeout: float = 60.0, log=None) -> None:
        self.url = url.rstrip("/")
        self.timeout = timeout
        self.log = log
        self.t0 = time.time()
        self.requests = 0
        self.failures = 0
        self.prefix = urllib.parse.urlsplit(self.url).path.rstrip("/")
        self._c: Optional[http.client.HTTPConnection] = None

    def _conn(self) -> http.client.HTTPConnection:
        if self._c is None:
            u = urllib.parse.urlsplit(self.url)
            self._c = http.client.HTTPConnection(u.hostname or "localhost", u.port or 80, timeout=self.timeout)
        return self._c

    def _req(self, method: str, path: str, body: Optional[dict] = None) -> Tuple[Optional[int], Any]:
        """One request over a persistent keep-alive connection.

        A new TCP connection per request (urllib) leaves a TIME_WAIT socket each time; at several
        hundred requests per second that exhausts the ephemeral ports ("[Errno 49] Can't assign
        requested address") and starves the monitor. http.client also ignores http_proxy."""
        data = json.dumps(body).encode() if body is not None else None
        headers = {"Content-Type": "application/json"} if data else {}
        for attempt in (0, 1):
            reused = self._c is not None
            try:
                c = self._conn()
                c.request(method, self.prefix + path, body=data, headers=headers)
                r = c.getresponse()
                raw = r.read()
                try:
                    js = json.loads(raw.decode("utf-8") or "null")
                except ValueError:
                    js = None
                return r.status, js
            except (http.client.HTTPException, OSError) as e:
                if self._c is not None:
                    self._c.close()
                self._c = None
                # Retry once only when a reused keep-alive connection had been closed by the server
                # (idle timeout); otherwise the request may already have run and must not be repeated.
                stale = reused and isinstance(e, (http.client.RemoteDisconnected, BrokenPipeError,
                                                  ConnectionResetError))
                if attempt == 1 or not stale:
                    return None, {"error": f"{type(e).__name__}: {e}"}
        return None, None

    def action(self, actions: List[str], note: Optional[str] = None) -> Tuple[Optional[int], Optional[dict]]:
        t = time.time()
        status, js = self._req("POST", "/action", {"actions": actions})
        self.requests += 1
        state = js.get("state_after") if isinstance(js, dict) and status == 200 else None
        if status != 200:
            self.failures += 1
        if self.log is not None:
            rec = {"t": round(t - self.t0, 2), "ms": round((time.time() - t) * 1000), "actions": actions,
                   "status": status}
            rec.update(summarize(state))
            if note:
                rec["note"] = note
            if status != 200:
                rec["error"] = str(js)[:300]
            self.log.write(json.dumps(rec) + "\n")
            self.log.flush()
        return status, state

    def state(self) -> Optional[dict]:
        status, js = self._req("GET", "/state")
        return js if status == 200 and isinstance(js, dict) else None


def summarize(state: Optional[dict]) -> dict:
    """Small, never-failing view of /state for decisions and logs."""
    out: dict = {}
    if not isinstance(state, dict):
        return out
    try:
        out["map"] = (state.get("map") or {}).get("map_id")
        pos = (state.get("player") or {}).get("position") or {}
        out["x"], out["y"] = pos.get("x"), pos.get("y")
        out["dialog"] = bool((state.get("dialog") or {}).get("active"))
        out["battle"] = bool((state.get("battle") or {}).get("in_battle"))
        out["name"] = (state.get("player") or {}).get("name")
        out["rival"] = (state.get("player") or {}).get("rival_name")
    except Exception:  # noqa: BLE001
        pass
    return out


def names_set(state: Optional[dict]) -> bool:
    s = summarize(state)
    name, rival = str(s.get("name") or ""), str(s.get("rival") or "")
    if (name, rival) == DEFAULT_NAMES or name == DEFAULT_NAMES[0] or rival == DEFAULT_NAMES[1]:
        return False
    return bool(NAME_RE.match(name) and NAME_RE.match(rival))


def position(state: Optional[dict]):
    s = summarize(state)
    if s.get("x") is None:
        return None
    return (s.get("map"), s.get("x"), s.get("y"))


class Driver:
    def __init__(self, client: Client, deadline: float, rng: random.Random, out=sys.stdout) -> None:
        self.c = client
        self.deadline = deadline
        self.rng = rng
        self.out = out
        self.last_state: Optional[dict] = None
        self.conn_fail_since: Optional[float] = None
        self.recoveries = 0

    max_requests: Optional[int] = None

    def time_left(self) -> bool:
        if self.max_requests is not None and self.c.requests >= self.max_requests:
            return False
        return time.time() < self.deadline

    def act(self, actions: List[str], note: Optional[str] = None) -> Optional[dict]:
        status, state = self.c.action(actions, note)
        if status is None:
            if self.conn_fail_since is None:
                self.conn_fail_since = time.time()
            elif time.time() - self.conn_fail_since > 30:
                raise ConnectionError(f"server unreachable for 30s at {self.c.url}")
            time.sleep(1.0)
        else:
            self.conn_fail_since = None
        if state is not None:
            self.last_state = state
        return state

    def say(self, msg: str) -> None:
        print(f"[scripted] {msg}", file=self.out, flush=True)

    # -- opening ------------------------------------------------------------

    def opening(self) -> bool:
        """Title → NEW GAME → dialogue → preset names. Returns True when both names are set."""
        self.say("opening: phase 1 (START through intro/title)")
        for _ in range(24):
            if not self.time_left():
                return False
            self.act(["press_start", "wait_40"])
        self.say("opening: phase 2a (A: NEW GAME)")
        for _ in range(3):
            if not self.time_left():
                return False
            self.act(["press_a", "wait_30"])
        self.say("opening: phase 2b (DOWN+A: dialogue and preset names)")
        done_at = None
        for i in range(160):
            if not self.time_left():
                return False
            state = self.act(["press_down", "press_a", "wait_20"])
            if done_at is None and names_set(state):
                done_at = i
                self.say(f"opening: names set ({summarize(state).get('name')}/{summarize(state).get('rival')})")
            if done_at is not None and i - done_at >= 12:
                break
        for _ in range(4):  # finish the last lines of Oak's speech
            if not self.time_left():
                break
            self.act(["press_a", "wait_30"])
        ok = names_set(self.last_state)
        self.say("opening: done" if ok else "opening: names not detected, continuing with random walk")
        return ok

    # -- battle ---------------------------------------------------------------

    def battle_actions(self) -> List[str]:
        """One battle step. The main battle menu is FIGHT/PKMN (top) and ITEM/RUN (bottom).
        Every step starts with B three times: that backs out of any sub-menu (party list with
        SWITCH/STATS/CANCEL, an empty ITEM list, the move list) to the main menu and advances
        text; B does nothing on the main menu itself. Then UP+LEFT selects FIGHT (A, A = first
        move) or DOWN+RIGHT selects RUN (fails harmlessly in trainer battles)."""
        back = ["press_b", "press_b", "press_b"]
        if self.rng.random() < 0.25:
            return back + ["press_down", "press_right", "press_a", "wait_30"]
        return back + ["press_up", "press_left", "press_a", "wait_10", "press_a", "wait_30"]

    # -- free roaming -------------------------------------------------------

    def roam(self) -> None:
        self.say("roam: random walk with periodic A/B")
        direction = self.rng.choice(DIRECTIONS)
        streak = 0
        n = 0
        stalled = 0  # consecutive walk requests (overworld, no battle/dialog) without a position change
        tried: set = set()  # directions tried during that streak
        while self.time_left():
            n += 1
            s = summarize(self.last_state)
            if s.get("battle"):
                stalled, tried = 0, set()
                self.act(self.battle_actions())
                continue
            if s.get("dialog"):
                stalled, tried = 0, set()
                self.act(["press_a", "wait_20"])
                continue
            if stalled >= STALL_MOVES and len(tried) == len(DIRECTIONS):
                self.recoveries += 1
                self.say(f"recover_tilemap #{self.recoveries}: {stalled} walk requests in all 4 directions "
                         f"without moving at "
                         f"{position(self.last_state)}; START then B")
                self.act(RECOVER_TILEMAP, note="recover_tilemap")
                stalled, tried = 0, set()
                continue
            if streak <= 0 or self.rng.random() < 0.15:
                direction = self.rng.choice(DIRECTIONS)
                streak = self.rng.randint(2, 6)
            steps = self.rng.randint(1, 3)
            actions = [f"walk_{direction}"] * steps
            if n % 6 == 0:
                actions.append("press_a")
            if n % 11 == 0:
                actions.append("press_b")
            before = position(self.last_state)
            state = self.act(actions)
            streak -= steps
            if state is not None:
                after = summarize(state)
                moved = position(state) != before
                if moved or after.get("battle") or after.get("dialog"):
                    stalled, tried = 0, set()
                else:
                    stalled += 1
                    tried.add(direction)
            if state is not None and position(state) == before:
                # blocked (or a menu/text ate the input): pick another direction next time
                direction = self.rng.choice([d for d in DIRECTIONS if d != direction])
                streak = self.rng.randint(2, 6)


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Scripted (no-LLM) pokered driver over the pokemon-agent HTTP API")
    ap.add_argument("--url", default=None, help="server URL (default http://localhost:<port>)")
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--seconds", type=float, default=60.0, help="wall-clock driving time (default 60)")
    ap.add_argument("--seed", type=int, default=None, help="random seed (default: time based)")
    ap.add_argument("--intro", choices=["auto", "yes", "no"], default="auto",
                    help="run the opening sequence: auto = only if the names are not set yet")
    ap.add_argument("--log", default=None, help="append one JSON line per /action to this file")
    ap.add_argument("--max-requests", type=int, default=None,
                    help="also stop after this many /action requests (for deterministic tests)")
    args = ap.parse_args(argv)

    url = args.url or f"http://localhost:{args.port}"
    seed = args.seed if args.seed is not None else int(time.time())
    log = open(args.log, "a", encoding="utf-8") if args.log else None
    client = Client(url, log=log)
    drv = Driver(client, time.time() + args.seconds, random.Random(seed))
    drv.max_requests = args.max_requests
    drv.say(f"url={url} seconds={args.seconds} seed={seed} intro={args.intro}")
    try:
        intro = args.intro
        if intro == "auto":
            st = client.state()
            if st is None:
                drv.say("could not read /state; assuming the opening is still needed")
            intro = "no" if names_set(st) else "yes"
            if st is not None:
                drv.last_state = st
        if intro == "yes":
            drv.opening()
        drv.roam()
    except ConnectionError as e:
        drv.say(f"ERROR {e}")
        return 1
    except KeyboardInterrupt:
        pass
    finally:
        drv.say(f"done: {client.requests} /action requests, {client.failures} failed, "
                f"{drv.recoveries} recover_tilemap")
        if log:
            log.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
