"""Process helpers for tests that start a local HTTP server."""

from __future__ import annotations

import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

# 本机设置了 http_proxy（127.0.0.1:7897）而没有 no_proxy：urllib/curl 默认会把 localhost 请求也交给代理，
# 端口未监听时得到的是代理的 502（数秒后）而不是"连接被拒绝"。所有 localhost 请求一律绕过代理。
_OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))

TESTS = Path(__file__).resolve().parent
PYDIR = TESTS.parent
REPO = PYDIR.parent


def free_port() -> int:
    """A currently free TCP port *outside* the ephemeral range (macOS: 49152-65535).

    Asking the kernel for port 0 hands out an ephemeral port, which outgoing connections (this
    machine runs a local HTTP proxy) can grab again before the server binds it; that showed up as
    uvicorn exiting with "[Errno 48] address already in use". Ports are checked on 0.0.0.0, which
    is what uvicorn binds."""
    import random

    rng = random.Random()
    for _ in range(200):
        port = rng.randrange(20000, 45000)
        with socket.socket() as s:
            try:
                s.bind(("0.0.0.0", port))
            except OSError:
                continue
            return port
    raise RuntimeError("no free port found")


def env_with_paths(extra: dict | None = None) -> dict:
    env = dict(os.environ)
    env["PYTHONPATH"] = os.pathsep.join([str(PYDIR), str(TESTS)] + [p for p in [env.get("PYTHONPATH")] if p])
    env.update(extra or {})
    return env


def get(url: str, timeout: float = 10.0):
    with _OPENER.open(url, timeout=timeout) as r:
        return r.status, r.headers.get("content-type", ""), r.read()


def post(url: str, body: dict, timeout: float = 30.0):
    req = urllib.request.Request(url, data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json"})
    try:
        with _OPENER.open(req, timeout=timeout) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"null")


def wait_health(url: str, proc: subprocess.Popen, timeout: float = 60.0) -> dict:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"server exited with {proc.returncode}")
        try:
            status, _, body = get(url + "/health", timeout=2)
            js = json.loads(body)
            if status == 200 and js.get("emulator_ready"):
                return js
        except Exception:  # noqa: BLE001
            pass
        time.sleep(0.3)
    raise TimeoutError(f"{url} not ready")


class Server:
    """Context manager: start `python <args>` in its own process group, stop it on exit."""

    def __init__(self, args: list, env: dict, log: Path, cwd: Path = REPO) -> None:
        self.args, self.env, self.log, self.cwd = args, env, log, cwd
        self.proc: subprocess.Popen | None = None

    def __enter__(self) -> "Server":
        self._log = open(self.log, "wb")
        self.proc = subprocess.Popen([sys.executable, *self.args], env=self.env, cwd=str(self.cwd),
                                     stdout=self._log, stderr=subprocess.STDOUT, start_new_session=True)
        return self

    def __exit__(self, *exc) -> None:
        if self.proc and self.proc.poll() is None:
            os.killpg(self.proc.pid, signal.SIGINT)
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.proc.pid, signal.SIGKILL)
                self.proc.wait()
        self._log.close()
