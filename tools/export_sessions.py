#!/usr/bin/env python3
"""导出 Claude Code 会话记录到 sessions/（PLAN §7 第 3 项，T91），复制时打码密钥并生成 INDEX.md。

用法：
    python3 -I tools/export_sessions.py --out /tmp/gbemu-sessions-dryrun          # dry-run（任意目录）
    python3 -I tools/export_sessions.py --out sessions --final                    # 正式导出（T91）

源目录默认为 ~/.claude/projects/-Users-skyseek-Desktop-project-open-research-platform-GB-emulator*
（覆盖主仓库与各 lane worktree 的项目目录；子代理记录在 <session>/subagents/ 下）。
每个源目录复制到 <out>/<源目录名>/，保持相对结构；所有文件按文本处理（surrogateescape，字节不变），
命中的密钥替换为 ***REDACTED***，其余内容原样保留。

stdout 只打印文件数与各类命中次数，绝不打印密钥原文。只用标准库。
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path

REDACTED = "***REDACTED***"
DEFAULT_GLOB = "~/.claude/projects/-Users-skyseek-Desktop-project-open-research-platform-GB-emulator*"

# 值：令牌字符集（base64/base64url/十六进制等）、至少 8 个字符；已打码的 ***REDACTED*** 不再匹配。
# 只认令牌字符，避免把文档里的 `ANTHROPIC_API_KEY=...` 之类说明文字连带打掉。
_VALUE = r"[A-Za-z0-9._~+/=\-]{8,}"
# 名字与值之间允许的分隔：= 或 :，以及 JSON 中转义过的引号。
_SEP = r"""(?:\\?["'])?\s*[=:]\s*(?:\\?["'])?\s*"""

PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    # 名字=值 形式：值整段打码（保留名字，便于看出打码位置）
    ("ANTHROPIC_API_KEY", re.compile(r"(ANTHROPIC_API_KEY" + _SEP + r")(" + _VALUE + r")")),
    ("ANTHROPIC_AUTH_TOKEN", re.compile(r"(ANTHROPIC_AUTH_TOKEN" + _SEP + r")(" + _VALUE + r")")),
    ("authorization-bearer", re.compile(r"(?i)(authorization" + _SEP + r"bearer\s+)(" + _VALUE + r")")),
    ("x-api-key", re.compile(r"(?i)(x-api-key" + _SEP + r")(" + _VALUE + r")")),
    # sk- 开头的密钥样式（前面不能紧挨字母数字，如 task-、disk-）
    ("sk-key", re.compile(r"()((?<![A-Za-z0-9])sk-[A-Za-z0-9_\-]{16,})")),
    # 已部分打码、只露出前几位的令牌（如 sk-abcd***）：前缀也一并屏蔽
    ("sk-prefix", re.compile(r"()((?<![A-Za-z0-9])sk-[A-Za-z0-9]{1,15}(?=\*{3}))")),
]

# 环境变量与 Claude 设置中的这些名字的实际值，出现在任何位置都打码。
# ANTHROPIC_BASE_URL 是内部网关地址，不是凭据，但同样不对外公开，按字面值打码。
SECRET_ENV_NAMES = ("ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY", "ANTHROPIC_BASE_URL")


def secret_values(extra_settings: list[Path] | None = None) -> list[str]:
    """收集需要按字面值打码的密钥：环境变量，以及 ~/.claude/settings*.json 的 env 中同名项。不打印。"""
    vals: set[str] = set()
    for name in SECRET_ENV_NAMES:
        v = os.environ.get(name, "")
        if len(v) >= 8:
            vals.add(v)
    settings = extra_settings
    if settings is None:
        home = Path.home() / ".claude"
        settings = [home / "settings.json", home / "settings.local.json"]
    for p in settings:
        try:
            env = json.loads(p.read_text(encoding="utf-8")).get("env", {})
        except (OSError, ValueError, AttributeError):
            continue
        if not isinstance(env, dict):
            continue
        for name in SECRET_ENV_NAMES:
            v = env.get(name)
            if isinstance(v, str) and len(v) >= 8:
                vals.add(v)
    # sk- 令牌的前 7 个字符（sk- 加 4 位）也按字面值打码：记录中可能出现只露出前几位的写法。
    # 前缀在运行时从真实令牌推导，绝不写进代码或输出。
    vals |= {v[:7] for v in list(vals) if v.startswith("sk-") and len(v) >= 12}
    # 长的先替换，避免短值是长值的子串时留下残片
    return sorted(vals, key=len, reverse=True)


def redact(text: str, literals: list[str]) -> tuple[str, dict[str, int]]:
    """返回 (打码后的文本, 各类命中次数)。"""
    hits: dict[str, int] = {}
    for lit in literals:
        n = text.count(lit)
        if n:
            text = text.replace(lit, REDACTED)
            hits["env-literal"] = hits.get("env-literal", 0) + n
    for name, pat in PATTERNS:
        text, n = pat.subn(lambda m: m.group(1) + REDACTED, text)
        if n:
            hits[name] = hits.get(name, 0) + n
    return text, hits


@dataclass
class FileInfo:
    rel: str  # 相对 <out> 的路径
    kind: str
    first_ts: str = ""
    last_ts: str = ""
    messages: int = 0
    hits: dict[str, int] = field(default_factory=dict)
    agent: str = ""
    task: str = ""
    cwd: str = ""

    @property
    def hit_total(self) -> int:
        return sum(self.hits.values())


def _clip(s: str, n: int = 80) -> str:
    s = " ".join(str(s).split())
    return s if len(s) <= n else s[: n - 1] + "…"


def analyze_jsonl(text: str, rel: str, meta: dict | None) -> FileInfo:
    """从（已打码的）jsonl 文本推断时间范围、类型、消息数、代理与任务。"""
    info = FileInfo(rel=rel, kind="")
    entrypoints: set[str] = set()
    sidechain = False
    title = ""
    first_prompt = ""
    for line in text.splitlines():
        try:
            o = json.loads(line)
        except ValueError:
            continue
        if not isinstance(o, dict):
            continue
        ts = o.get("timestamp")
        if isinstance(ts, str):
            info.first_ts = info.first_ts or ts
            info.last_ts = ts
        t = o.get("type")
        if t in ("user", "assistant"):
            info.messages += 1
        if isinstance(o.get("entrypoint"), str):
            entrypoints.add(o["entrypoint"])
        if o.get("isSidechain") is True:
            sidechain = True
        if t == "ai-title" and isinstance(o.get("aiTitle"), str):
            title = o["aiTitle"]
        if not info.cwd and isinstance(o.get("cwd"), str):
            info.cwd = o["cwd"]
        if t == "user" and not first_prompt:
            msg = o.get("message")
            c = msg.get("content") if isinstance(msg, dict) else None
            if isinstance(c, str):
                first_prompt = c
    if "/subagents/" in "/" + rel or sidechain:
        info.kind = "子代理"
    elif entrypoints and entrypoints != {"cli"}:
        info.kind = "claude -p 驾驶"
    else:
        info.kind = "主会话"
    if meta:
        info.agent = _clip(" ".join(str(meta.get(k, "")) for k in ("agentType", "model", "effort")).strip(), 60)
        info.task = _clip(meta.get("description", ""))
    if not info.task:
        info.task = _clip(title or first_prompt)
    return info


def default_sources(pattern: str = DEFAULT_GLOB) -> list[Path]:
    return sorted(Path(p) for p in glob.glob(os.path.expanduser(pattern)) if Path(p).is_dir())


def export(sources: list[Path], out: Path, literals: list[str]) -> list[FileInfo]:
    infos: list[FileInfo] = []
    for src in sources:
        files = sorted(p for p in src.rglob("*") if p.is_file())
        for f in files:
            rel = Path(src.name) / f.relative_to(src)
            raw = f.read_bytes()
            text = raw.decode("utf-8", errors="surrogateescape")
            red, hits = redact(text, literals)
            dst = out / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(red.encode("utf-8", errors="surrogateescape"))
            shutil.copystat(f, dst)
            rel_s = rel.as_posix()
            if f.suffix == ".jsonl":
                meta = None
                meta_path = f.with_name(f.stem + ".meta.json")
                if meta_path.is_file():
                    try:
                        meta = json.loads(redact(meta_path.read_text(encoding="utf-8"), literals)[0])
                    except ValueError:
                        meta = None
                info = analyze_jsonl(red, rel_s, meta)
            elif f.name.endswith(".meta.json"):
                info = FileInfo(rel=rel_s, kind="子代理元数据")
            elif "tool-results" in f.parts:
                info = FileInfo(rel=rel_s, kind="工具输出")
            else:
                info = FileInfo(rel=rel_s, kind="其他")
            info.hits = hits
            infos.append(info)
    return infos


def _md(s: str) -> str:
    return s.replace("|", "\\|")


def index_md(infos: list[FileInfo], sources: list[Path]) -> str:
    total_hits: dict[str, int] = {}
    for i in infos:
        for k, v in i.hits.items():
            total_hits[k] = total_hits.get(k, 0) + v
    lines = [
        "# sessions INDEX",
        "",
        f"源目录（{len(sources)} 个）：" + "、".join(f"`{s.name}`" for s in sources),
        "",
        f"文件 {len(infos)} 个；会话记录（.jsonl）{sum(1 for i in infos if i.rel.endswith('.jsonl'))} 个。",
        "",
        "打码：密钥只替换为 `***REDACTED***`，不删除任何记录。命中统计："
        + ("、".join(f"{k} {v}" for k, v in sorted(total_hits.items())) if total_hits else "无"),
        "",
        "类型推断：路径含 `subagents/` 或 `isSidechain` 为子代理（代理与任务取自同名 `.meta.json`）；"
        "`entrypoint` 不是 `cli` 的为 `claude -p` 驾驶；其余为主会话（任务取最后的 AI 标题或首条提示）。",
        "",
        "| 文件 | 类型 | 首条时间 | 末条时间 | 消息数 | 打码命中 | 代理 | 任务 | cwd |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for i in infos:
        hit = str(i.hit_total) + (" (" + ", ".join(f"{k} {v}" for k, v in sorted(i.hits.items())) + ")" if i.hits else "")
        cwd = Path(i.cwd).name if i.cwd else ""
        lines.append(
            f"| `{_md(i.rel)}` | {i.kind} | {i.first_ts} | {i.last_ts} | "
            f"{i.messages if i.rel.endswith('.jsonl') else ''} | {hit} | {_md(i.agent)} | {_md(i.task)} | {_md(cwd)} |"
        )
    return "\n".join(lines) + "\n"


def leftover_literals(out: Path, literals: list[str]) -> int:
    """导出后自检：输出目录中不得再出现任何字面密钥。返回命中文件数。"""
    bad = 0
    if not literals:
        return 0
    for f in out.rglob("*"):
        if f.is_file():
            t = f.read_bytes().decode("utf-8", errors="surrogateescape")
            if any(lit in t for lit in literals):
                bad += 1
    return bad


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=True, type=Path, help="输出目录")
    ap.add_argument("--src", action="append", type=Path, help="源目录（可重复；默认按通配符查找）")
    ap.add_argument("--final", action="store_true", help="允许写入仓库的 sessions/（T91 正式导出）")
    ap.add_argument("--force", action="store_true", help="输出目录非空时先清空")
    a = ap.parse_args(argv)

    out = a.out.expanduser().resolve()
    repo = Path(__file__).resolve().parent.parent
    if not a.final and (out == repo / "sessions" or (repo / "sessions") in out.parents):
        print("ERROR refusing to write into the repo's sessions/ without --final (dry-run goes to /tmp)")
        return 2
    sources = [s.expanduser().resolve() for s in a.src] if a.src else default_sources()
    if not sources:
        print("ERROR no source directories found")
        return 1
    if out.exists() and any(out.iterdir()):
        if not a.force:
            print(f"ERROR {out} is not empty (use --force)")
            return 1
        shutil.rmtree(out)
    out.mkdir(parents=True, exist_ok=True)

    literals = secret_values()
    infos = export(sources, out, literals)
    (out / "INDEX.md").write_text(index_md(infos, sources), encoding="utf-8")

    totals: dict[str, int] = {}
    for i in infos:
        for k, v in i.hits.items():
            totals[k] = totals.get(k, 0) + v
    kinds: dict[str, int] = {}
    for i in infos:
        kinds[i.kind] = kinds.get(i.kind, 0) + 1
    print(f"sources: {len(sources)}  files copied: {len(infos)}  -> {out}")
    print("kinds: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items())))
    print("redaction hits: " + (", ".join(f"{k} {v}" for k, v in sorted(totals.items())) or "none")
          + f" (files with hits: {sum(1 for i in infos if i.hits)}; literal secrets known: {len(literals)})")
    bad = leftover_literals(out, literals)
    if bad:
        print(f"ERROR {bad} exported files still contain a known secret literal")
        return 1
    print("self-check: no known secret literal left in the export")
    return 0


if __name__ == "__main__":
    sys.exit(main())
