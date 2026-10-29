"""Launcher: run the pokemon-agent server on the gbemu backend (PLAN §3.8, way 1).

Usage::

    GBEMU_LIB=/abs/target/release/libgb_ffi.dylib \
        python -m gbemu.launch serve --rom roms/pokered.gb --port 8765 --data-dir /tmp/run/data

What it does, in order:

1. blocks any later ``import pyboy`` (the red line: PyBoy is never used);
2. points ``pokemon_agent.emulator._EXT_MAP['.gb'/'.gbc']`` at
   :class:`gbemu.backend.GbEmuEmulator` (``server.py`` imports
   ``create_emulator`` inside its functions, so patching the module dict is
   enough for both startup and ``/games/new``);
3. asserts ``"pyboy" not in sys.modules``;
4. adds a read-only monitoring probe to the FastAPI app (``GET /gbemu/stats``:
   per-``/action`` frame-count deltas, ``/games/new`` count, backend info) —
   existing endpoints are not changed;
5. hands over to ``pokemon_agent.cli.main()`` with the given arguments.
"""

from __future__ import annotations

import importlib.abc
import os
import sys
import threading
import time
from collections import deque
from typing import List, Optional

_installed = False


class _BlockPyboy(importlib.abc.MetaPathFinder):
    """Make ``import pyboy`` fail loudly instead of silently using PyBoy."""

    def find_spec(self, fullname, path=None, target=None):  # noqa: D401
        if fullname == "pyboy" or fullname.startswith("pyboy."):
            raise ImportError("pyboy is forbidden in this project (gbemu.launch blocks it)")
        return None


def install() -> None:
    """Route .gb/.gbc ROMs to GbEmuEmulator (idempotent)."""
    global _installed
    if not any(isinstance(f, _BlockPyboy) for f in sys.meta_path):
        sys.meta_path.insert(0, _BlockPyboy())
    import pokemon_agent.emulator as pa_emulator

    from gbemu.backend import GbEmuEmulator

    pa_emulator._EXT_MAP[".gb"] = GbEmuEmulator
    pa_emulator._EXT_MAP[".gbc"] = GbEmuEmulator
    _installed = True
    assert "pyboy" not in sys.modules, "pyboy must never be imported"


class ActionProbe:
    """Records, for every ``POST /action``, the backend frame count before/after.

    The server advances frames only on demand, so a successful ``/action`` that
    leaves ``frame_count`` unchanged means the backend is not actually running.
    """

    def __init__(self, server_module, history: int = 64) -> None:
        self._server = server_module
        self._lock = threading.Lock()
        self.started = time.time()
        self.actions_total = 0
        self.actions_ok = 0
        self.actions_failed = 0
        self.actions_without_frame_advance = 0
        self.last_action_time: Optional[float] = None
        self.last_action_frames: Optional[int] = None
        self.games_new_calls = 0
        self.recent: deque = deque(maxlen=history)

    def _frames(self):
        emu = getattr(self._server, "_emulator", None)
        return emu, (getattr(emu, "frame_count", None) if emu is not None else None)

    async def middleware(self, request, call_next):
        path, method = request.url.path, request.method
        if method == "POST" and path == "/games/new":
            with self._lock:
                self.games_new_calls += 1
            return await call_next(request)
        if not (method == "POST" and path == "/action"):
            return await call_next(request)
        emu0, f0 = self._frames()
        t0 = time.time()
        response = await call_next(request)
        emu1, f1 = self._frames()
        with self._lock:
            self.actions_total += 1
            ok = response.status_code == 200
            delta = (f1 - f0) if (emu0 is emu1 and f0 is not None and f1 is not None) else None
            if ok:
                self.actions_ok += 1
                if delta is not None and delta <= 0:
                    self.actions_without_frame_advance += 1
            else:
                self.actions_failed += 1
            self.last_action_time = time.time()
            self.last_action_frames = delta
            self.recent.append(
                {"t": round(t0, 3), "status": response.status_code, "frames_before": f0,
                 "frames_after": f1, "delta": delta}
            )
        return response

    def snapshot(self) -> dict:
        emu, frames = self._frames()
        with self._lock:
            return {
                "backend": type(emu).__name__ if emu is not None else None,
                "frame_count": frames,
                "pyboy_loaded": "pyboy" in sys.modules,
                "gbemu_lib": os.environ.get("GBEMU_LIB"),
                "pid": os.getpid(),
                "uptime_seconds": round(time.time() - self.started, 1),
                "actions_total": self.actions_total,
                "actions_ok": self.actions_ok,
                "actions_failed": self.actions_failed,
                "actions_without_frame_advance": self.actions_without_frame_advance,
                "last_action_time": self.last_action_time,
                "last_action_frames": self.last_action_frames,
                "games_new_calls": self.games_new_calls,
                "recent_actions": list(self.recent),
            }


def instrument(app=None, server_module=None) -> ActionProbe:
    """Attach the ``/gbemu/stats`` probe to the pokemon-agent FastAPI app."""
    if server_module is None:
        import pokemon_agent.server as server_module  # noqa: PLW0127
    if app is None:
        app = server_module.app
    existing = getattr(app.state, "gbemu_probe", None)
    if existing is not None:
        return existing
    probe = ActionProbe(server_module)
    app.middleware("http")(probe.middleware)

    async def gbemu_stats():
        return probe.snapshot()

    app.add_api_route("/gbemu/stats", gbemu_stats, methods=["GET"])
    app.state.gbemu_probe = probe
    return probe


def main(argv: Optional[List[str]] = None) -> None:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "serve":
        # Fail before uvicorn starts if the library is missing.
        from gbemu import native

        native.load_library()
    install()
    import pokemon_agent.cli as pa_cli

    if argv and argv[0] == "serve":
        instrument()
    assert "pyboy" not in sys.modules, "pyboy must never be imported"
    sys.argv = ["pokemon-agent", *argv]
    pa_cli.main()


if __name__ == "__main__":
    main()
