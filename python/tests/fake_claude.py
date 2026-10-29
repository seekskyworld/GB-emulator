#!/usr/bin/env python3
"""Stand-in for `claude -p … --output-format json` used by the drive_loop.sh tests.

Reads the prompt from stdin, checks the flags drive_loop.sh must pass, then
(FAKE_CLAUDE_MODE=ok) POSTs two /action calls and an /objectives update to the
server named in the prompt and prints a JSON result with the session id;
FAKE_CLAUDE_MODE=fail exits 1; FAKE_CLAUDE_MODE=hang sleeps forever.
"""

import json
import os
import re
import sys
import time
import urllib.request

args = sys.argv[1:]
prompt = sys.stdin.read()
log = os.environ.get("FAKE_CLAUDE_LOG")
if log:
    with open(log, "a") as f:
        f.write(json.dumps({"argv": args, "prompt_len": len(prompt)}) + "\n")


def flag(name):
    return args[args.index(name) + 1] if name in args else None


assert "-p" in args
assert flag("--model") == "global.anthropic.claude-sonnet-5-5", args
assert flag("--effort") == "high"
assert flag("--output-format") == "json"
assert flag("--permission-mode") == "dontAsk"
i = args.index("--allowedTools")
assert args[i + 1 : i + 3] == ["Bash(curl *localhost*)", "Read"], args
assert "禁止调用 `/games/new`、`/games/{id}/load`、`/load`" in prompt
assert "{{" not in prompt, "unrendered placeholder"
sid = flag("--session-id")
port = re.search(r"localhost:(\d+)", prompt).group(1)
base = f"http://localhost:{port}"

mode = os.environ.get("FAKE_CLAUDE_MODE", "ok")
if mode == "fail":
    print("boom", file=sys.stderr)
    sys.exit(1)
if mode == "hang":
    time.sleep(3600)


def post(path, body):
    req = urllib.request.Request(base + path, data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.status


post("/action", {"actions": ["walk_down"]})
post("/action", {"actions": ["walk_right", "press_a"]})
post("/objectives", {"objectives": [{"tier": "primary", "text": "fake round", "done": False}]})
print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "session_id": sid,
                  "result": "did 2 actions"}))
