#!/usr/bin/env python3
"""tools/export_sessions.py 的单测（标准库 unittest）：python3 -I tools/test_export_sessions.py

样本中的密钥全是伪造的。验证：各类密钥全部打码、只打码不删除、无密钥文件字节不变、INDEX 正确、
stdout 不出现密钥原文、拒绝在没有 --final 时写入仓库 sessions/。
"""

import contextlib
import io
import json
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import export_sessions as ex  # noqa: E402

FAKE_SK = "sk-ant-api03-FAKEfake0123456789abcdefABCDEF-_xyz"
FAKE_SK2 = "sk-proj-FAKE9876543210zyxwvu"
FAKE_API = "fakeApiKeyValue12345678"
FAKE_AUTH = "fakeAuthTokenValue_ABCDEFGH12345"
FAKE_BEARER = "eyFAKEbearerToken.abc.def123"
FAKE_XAPI = "fakeXApiKey0987654321"
ALL_FAKES = [FAKE_SK, FAKE_SK2, FAKE_API, FAKE_AUTH, FAKE_BEARER, FAKE_XAPI]


def jl(*records):
    return "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in records)


def build_sources(root: Path) -> Path:
    src = root / "-Users-x-GB-emulator"
    sess = "11111111-2222-3333-4444-555555555555"
    (src / sess / "subagents").mkdir(parents=True)
    (src / sess / "tool-results").mkdir(parents=True)
    # 主会话：bash 输出里带 ANTHROPIC_API_KEY=、Authorization 头（JSON 内被转义的形式）
    (src / f"{sess}.jsonl").write_text(
        jl(
            {"type": "mode", "mode": "x", "sessionId": sess},
            {"type": "user", "entrypoint": "cli", "cwd": "/repo/GB-emulator", "timestamp": "2026-10-29T10:00:00.000Z",
             "message": {"role": "user", "content": "start T00"}},
            {"type": "assistant", "entrypoint": "cli", "timestamp": "2026-10-29T10:01:00.000Z",
             "message": {"content": [{"type": "text", "text": f"env: ANTHROPIC_API_KEY={FAKE_API}\n"
                                      f"curl -H 'Authorization: Bearer {FAKE_BEARER}' -H \"x-api-key: {FAKE_XAPI}\""}]}},
            {"type": "ai-title", "aiTitle": "Integrator: scaffold | refs", "sessionId": sess},
            {"type": "user", "entrypoint": "cli", "timestamp": "2026-10-29T10:05:00.000Z",
             "message": {"role": "user", "content": f"token is {FAKE_AUTH} and key {FAKE_SK}"}},
        ),
        encoding="utf-8",
    )
    # 子代理 + meta
    (src / sess / "subagents" / "agent-abc.jsonl").write_text(
        jl(
            {"type": "user", "entrypoint": "cli", "isSidechain": True, "cwd": "/lanes/platform",
             "timestamp": "2026-10-29T11:00:00.000Z", "message": {"role": "user", "content": "do T05a"}},
            {"type": "assistant", "isSidechain": True, "timestamp": "2026-10-29T11:30:00.000Z",
             "message": {"content": [{"type": "text", "text": f'{{"ANTHROPIC_AUTH_TOKEN": "{FAKE_AUTH}"}}'}]}},
        ),
        encoding="utf-8",
    )
    (src / sess / "subagents" / "agent-abc.meta.json").write_text(
        json.dumps({"agentType": "platform", "description": "platform lane: gates T05a", "model": "opus", "effort": "high"}),
        encoding="utf-8",
    )
    # claude -p 驾驶会话
    (src / "99999999-0000-0000-0000-000000000000.jsonl").write_text(
        jl(
            {"type": "user", "entrypoint": "sdk-cli", "timestamp": "2026-10-30T01:00:00.000Z",
             "message": {"role": "user", "content": "drive pokemon"}},
            {"type": "assistant", "entrypoint": "sdk-cli", "timestamp": "2026-10-30T01:02:00.000Z",
             "message": {"content": [{"type": "text", "text": "pressed A; " + FAKE_SK2}]}},
        ),
        encoding="utf-8",
    )
    # 工具输出：没有密钥，含说明文字与非 UTF-8 字节，必须字节不变
    (src / sess / "tool-results" / "out.txt").write_bytes(
        b"set ANTHROPIC_API_KEY=... first; task-1234567890abcdefgh risk-0123456789abcdefgh \xff\xfe end\n"
    )
    return src


class ExportTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.src = build_sources(root)
        self.out = root / "out"
        self.out.mkdir()
        os.environ["ANTHROPIC_AUTH_TOKEN"] = FAKE_AUTH
        os.environ.pop("ANTHROPIC_API_KEY", None)
        self.literals = ex.secret_values(extra_settings=[])
        self.infos = ex.export([self.src], self.out, self.literals)
        (self.out / "INDEX.md").write_text(ex.index_md(self.infos, [self.src]), encoding="utf-8")

    def tearDown(self):
        self.tmp.cleanup()
        os.environ.pop("ANTHROPIC_AUTH_TOKEN", None)

    def all_text(self):
        return "".join(
            f.read_bytes().decode("utf-8", errors="surrogateescape") for f in self.out.rglob("*") if f.is_file()
        )

    def test_every_fake_secret_is_redacted(self):
        text = self.all_text()
        for fake in ALL_FAKES:
            self.assertNotIn(fake, text)
        # 前缀保留，便于看出打码位置
        self.assertIn("ANTHROPIC_API_KEY=" + ex.REDACTED, text)
        self.assertIn("Authorization: Bearer " + ex.REDACTED, text)
        self.assertIn("x-api-key: " + ex.REDACTED, text)

    def test_only_redacts_never_deletes(self):
        main = (self.out / self.src.name / "11111111-2222-3333-4444-555555555555.jsonl").read_text(encoding="utf-8")
        orig = (self.src / "11111111-2222-3333-4444-555555555555.jsonl").read_text(encoding="utf-8")
        self.assertEqual(len(main.splitlines()), len(orig.splitlines()))
        for line in main.splitlines():
            json.loads(line)  # 打码后仍是合法 JSON
        self.assertIn("start T00", main)
        self.assertIn("curl -H", main)

    def test_untouched_file_is_byte_identical(self):
        rel = Path(self.src.name) / "11111111-2222-3333-4444-555555555555" / "tool-results" / "out.txt"
        self.assertEqual((self.out / rel).read_bytes(), (self.src.parent / rel).read_bytes())

    def test_hits_per_file(self):
        by = {i.rel.split("/")[-1]: i for i in self.infos}
        main = by["11111111-2222-3333-4444-555555555555.jsonl"]
        self.assertEqual(main.hits.get("ANTHROPIC_API_KEY"), 1)
        self.assertEqual(main.hits.get("authorization-bearer"), 1)
        self.assertEqual(main.hits.get("x-api-key"), 1)
        self.assertEqual(main.hits.get("env-literal"), 1)
        self.assertEqual(main.hits.get("sk-key"), 1)
        sub = by["agent-abc.jsonl"]
        self.assertGreaterEqual(sub.hit_total, 1)
        self.assertEqual(by["99999999-0000-0000-0000-000000000000.jsonl"].hits, {"sk-key": 1})
        self.assertEqual(by["out.txt"].hits, {})

    def test_index(self):
        idx = (self.out / "INDEX.md").read_text(encoding="utf-8")
        for fake in ALL_FAKES:
            self.assertNotIn(fake, idx)
        rows = {}
        for line in idx.splitlines():
            if line.startswith("| `"):
                cells = [c.strip() for c in re.split(r"(?<!\\)\|", line)[1:-1]]
                rows[cells[0].strip("`").split("/")[-1]] = cells
        self.assertEqual(len(rows), 5)
        m = rows["11111111-2222-3333-4444-555555555555.jsonl"]
        self.assertEqual(m[1], "主会话")
        self.assertEqual((m[2], m[3], m[4]), ("2026-10-29T10:00:00.000Z", "2026-10-29T10:05:00.000Z", "3"))
        self.assertTrue(m[5].startswith("5 "), m[5])
        self.assertEqual(m[7], "Integrator: scaffold \\| refs")
        self.assertEqual(m[8], "GB-emulator")
        s = rows["agent-abc.jsonl"]
        self.assertEqual(s[1], "子代理")
        self.assertEqual(s[4], "2")
        self.assertEqual(s[6], "platform opus high")
        self.assertEqual(s[7], "platform lane: gates T05a")
        self.assertEqual(s[8], "platform")
        d = rows["99999999-0000-0000-0000-000000000000.jsonl"]
        self.assertEqual(d[1], "claude -p 驾驶")
        self.assertEqual(d[7], "drive pokemon")
        self.assertEqual(rows["agent-abc.meta.json"][1], "子代理元数据")
        self.assertEqual(rows["out.txt"][1], "工具输出")
        self.assertIn("env-literal", idx)

    def test_redact_patterns_directly(self):
        cases = [
            (f"ANTHROPIC_API_KEY: {FAKE_API}", FAKE_API),
            (f'\\"ANTHROPIC_API_KEY\\": \\"{FAKE_API}\\"', FAKE_API),
            (f"export ANTHROPIC_AUTH_TOKEN={FAKE_AUTH}", FAKE_AUTH),
            (f'"authorization": "bearer {FAKE_BEARER}"', FAKE_BEARER),
            (f"X-Api-Key={FAKE_XAPI}", FAKE_XAPI),
            (f"({FAKE_SK})", FAKE_SK),
        ]
        for text, secret in cases:
            red, hits = ex.redact(text, [])
            self.assertNotIn(secret, red, text)
            self.assertIn(ex.REDACTED, red)
            self.assertTrue(hits)
        # 不误伤：说明文字、普通单词里的 sk-
        for benign in ["ANTHROPIC_API_KEY=...", "task-1234567890abcdefghij", "use `sk-` prefixed keys", "x-api-key: <key>"]:
            self.assertEqual(ex.redact(benign, []), (benign, {}), benign)
        # 幂等：再跑一遍不再命中
        red, _ = ex.redact(f"ANTHROPIC_API_KEY={FAKE_API} {FAKE_SK}", [])
        self.assertEqual(ex.redact(red, []), (red, {}))

    def test_main_stdout_has_no_secret_and_guards_sessions_dir(self):
        out2 = Path(self.tmp.name) / "out2"
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = ex.main(["--out", str(out2), "--src", str(self.src)])
        self.assertEqual(rc, 0, buf.getvalue())
        printed = buf.getvalue()
        for fake in ALL_FAKES:
            self.assertNotIn(fake, printed)
        self.assertIn("files copied: 5", printed)
        self.assertIn("self-check", printed)
        # 非空输出目录需要 --force
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(ex.main(["--out", str(out2), "--src", str(self.src)]), 1)
            self.assertEqual(ex.main(["--out", str(out2), "--src", str(self.src), "--force"]), 0)
        # 没有 --final 不得写进仓库 sessions/
        repo_sessions = Path(ex.__file__).resolve().parent.parent / "sessions" / "x"
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            self.assertEqual(ex.main(["--out", str(repo_sessions), "--src", str(self.src)]), 2)
        self.assertFalse(repo_sessions.exists())


if __name__ == "__main__":
    unittest.main(verbosity=1)
