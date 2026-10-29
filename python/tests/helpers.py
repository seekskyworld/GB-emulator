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

TESTS = Path(__file__).resolve().parent
PYDIR = TESTS.parent
REPO = PYDIR.parent


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def env_with_paths(extra: dict | None = None) -> dict:
    env = dict(os.environ)
    env["PYTHONPATH"] = os.pathsep.join([str(PYDIR), str(TESTS)] + [p for p in [env.get("PYTHONPATH")] if p])
    env.update(extra or {})
    return env


def get(url: str, timeout: float = 10.0):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return r.status, r.headers.get("content-type", ""), r.read()


def post(url: str, body: dict, timeout: float = 30.0):
    req = urllib.request.Request(url, data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
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
