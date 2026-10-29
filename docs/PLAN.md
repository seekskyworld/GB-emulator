# GB（DMG）模拟器从零实现：计划与任务（交接唯一文档）

> 面向接手实施的 Claude。**本文件是唯一的交接物**：第 1–7 章是计划，第 8 章是可逐项验收的任务清单，第 9 章是参考资料要点与经验来源，不依赖任何聊天记录或其他文件。
> 题目来源：同目录下的 `GB 模拟器高速编程研究 · 技术文档与资料.rtf`（以下简称"题面"）。所有外部事实与下载地址已于 2026-10-29 实测核对。
> 冲突裁决顺序：题面红线 > §1 > 第 2–7 章 > 第 8 章 > 你自己的判断。**例外**：本文的技术陈述（第 2–3 章）若与 Pan Docs 原文或测试 ROM 的实际结果冲突，以后两者为准，并记入 `DECISIONS.md`；不要为了"符合计划"去对抗测试结果。
> 第 9 章汇总了题面参考资料（3 篇 Anthropic 文章、4 篇评测论文）的要点；本文的工作流规则大多由此而来，方括号中的 [A1]–[P4]、[M1] 标注出处。
> **v2（2026-10-30，速度优先重构）**：非 G1–G5 必需的工作全部移到 G5 之后的收尾阶段（W4）；不做计时与耗时记录；写代码的通道由 4 个减为 3 个（取消 periph，并入 core）；M0 压缩（T02 只冻结跨通道接口、模型验证简化、gates 分 T05a/T05b 两步）；30 分钟实玩默认只跑一次；记账频率降为按里程碑。

---

## 0. 开工前 5 分钟必读（TL;DR）

1. **红线**：从零写；不得阅读、复制、改写任何现成模拟器的源码（PyBoy、SameBoy、mGBA、Gambatte、BGB 等），**不安装 pyboy**。只看 `$REFS` 中的硬件文档、指令表、测试 ROM 说明与测试源码。完整保留 git 历史和全部 session 记录。
2. **路径**：仓库 `REPO=/Users/skyseek/Desktop/project/open/research-platform/GB-emulator`；参考资料 `REFS=$REPO/refs`（由 §2.1 的脚本并行下载，约 1 分钟）。所有会话都从 `$REPO` 或其 lane worktree 启动（便于导出会话记录，见 §7）。
3. **路线**：Rust 核心（无第三方依赖）→ C ABI → Python `ctypes` → pokemon-agent。
4. **先建反馈回路**：`gates` 一条命令跑完全部门槛，输出不超过 30 行，失败行以 `ERROR` 开头；详细内容写日志。**任何合并都不得让已通过的项目变为失败**（棘轮，§4.1）。
5. **接口和时序在 T02 一次定死**：每次总线访问或内部周期 = 1 个 M-cycle，外设同步推进 4 T；外设接口签名、IF/VRAM/OAM 的归属都在 T02 冻结。之后的并行开发全靠这次冻结。
6. **必过**：cpu_instrs 11/11、instr_timing、mem_timing、dmg-acid2 逐像素一致、宝可梦红 30 分钟实玩。**加分**：Mooneye。
7. **可恢复**：每完成一个任务，更新 `PROGRESS.md` 并提交。每次新会话或上下文重置后，先执行 §6.8 的开场流程（读状态 → 冒烟测试 → 确认基线完好），再开始新任务。
8. **整机约束**：所有门槛必须由同一套按 M-cycle 驱动的整机通过，禁止只为某个测试存在的代码路径（R6）。G5 之前由宝可梦冒烟与实玩充当"隐藏集"，G5 之后再用 Mooneye 复核，用来发现"单测能过、整机不对"（§4.5）。
9. **证据优先**：任务完成与否只看 `reports/gates.json` 和运行证据（日志、截图、帧哈希），不认"我已实现"之类的自述。
10. **模型配置**（只看质量与速度；本机网关只接受带 `global.anthropic.` 前缀的完整 ID，见 §6.6）：主会话 `claude --model global.anthropic.claude-opus-5-5 --effort max`，只负责冻结接口、合并、基线和攻关 Blocker，不承担大段编码；`core`、`ppu` 子代理用 Opus 5.5 xhigh；`platform`、`scout`、`reviewer` 用 Opus 5.5 high；实玩驾驶用 Sonnet 5.5 high；机械事务用 Haiku 5.5 medium；子代理不用 max。fast 模式在 T00 探测，可用才开。
11. **开工硬性前提**（T00）：宝可梦 ROM 已放到 `$REPO/roms/pokered.gb` 且 SHA1 正确（已就绪：`$REPO/ROM/pokered.gb`，由 pret/pokered @ `171e2c2` 编译，SHA1 已校验，T00 复制即可）；探测到的模型 ID 可用；子代理的权限已预先配置，且禁止联网。任一项不满足就先停下来报告，不要先写代码。
12. **优先级边界（速度优先）**：G1–G5 完成（tag `v1.0-gates`）之前，关键路径上只做 G1–G5 必需的事；Mooneye、MBC2/MBC5、性能优化、回放基线、评审与合规自查、REPORT/README/sessions 导出全部放到 G5 之后的收尾阶段 W4（§6.3）。判断一件事该不该现在做，只问一句："不做它，G1–G5 会不会变慢或过不了？"不做计时，也不记录耗时、token 与费用。

---

## 1. 目标、红线、待确认事项

### 1.1 验收目标

| 编号 | 项目 | 判定（由 gates 自动判定） | 性质 |
| --- | --- | --- | --- |
| G1 | Blargg `cpu_instrs` 11 个 individual（附带跑合集） | 串口文本含 `Passed`、不含 `Failed`；合集为 `Passed all tests` | 必过 |
| G2 | Blargg `instr_timing.gb` | 串口含 `Passed` | 必过 |
| G3 | Blargg `mem_timing/`（合集与 3 个 individual；**不是** `mem_timing-2`） | 串口含 `Passed` | 必过 |
| G4 | `dmg-acid2.gb` v1.0 | 首次执行 `LD B,B` 后再跑 1 帧（上限 120 帧），完整帧与 `reference-dmg.png` 转灰度后**逐像素一致** | 必过 |
| G5 | 宝可梦红（美版 v1.0）30 分钟实玩 | 不崩溃、不卡死；`/state` 可读；抽查截图无花屏、错位、黑屏；地图或坐标有变化（§4.4） | 必过 |
| B1 | Mooneye `mts-20260714-0944-31510e1` | 首次执行 `LD B,B` 时 B/C/D/E/H/L = 3/5/8/13/21/34 为通过；全为 0x42 或超时为失败 | 加分，按通过个数计 |

Blargg 每个 ROM 结束时另存一张截图作为"屏幕输出"证据（题面说结果"打印到屏幕，同时从串口输出"）。
不做：声音（APU 寄存器只做可读写存储）、CGB 模式、SGB 功能、执行 boot ROM、链接线对传。

### 1.2 红线（违反任何一条，本题作废）

- **R1 不看他人模拟器代码**：不 clone、不浏览、不安装。题面中的 gbdev 资源汇总页（https://gbdev.io/resources.html）列有模拟器项目链接，**不要点开它们的源码**。测试 ROM 的汇编源码（Blargg、acid2、Mooneye）属于测试，允许阅读。
- **R2 不特判测试**：`crates/core/src` 不得感知 ROM 标题、哈希或文件名；期望结果只能在 gates 中出现。
- **R3 git 历史完整**：小步提交；禁止 `rebase`、`commit --amend`、`push --force`、squash，禁止用 `reset --hard` 丢弃已提交的内容。撤销用 `git revert`。
- **R4 session 记录完整**：主会话、所有子代理、实玩驾驶会话的记录都要导出到 `sessions/`（§7）。
- **R6 整机约束，不做针对性实现**：Blargg、acid2、Mooneye、宝可梦必须由同一套整机代码通过；禁止为某个测试单独查表补周期、写死时序、跳过检查。core 中不得出现 `Passed`/`Failed` 字面量，也不得出现除 opcode 周期表（必须由 Opcodes.json 单测校验）之外的大型常量表。
- **R5 ROM 不入库**：所有 `.gb/.gbc/.sav/.state` 都不进 git。宝可梦 ROM 由接单人自行合法获取，放到 `$REPO/roms/pokered.gb`，SHA1 必须为 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`。

### 1.3 待发包方确认（未确认时按"默认"执行）

| # | 问题 | 默认 |
| --- | --- | --- |
| Q1 | 能否使用 SingleStepTests/sm83（MIT；逐指令测试向量，含逐 M-cycle 总线访问，但由其他实现生成）？ | **已确定：允许使用**。由 §2.1 的脚本下载，作为 CPU 的主要单测数据（T12，从 T10 开始就用）；只能放在 `refs/` 下由测试读取，不得编入 `crates/core/src`，也不得提交进 git |
| Q2 | 能否使用 gameboy-doctor（参考日志来自其他模拟器）？ | 不用 |
| Q3 | "高速"如何体现，能否并行子代理？ | **已确定：速度优先，不做计时与耗时记录**。T00 的准备工作必须最大限度并行：第一步就在后台启动 `fetch_refs.sh` 和 Python 依赖安装，同时进行模型探测、子代理定义与权限配置（见 T00）。允许并行子代理（§6） |
| Q4 | 30 分钟实玩由谁驾驶（pokemon-agent 自带驱动需要 `hermes` CLI）？ | `tools/drive_loop.sh` 外层循环：每一轮启动一个新的 `claude -p` 驾驶会话（Sonnet 5.5 high，§6.6），读 `/objectives`、`/state` 和截图，执行若干动作后更新 `/objectives`，然后退出；循环直到满 30 分钟。约束见 §4.4 |
| Q5 | Mooneye 计分集合？ | 适用于 DMG-ABC 的 `acceptance/` 与 `emulator-only/`（规则见 §2.4），同时报告 115 个的原始结果 |
| Q6 | 各代理使用的模型与推理强度 | **已确定**（只看质量与速度，不考虑成本）：主会话 Opus 5.5 max（fast 模式可用才开）；子代理按 §6.6 分配（编码类用 Opus 5.5 xhigh/high，实玩驾驶用 Sonnet 5.5 high，机械事务用 Haiku 5.5 medium） |
| Q7 | Python 侧的 pytest、Pillow（pokemon-agent 依赖）等通用库是否允许？ | 允许，用 `uv.lock` 固定版本。Rust 核心与 cli 零依赖；cli 只**写** PNG（stored deflate），不解码 PNG；acid2 参考图在 T01 用 Pillow 转成原始灰度字节 |

---

## 2. 资料获取与已核实事实

### 2.1 资料获取：`tools/fetch_refs.sh`（T00 时后台执行，并行下载约 1 分钟）

全部资料都由脚本下载到 `$REFS=$REPO/refs/`（已加入 `.gitignore`，不入库），并对 22 个关键文件做 sha256 校验。所有代理（包括 lane worktree 中的代理）都用**绝对路径** `$REFS` 访问。

| 需要什么 | `$REFS` 下的路径 | 来源与版本 |
| --- | --- | --- |
| Pan Docs（Markdown 源码，可直接 grep） | `pandocs/src/*.md`（如 `Timer_and_Divider_Registers.md`、`Window.md`、`halt.md`、`MBC3.md`、`Power_Up_Sequence.md`） | gbdev/pandocs @ `5a8a5ed750424667f6c31b186c2a8b9eb388dce5`（CC0） |
| Pan Docs 单页版 | `pandocs-print.html` | https://gbdev.io/pandocs/print.html |
| 指令表 | `Opcodes.json`（CC0，可提交进仓库） | gbdev/gb-opcodes @ `376f61c86fdac2048f7ce5fe838ae756b306017e` 根目录的 `Opcodes.json`（**不是** `_data/Opcodes.json`）；网页版 https://gbdev.io/gb-opcodes/optables/ |
| 指令语义 | `gbz80.7.txt`（由 man 源文件渲染） | RGBDS `v1.0.4` 的 `man/gbz80.7`（MIT） |
| Blargg | `blargg/{cpu_instrs,instr_timing,mem_timing}/`（含 readme 与源码） | retrio/gb-test-roms @ `c240dd7d700e5c0b00a7bbba52b53e4ee67b5f15` |
| dmg-acid2 | `acid2/dmg-acid2.gb`、`acid2/reference-dmg.png`；指南与失败示例 `acid2-src/README.md`、`acid2-src/img/failures/` | release v1.0；仓库 @ `8a98ce731f96dde032ffb22ec36dc985d78fdb18`（MIT） |
| Mooneye | `mooneye/mts-20260714-0944-31510e1/`（115 个 ROM）；测试汇编源码 `mooneye-src/`（`README.markdown` 含协议与型号分组） | gekkio.fi 最新一期；Gekkio/mooneye-test-suite @ `31510e12eea6286d36eea060a6adde755e1067aa`（同一版本，MIT） |
| pokemon-agent | `pokemon-agent/`（只读参考，含 `skill/SKILL.md`） | NousResearch/pokemon-agent @ `f8a03e4a8bc58f1d9dda6d677a7436cc832a57a9`（MIT） |
| 宝可梦红相关 | `pokered/{roms.sha1,Makefile,random.asm,vblank.asm}` | pret/pokered @ `171e2c204f9c1ff8e577ca387a4bbb009e130843` |
| CPU 逐指令测试（Q1 已允许） | `sst/v1/*.json`（500 个文件：无前缀 244 个，CB 前缀 256 个，文件名形如 `00.json`、`cb 00.json`，含空格；共约 161 MB） | SingleStepTests/sm83 @ `f9c30210245dd691661db39f5ace022c465ecc2f`（MIT）；codeload 下载约 8 秒 |

**不下载**：宝可梦红 ROM（由接单人合法获取）；gameboy-doctor（Q2 未获准）；任何模拟器源码（R1）。题面中的设计参考文章与论文非必读，需要时直接在线阅读（Anthropic 三篇：`harness-design-long-running-apps`、`effective-harnesses-for-long-running-agents`、`building-c-compiler`；arXiv `2609.38269`、`2609.38335`、`2605.21384`、`2608.21833`）。

**并行下载经验（实测于 2026-10-29）**

1. **GitHub 仓库用 codeload 按提交下载 tarball**，`https://codeload.github.com/<owner>/<repo>/tar.gz/<sha>`，配合 `tar --strip-components=1` 只解出需要的子目录。每个只需 1–2 秒，不带 `.git`，版本由提交号锁定。`git clone` 加 sparse 或 `--filter=blob:none` 反而更慢（Pan Docs 实测 38 秒）。
2. **所有任务用 `&` 放到后台，再逐个 `wait $pid` 收集失败**，总耗时约等于最慢的一项。Pan Docs 带大量图片，约 30–60 秒，最先启动；在它下载期间可以先用 `pandocs-print.html`（1 秒内到达）。13 项全部完成实测 45–67 秒，逐个串行下载需要数分钟；加上 SingleStepTests（约 8 秒，与其他项并行）后总耗时基本不变。
3. **每个 curl 都要加 `--retry 4 --retry-all-errors --retry-delay 2 --connect-timeout 15`**。实测 GitHub release 资源（`release-assets.githubusercontent.com`）出现过一次 `SSL_ERROR_SYSCALL`，重试即可成功。
4. **下载完立即校验 sha256**，任何不一致就让脚本失败。脚本可重复执行（覆盖写入），失败时直接重跑。
5. **文件名含空格和逗号**（如 `03-op sp,hl.gb`）：不要逐个拼 raw URL，整包下载 tarball 再解压；脚本中所有路径都加引号。
6. **RGBDS 文档网站是前端渲染的**，curl 拿不到正文；改为下载 `man/gbz80.7` 源文件，用 `mandoc -T utf8 | col -b` 转成纯文本。
7. **gb-opcodes 仓库有两个 `Opcodes.json`**，只有根目录那个才是网站发布的完整版本（sha256 `e7e3cd65…`）。
8. Mooneye 的预编译包在 gekkio.fi，不在 GitHub release；预编译包和源码要取同一版本（`31510e1`）。

```bash
#!/usr/bin/env bash
# tools/fetch_refs.sh：并行获取全部参考资料到 $1（默认 refs/），并校验 sha256
set -euo pipefail
DEST="${1:-refs}"; mkdir -p "$DEST"; cd "$DEST"
C="curl -fsSL --retry 4 --retry-all-errors --retry-delay 2 --connect-timeout 15"
gh_tar() { # 仓库 提交 目标目录 [只解出的子路径...]
  local repo=$1 sha=$2 out=$3; shift 3; mkdir -p "$out"
  local name; name="$(basename "$repo")-$sha"
  local paths=(); for p in "$@"; do paths+=("$name/$p"); done
  $C "https://codeload.github.com/$repo/tar.gz/$sha" | tar xz -C "$out" --strip-components=1 "${paths[@]}"
  echo "$sha" > "$out/COMMIT"
}
pids=()
# 最慢的一项（Pan Docs）最先启动
gh_tar gbdev/pandocs 5a8a5ed750424667f6c31b186c2a8b9eb388dce5 pandocs src LICENSE & pids+=($!)
$C -o pandocs-print.html https://gbdev.io/pandocs/print.html & pids+=($!)
gh_tar retrio/gb-test-roms c240dd7d700e5c0b00a7bbba52b53e4ee67b5f15 blargg cpu_instrs instr_timing mem_timing & pids+=($!)
gh_tar mattcurrie/dmg-acid2 8a98ce731f96dde032ffb22ec36dc985d78fdb18 acid2-src & pids+=($!)
( mkdir -p acid2 && $C -o acid2/dmg-acid2.gb https://github.com/mattcurrie/dmg-acid2/releases/download/v1.0/dmg-acid2.gb ) & pids+=($!)
( mkdir -p mooneye && $C https://gekkio.fi/files/mooneye-test-suite/mts-20260714-0944-31510e1/mts-20260714-0944-31510e1.tar.gz -o mooneye/mts.tar.gz && tar xzf mooneye/mts.tar.gz -C mooneye ) & pids+=($!)
gh_tar Gekkio/mooneye-test-suite 31510e12eea6286d36eea060a6adde755e1067aa mooneye-src & pids+=($!)
$C -o Opcodes.json https://raw.githubusercontent.com/gbdev/gb-opcodes/376f61c86fdac2048f7ce5fe838ae756b306017e/Opcodes.json & pids+=($!)
( $C -o gbz80.7 https://raw.githubusercontent.com/gbdev/rgbds/v1.0.4/man/gbz80.7 && (mandoc -T utf8 gbz80.7 2>/dev/null || man ./gbz80.7) | col -b > gbz80.7.txt ) & pids+=($!)
gh_tar NousResearch/pokemon-agent f8a03e4a8bc58f1d9dda6d677a7436cc832a57a9 pokemon-agent & pids+=($!)
gh_tar SingleStepTests/sm83 f9c30210245dd691661db39f5ace022c465ecc2f sst v1 LICENSE & pids+=($!)
( mkdir -p pokered && for f in roms.sha1 Makefile engine/math/random.asm home/vblank.asm; do
    $C -o "pokered/$(basename "$f")" "https://raw.githubusercontent.com/pret/pokered/171e2c204f9c1ff8e577ca387a4bbb009e130843/$f"; done ) & pids+=($!)
fail=0; for p in "${pids[@]}"; do wait "$p" || fail=1; done
[ "$fail" = 0 ] || { echo "ERROR 有下载任务失败，重跑本脚本即可（已完成的会被覆盖）"; exit 1; }
cp acid2-src/img/reference-dmg.png acid2/reference-dmg.png
shasum -a 256 -c - <<'SUMS'
8c5e12f41e0ba5bbca796944f92ffe6de28809198682c4332e38d1b3cf56fcf2  blargg/cpu_instrs/cpu_instrs.gb
fe61349cbaee10cc384b50f356e541c90d1bc380185716706b5d8c465a03cf89  blargg/cpu_instrs/individual/01-special.gb
fb90b0d2b9501910c49709abda1d8e70f757dc12020ebf8409a7779bbfd12229  blargg/cpu_instrs/individual/02-interrupts.gb
ca553e606d9b9c86fbd318f1b916c6f0b9df0cf1774825d4361a3fdff2e5a136  blargg/cpu_instrs/individual/03-op sp,hl.gb
7686aa7a39ef3d2520ec1037371b5f94dc283fbbfd0f5051d1f64d987bdd6671  blargg/cpu_instrs/individual/04-op r,imm.gb
d504adfa0a4c4793436a154f14492f044d38b3c6db9efc44138f3c9ad138b775  blargg/cpu_instrs/individual/05-op rp.gb
17ada54b0b9c1a33cd5429fce5b765e42392189ca36da96312222ffe309e7ed1  blargg/cpu_instrs/individual/06-ld r,r.gb
ab31d3daaaa3a98bdbd9395b64f48c1bdaa889aba5b19dd5aaff4ec2a7d228a3  blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb
974a71fe4c67f70f5cc6e98d4dc8c096057ff8a028b7bfa9f7a4330038cf8b7e  blargg/cpu_instrs/individual/08-misc instrs.gb
b28e1be5cd95f22bd1ecacdd33c6f03e607d68870e31a47b15a0229033d5ba2a  blargg/cpu_instrs/individual/09-op r,r.gb
7f5b8e488c6988b5aaba8c2a74529b7c180c55a58449d5ee89d606a07c53514a  blargg/cpu_instrs/individual/10-bit ops.gb
0ec0cf9fda3f00becaefa476df6fb526c434abd9d4a4beac237c2c2692dac5d3  blargg/cpu_instrs/individual/11-op a,(hl).gb
646067b3d6c79fda810e9c3f1cb7c0efd5abb0a7ac06437c54e65720c15d9925  blargg/instr_timing/instr_timing.gb
791cb418fd054b482a6ada5017e2405b96c2373a0f3914c6b23afeee76fa98ce  blargg/mem_timing/mem_timing.gb
52724532c5709e38e947eb429337c124c38bc68f373874435a7460548098b617  blargg/mem_timing/individual/01-read_timing.gb
eea92d3f4e95aab5910e0f7080916a3c42a2b8deae1ee5d45d1e3751d648f3f6  blargg/mem_timing/individual/02-write_timing.gb
2e9067c670ff8b45916bf321677ad04a6896d06a057dbcb82ae9f208a1ae9c34  blargg/mem_timing/individual/03-modify_timing.gb
464e14b7d42e7feea0b7ede42be7071dc88913f75b9ffa444299424b63d1dff1  acid2/dmg-acid2.gb
ca966d50895c7efef05838590d148c2cbfd7fba57dab986f25b35b4da71abb57  acid2/reference-dmg.png
5f0694e4e9e58441db23ff057a5ab681870d4cfaed91267e99885936db8e6f1d  mooneye/mts.tar.gz
e7e3cd657d8e87b44570474eb3a6ed735501c9a00520f3d6937881203c823bc5  Opcodes.json
5c1e4c3e2e599c5f5f3e8da150f6c132b55dd209ff824b7ae18fd9c749326350  pokered/roms.sha1
fa7f656f78aec939dea1094fd48f555dbde9a796458c5ae2b3e5c236e9720d66  sst/v1/00.json
9833b63c2c9b0ed99adae86c6bde6fd6702ada7a876ff80f2c3adc638ef787c5  sst/v1/cb 00.json
9b9e332ee11019f131e945d40e20ab0c48b52921754dc03fe62cc85b579e6c61  sst/LICENSE
SUMS
echo "OK refs ready: $(pwd)"
```

### 2.2 pokemon-agent（提交 `f8a03e4a8bc58f1d9dda6d677a7436cc832a57a9`，MIT）

- `Emulator` 抽象方法：`load(rom_path)`、`close()`、`press(button, frames=1)`（按住 frames 帧后松开）、`release_all()`、`tick(frames=1)`、`get_screen() -> PIL.Image`、`read_u8/read_u16/read_u32(addr)`（小端）、`read_range(addr, size) -> bytes`、`save_state(path)`、`load_state(path)`；基类 `__init__` 设置 `frame_count`、`rom_path`。按键名：`a b start select up down left right`，未知按键抛 `ValueError`。
- `create_emulator()` 先 `cls()` 再 `.load()`，按扩展名查模块级 `_EXT_MAP`；`server.py` 在**函数内**导入 `create_emulator`（启动时和 `/games/new` 时各一次）。覆盖 `_EXT_MAP['.gb']`、`['.gbc']` 即可接入；`/games/new` 不会 close 旧实例，后端需要在 `__del__` 中释放原生句柄。
- **所有模拟器调用都通过 `_run_sync`（线程池）执行**：监控读取 `/state`、`/screenshot` 和驾驶 agent 的 `/action` 可能并发进入同一个实例。后端必须用一把 `threading.Lock` 串行化所有原生调用。
- 服务端是被动的、按需推进帧：一次动作通常是 `press(btn, 8)` 加 `tick(12)`。`red.py`、`collision.py` 只读 WRAM `0xC000–0xDFFF`。`read_*` 必须是无副作用的 peek。
- CLI：`serve --rom <path> --port <n> --data-dir <dir>`，`--data-dir` 默认 `~/.pokemon-agent`（会残留旧存档，每次实玩都必须显式指定新目录）；`.gb/.gbc` 都识别为 `red`。

### 2.3 宝可梦红

- pret Makefile：`-m MBC3+RAM+BATTERY -r 03`（类型 `0x13`，32 KiB RAM，无 RTC），带 SGB 标志；ROM 1 MiB。按 DMG 运行，忽略 SGB 数据包。
- 随机数每次读 `rDIV`（`engine/math/random.asm`），跨模拟器结果不同属正常。
- 易踩坑：LCD 关闭期间不出 VBlank，但 `tick(frames)` 仍要按 70224 T/帧推进，**绝不能用"等到 VBlank"作为帧边界**；OAM DMA 例程在 HRAM 执行；对话框和菜单用 Window；游戏会检查链接线（外部时钟传输永不完成，不能卡死）。

### 2.4 测试 ROM 协议（已对照各自的 README 和源码）

- **卡带类型**：Blargg 的 cpu_instrs、instr_timing、mem_timing 全部是 MBC1（`0x147=0x01`；合集 64 KiB，individual 32 KiB）；dmg-acid2 是 ROM only（32 KiB）。因此 G1–G3 依赖 MBC1。
- **Blargg**：每个字符写 `SB(FF01)` 后写 `SC(FF02)=0x81`，然后软件延时，**不等待 SC.7**；结束后死循环。测试代码复制到 `$D000` 执行，WRAM 必须可执行。instr_timing 依赖定时器；mem_timing 用定时器测量指令内第几个 M-cycle 发生访问。超时：individual 60 秒、合集 180 秒（模拟时间，检测到结果即提前结束）。
- **Mooneye**（`$REFS/mooneye-src/README.markdown`）：通过时寄存器为斐波那契数，执行 `LD B,B`，再通过串口发送这 6 个字节，**并用忙等待等传输完成**（所以内部时钟传输必须在 8×512 T 后清 SC.7，否则卡住），最后 `LD B,B` 加 `JR` 自循环；失败时为 0x42。型号分组：G = dmg+mgb，S = sgb+sgb2，C = cgb+agb+ags，A = agb+ags。**纳入**：无型号后缀；后缀含 `G`（如 `-GS`）；后缀含 `dmgABC`（如 `-dmgABC`、`-dmgABCmgb`）。**排除**：其余后缀（`-dmg0`、`-mgb`、`-S`、`-sgb`、`-sgb2`、`-cgb*`、`-A`、`-C`），以及 `manual-only/`、`madness/`、`utils/`、`misc/`。
- **dmg-acid2**（`$REFS/acid2-src/README.md`）：逐行渲染即可；在 mode 2 用 LY=LYC 中断改寄存器；灰度必须精确为 `0xFF/0xAA/0x55/0x00`（色号 0 对应 `0xFF`）。

### 2.5 开机状态

不执行 boot ROM，`PC=0x0100`。CPU 与 I/O 寄存器初值严格照 `$REFS/pandocs/src/Power_Up_Sequence.md` 的 DMG 列。Mooneye 的 `boot_regs-dmgABC`、`boot_hwio-dmgABCmgb`、`boot_div-dmgABCmgb` 会校验这些初值（加分项）。

---

## 3. 架构

### 3.1 仓库布局与包名

```
$REPO/
  Cargo.toml                 # workspace；[profile.release] opt-level=3, lto="thin", debug=1
  crates/core/  (包名 gb-core，零依赖)
    src/lib.rs machine.rs cpu.rs decode.rs alu.rs bus.rs interrupts.rs
        timer.rs serial.rs joypad.rs ppu.rs dma.rs state.rs
        cart/{mod,mbc1,mbc2,mbc3,mbc5}.rs
    tests/data/Opcodes.json  # 从 $REFS 复制，CC0（单测只依赖它，不依赖 refs/）
    tests/*.rs
  crates/cli/   (包名 gb-cli，二进制名 gb-cli，零依赖；只写 PNG，参考图读原始灰度字节)
  crates/ffi/   (包名 gb-ffi，cdylib → target/release/libgb_ffi.dylib)
  python/       (pyproject.toml，包名 gbemu；gbemu/{native,backend,launch}.py；tests/)
  patches/pokemon-agent-emulator.patch   # 替代接入方式（§3.8）
  tools/{fetch_refs.sh, playtest.py, scripted_driver.py}
  refs/ roms/ reports/ sessions/ locks/ # refs/ 与 roms/ 不入库；roms/ 只放 pokered.gb
  PROGRESS.md DECISIONS.md REPORT.md README.md .gitignore
```

`.gitignore`：`/target`、`/refs`、`/roms`、`/ROM`、`.DS_Store`、`/.claude/*` 加例外 `!/.claude/agents/`（子代理定义要入库）、`*.sav`、`*.state`、`.venv`、`reports/**/*.png`（证据图用 `git add -f` 单独提交）、`reports/logs/`。

### 3.2 冻结接口（T02 只冻结**跨通道**接口：`Machine` 对外方法、`CpuBus`、`Peripheral`/`Snapshot`、`StateWriter/StateReader`、`Cart`、ppu 的 VRAM/OAM 访问与 `Ppu` 结构体；之后改动必须先记 DECISIONS.md）

```rust
// ---- machine.rs：对外接口 ----
pub struct Machine { cpu: Cpu, bus: Bus }
impl Machine {
    pub fn new(rom: Vec<u8>) -> Result<Self, CartError>;  // 解析头部、选择 MBC、设置 DMG 开机状态
    pub fn step(&mut self) -> u32;          // 执行 1 条指令或 1 次中断分派，返回 T 数；HALT/STOP/锁死时也返回 4，绝不返回 0
    pub fn run_frame(&mut self);            // 精确推进 70224 T（余量累计到下一帧），与 LCD 开关无关
    pub fn set_buttons(&mut self, mask: u8);// bit0..7 = A B Select Start Right Left Up Down，1=按下
    pub fn frame(&self) -> &[u8; 23040];    // 最近一个完整帧（前缓冲），值为 0xFF/0xAA/0x55/0x00
    pub fn peek(&self, addr: u16) -> u8;    // 无副作用读
    pub fn take_serial(&mut self) -> Vec<u8>;
    pub fn regs(&self) -> Regs;             // a f b c d e h l sp pc ime halted
    pub fn ld_b_b_hit(&mut self) -> bool;   // 自上次查询以来是否执行过 0x40，读后清零
    pub fn save_state(&self) -> Vec<u8>;
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), StateError>;
}

// ---- cpu.rs 依赖的总线 trait ----
pub trait CpuBus {
    fn read(&mut self, addr: u16) -> u8;    // 1 M-cycle
    fn write(&mut self, addr: u16, v: u8);  // 1 M-cycle
    fn idle(&mut self);                     // 1 M-cycle（内部周期）
    fn pending(&self) -> u8;                // IE & IF & 0x1F
    fn ack(&mut self, bit: u8);             // 清除 IF 对应位
}

// ---- 外设统一形态（timer/serial/joypad/ppu/dma 都遵守）----
pub trait Peripheral: Snapshot {
    fn tick_mcycle(&mut self) -> u8;        // 推进 4 T，返回本周期新请求的中断位（IF 位掩码）
    fn read_reg(&self, addr: u16) -> u8;
    fn write_reg(&mut self, addr: u16, v: u8);
}
pub trait Snapshot {
    fn save(&self, w: &mut StateWriter);
    fn load(&mut self, r: &mut StateReader) -> Result<(), StateError>;
}

// ---- alu.rs：纯函数，Flags 进 Flags 出（INC/DEC 必须保留 C）。不属于 T02 冻结范围：alu 只被 cpu 使用，签名由 core 在 T11 自定 ----
pub struct Flags { pub z: bool, pub n: bool, pub h: bool, pub c: bool }
pub fn add8(a: u8, b: u8, f: Flags, use_carry: bool) -> (u8, Flags); // ADD/ADC
pub fn sub8(a: u8, b: u8, f: Flags, use_carry: bool) -> (u8, Flags); // SUB/SBC/CP
// and8 or8 xor8 inc8 dec8 daa rlc rrc rl rr sla sra srl swap bit add16_hl add_sp_e ……（由 core 自定）
```

**归属（避免并行时冲突）**：IF/IE 由 `interrupts.rs` 持有；VRAM 与 OAM 数组由 `ppu.rs` 持有（bus 转发访问）；OAM DMA 由 `bus.rs` 驱动（每个 M-cycle 从总线读 1 字节，写入 ppu 的 OAM），`dma.rs` 只保存 DMA 状态；卡带 ROM/RAM 由 `cart/` 持有；WRAM、HRAM 由 `bus.rs` 持有。`Bus` 每个 M-cycle 依次推进 timer → serial → joypad → ppu → dma，把返回的中断位或进 IF。

### 3.3 时序模型

- CPU 驱动时钟：每条指令按真实的 M-cycle 序列调用 `read/write/idle`。**T02 定下默认顺序：在每个 M-cycle 内先推进外设 4 T，再执行访问**，写进 DECISIONS.md D002。T30 若用 mem_timing 证明需要反过来，就全局切换（一处代码），不得按指令打补丁。外设单测不依赖这个顺序。
- 每条指令的 M-cycle 总数必须等于 `Opcodes.json` 的 `cycles/4`（条件指令分 taken/not taken）。
- 内部周期位置：`PUSH` 在写之前 1 个；`CALL`、`RST` 在压栈前 1 个；`RET`、`RETI` 弹栈后 1 个；`RET cc` 判断条件时 1 个，条件成立时弹栈后再 1 个；`JP nn`、`JP cc`（成立时）、`JR`（成立时）各 1 个；**`JP HL` 没有内部周期（共 1 M-cycle）**；`ADD HL,rr`、`INC/DEC rr`、`LD SP,HL` 各 1 个；`ADD SP,e` 2 个；`LD HL,SP+e` 1 个。
- 中断：每条指令结束后检查 `IME && pending()`；分派共 5 M-cycle（2 个 idle、压 PCH、压 PCL、跳转向量），选最低位。`EI` 延迟一条指令生效；`RETI` 立即置 IME；`DI` 立即生效。HALT：有挂起中断即唤醒（IME=0 时继续执行而不分派），IME=0 且已有挂起中断时触发 HALT bug（`$REFS/pandocs/src/halt.md`）。HALT 期间每个 M-cycle 照常推进外设。
- `IF` 读出时高 3 位为 1；未使用的 I/O 读 `0xFF`；`FEA0–FEFF` 读 `0x00`；非法 opcode 使 CPU 锁死（外设照常推进）。

### 3.4 定时器（`Timer_and_Divider_Registers.md`、`Timer_Obscure_Behaviour.md`）

16 位内部计数器每 T 加 1，`DIV` 为其高 8 位；写 `DIV` 清零整个计数器。`TIMA` 在"所选位 AND TAC.enable"的**下降沿**加 1（TAC 00→bit9、01→bit3、10→bit5、11→bit7），因此写 DIV 或改 TAC 也可能触发一次加 1。溢出后 TIMA 先为 0x00，1 个 M-cycle 后装入 TMA 并置 IF bit2。

### 3.5 PPU（逐扫描线渲染加双缓冲）

- 每行 456 dot，共 154 行。mode 2 占 80 dot，mode 3 先固定 172 dot（可变长度是加分项），其余为 mode 0；LY 144–153 为 mode 1。
- **进入 mode 3 的时刻一次性渲染整行**到后缓冲。不能在行开始时渲染：acid2 会在 mode 2 期间由 LYC 中断改寄存器。
- **双缓冲**：进入 LY=144 时把后缓冲交换为前缓冲，`frame()` 只返回前缓冲。LCD 关闭时把前缓冲置为全白一次，LY=0、mode=0，帧计时照走；重新开启后从 LY=0 开始。
- STAT：LYC 标志与中断、mode 0/1/2 中断源，以**内部中断线的上升沿**触发。进入 LY=144 时请求 VBlank 中断。
- BG/Window：`LCDC.4` 选择 tile data 寻址（`0x8000` 无符号，或以 `0x9000` 为基址的有符号），`LCDC.3/6` 选择 tilemap，SCX/SCY 环绕。**窗口 Y 条件逐帧锁存**：某行开始时若 `WY == LY`，Y 条件变为真，并保持到本帧结束；窗口在 `LCDC.5=1`、Y 条件为真且 `WX` 在 0–166 时，从 `x = WX-7` 开始绘制；**窗口内部行计数器**只在该行实际绘制了窗口时加 1，每帧清零（`Window.md`）。`LCDC.0=0` 时 BG 和 Window 都显示色号 0（DMG 上精灵仍照常显示，并且总在 BG 之上）。
- 精灵：`LCDC.1=0` 时完全不显示精灵（但 OAM scan 照常进行）；`LCDC.2` 选择 8×8 或 8×16。在 OAM scan 阶段按 OAM 顺序选出 Y 命中的前 **10** 个（不论 X 是否可见）；8×16 模式忽略 tile 号 bit0，**Y 翻转作用于整个 16 行**（上下两个 tile 互换并各自翻转）；X/Y 翻转；色号 0 透明；OBP0/OBP1；DMG 优先级：X 小者在上，X 相同时 OAM 序号小者在上；OBJ-to-BG priority 位只让精灵退到 BG/Window **色号 1–3** 之下。
- VRAM/OAM 的访问封锁默认不做（G 门槛不需要）；Mooneye 需要时再加，并跑全量回归。

### 3.6 卡带（逐种写明，不要互相照搬）

| 类型（头部 0x147） | ROM bank 选择 | 写 0 时 | RAM |
| --- | --- | --- | --- |
| `00` ROM only | 无 | — | 无（或 8 KiB） |
| `01–03` MBC1 | 5 位寄存器加 2 位高位（mode 1 时作用于 0000 区和 RAM bank） | **只看低 5 位**：低 5 位为 0 时当作 1（因此 0x20/0x40/0x60 映射为 0x21/0x41/0x61） | 最多 4×8 KiB |
| `05–06` MBC2 | 4 位，地址 bit8=1 时写入 | 0 当作 1 | 内置 512×4 bit，读出高 4 位为 1，地址按 0x1FF 镜像 |
| `0F–13` MBC3 | 7 位 | 0 当作 1 | 4×8 KiB；`08–0C` 为 RTC 寄存器，读写存储即可 |
| `19–1E` MBC5 | 9 位（低 8 位加 bit8） | **0 就是 bank 0** | 最多 16×8 KiB |

bank 号按 ROM 实际 bank 数取模；RAM 未使能（`0x0A` 低 4 位）时读 `0xFF`、写忽略；未知类型返回 `CartError`，不 panic。MBC1 多卡带（multicart）为加分项。
**G5 之前只实现 ROM only、MBC1、MBC3**（G1–G5 只用到这三种，T50）；MBC2、MBC5 在 W4 补（T50b，供 Mooneye 使用），补齐之前这两类返回 `CartError`。

### 3.7 串口与 Joypad

- 串口：写 `SC` 时若 bit7=1 且 bit0=1（内部时钟），立即把 `SB` 追加到输出缓冲区；4096 T 后把 `SB` 置为 `0xFF`、清 SC.7、置 IF bit3。外部时钟传输永不完成。
- Joypad `FF00`：bit4=0 选方向键，bit5=0 选功能键，低有效；未选中的组读 1；高 2 位读 1。**中断只在被选中组的 P1 低 4 位出现 1→0 跳变时触发**（每个 M-cycle 比较即可）。

### 3.8 FFI 与 Python 后端

- C ABI（`gb-ffi`）：`gb_new(ptr,len)->*mut Gb`（失败返回 null）、`gb_free`、`gb_run_frames(gb,n)`、`gb_set_buttons(gb,mask)`、`gb_frame(gb,out23040)`、`gb_peek(gb,addr)`、`gb_peek_range(gb,addr,len,out)`、`gb_save_state(gb,out,cap)->i64`（cap 不足时返回所需长度的负值）、`gb_load_state(gb,ptr,len)->i32`、`gb_regs(gb,out)`、`gb_take_serial(gb,out,cap)->i64`。每个入口都用 `catch_unwind` 包住，panic 转成错误码。**实例不是线程安全的**，由调用方加锁。
- 存档格式：magic `GBEMUST`、版本 u16、ROM 指纹（头部校验和 0x14D、全局校验和 0x14E–0x14F、标题 16 字节；core 零依赖，不计算 sha）。指纹不匹配时返回错误。
- `python/gbemu/backend.py: GbEmuEmulator(Emulator)`：
  - `__init__()` 无参数，调用 `super().__init__()`；`self._lock = threading.Lock()`，**所有原生调用都在锁内执行**。
  - `press(button, frames)`：校验按键名（未知按键抛 `ValueError`）→ 置位 → `tick(frames)` → 清位。
  - `tick(frames)`：调用 `gb_run_frames`，并 `frame_count += frames`。
  - `get_screen()`：`Image.frombytes("L",(160,144),buf).convert("RGB")`。
  - `read_*`：通过 peek 实现，地址按 `& 0xFFFF` 处理。
  - `close()` 和 `__del__` 都调用 `gb_free`（可重复调用）。
  - 动态库路径只认环境变量 `GBEMU_LIB`，未设置时报出明确的错误。
- 接入方式 1（默认）：`python -m gbemu.launch serve --rom … --port … --data-dir …`。启动器覆盖 `_EXT_MAP`，断言 `"pyboy" not in sys.modules`，然后调用 `pokemon_agent.cli.main()`。
- 接入方式 2（供评审方用自己的方式启动）：`patches/pokemon-agent-emulator.patch`，在 `emulator.py` 的 `_EXT_MAP` 中把 `.gb/.gbc` 指向 `gbemu.backend.GbEmuEmulator`。README 写明两种方式。

---

## 4. 测试与验收体系

### 4.1 gates（T05a 建最小可用版，T05b 补全，此后全程使用）

**分两步交付**，T17 只依赖 T05a：
- **T05a（最小可用，约 15 分钟）**：18 个 ROM 的清单、线程池与超时、串口判定（Blargg）、acid2 逐字节比较（输出差异像素数）、汇总行与 `ERROR` 行、`--only`、`--fast`、`reports/gates.json`。
- **T05b（补全，与 T17 并行，不阻塞）**：`--baseline`/`--update-baseline` 棘轮、判定逻辑的对抗单测、`reports/logs/` 与 PC 轨迹、acid2 差异图、Blargg 截图、`gates.md`。`--mooneye` 与计分规则属于 W4（T70）。

`cargo run --release -p gb-cli -- gates [--fast] [--only G1,G4] [--mooneye] [--baseline reports/baseline.json] [--update-baseline]`（测试 ROM 从 `$REFS` 读取，宝可梦 ROM 从 `$REPO/roms/pokered.gb` 读取；路径可用 `GBEMU_REFS` 覆盖）

- **输出约束**（对 LLM 友好）：stdout 最多 30 行。第一行是汇总，例如 `G1 11/11 G2 1/1 G3 3/4 G4 0/1 MOONEYE 41/93`；每个失败项一行 `ERROR <rom> <原因摘要>`。完整的串口文本、末尾 32 条 PC 轨迹、差异统计写到 `reports/logs/<rom>.log`。同时写出 `reports/gates.json` 和 `reports/gates.md`。
- `--fast`：只跑 individual ROM 和 acid2（合集跳过），用于开发中的快速循环和日常合并；全量只在打里程碑 tag 时跑。
- **唯一状态来源**：`reports/gates.json` 是门槛状态的唯一来源，PROGRESS.md 中的汇总行只能从它生成；任何代理都不得手工改写通过状态 [A2]。
- **判定逻辑先经对抗验证**：gates 自身要有单测，证明以下伪造结果都判为失败：串口只输出了 `Pass`；`Passed` 与 `Failed` 同时出现；超时前没有任何输出；acid2 帧只差 1 个像素；Mooneye 只有 5 个寄存器正确 [P1]。
- **棘轮**：`--baseline` 时，任何在基线中为通过、本次为失败的项目都打印 `REGRESSION <rom>`，并以退出码 2 退出。integrator 每次合并后执行带 `--baseline` 的 `gates --fast`（T05b 完成之前，手动对比合并前后的汇总行）；打里程碑 tag 时跑全量；通过后可以在本地 `--update-baseline`，但**只在里程碑 tag 时提交基线文件**，避免频繁提交。
- 每个 ROM 在独立线程中运行（线程数默认 4），panic 被捕获并记为失败。超时见 §2.4；Mooneye 30 秒；acid2 120 帧。
- acid2 与 `$REFS/acid2/reference-dmg.gray`（T01 由参考图转出的 23040 字节原始灰度数据，sha256 `cb26ef6174cd5b61662c2e74f2bdb2b0f8da0750234c75144c3a4d258fda8347`）逐字节比较；失败时写出 `reports/acid2-actual.png` 和 `reports/acid2-diff.png`（差异像素标红），并输出差异像素数与包围盒；对照 `$REFS/acid2-src/README.md` 的 Failure Examples 定位。
- Blargg 每个 ROM 结束时写出 `reports/blargg/<rom>.png`。
- ROM 来源：`tools/fetch_refs.sh`（§2.1）已完成下载与校验；gates 启动时若找不到 `$REFS/blargg`，打印 `ERROR refs missing, run tools/fetch_refs.sh` 并退出。

### 4.2 单元测试（`cargo test`，不依赖 ROM）

- `tests/opcode_table.rs`：读取 `tests/data/Opcodes.json`，对除 11 个非法 opcode（`D3 DB DD E3 E4 EB EC ED F4 FC FD`）和 `0xCB` 前缀本身之外的 500 个 opcode（无前缀 244 个、CB 前缀 256 个），在平铺 RAM 的假总线上执行，断言指令长度、M-cycle 数（条件指令两种分支各测一次）、标志位约束（`-` 不变，`0/1` 被强制）。标志位断言在 T11 完成前标记 `#[ignore]`。
- `crates/core/tests/sst.rs`（Q1 已允许，T12）：读取 `$REFS/sst/v1/*.json`（`$REFS` 缺失时 SKIP）。每个文件 1000 组用例：按 `initial` 设置寄存器、IME 与 RAM（平铺 64 KiB 的假 `CpuBus`），执行 1 条指令，比较 `final` 中的寄存器与 RAM，以及 `cycles` 中**逐 M-cycle 的总线访问序列**（`[地址, 数据, "r-m"/"-wm"]`，`null` 表示内部周期）。支持 `SST_ONLY=<opcode>` 只跑一个文件，失败时打印首个不一致的用例与字段。它直接验证 §3.3 的内部周期位置和访问顺序，是 G1 和 G3 最快的定位手段。`HALT`（0x76）、`STOP`（0x10）等与外设交互的指令如果和测试约定不一致，可以把它们列为例外，记入 DECISIONS.md。
- `tests/alu.rs`：DAA（加法和减法之后各取若干值）、ADC/SBC 带进位的半进位、`ADD SP,e` 与 `LD HL,SP+e` 的 H/C 按**低字节无符号相加**计算、`POP AF` 清除 F 低 4 位、`BIT` 置 H=1。期望值按文档手算。
- 外设单测（定时器下降沿、STAT 上升沿、窗口行计数器、10 个精灵上限、优先级、各 MBC 的 bank 映射）一律**直接断言像素数组或寄存器值**；PNG 只由 cli 导出，供调试查看。
- 需要 ROM 的集成测试放在 `crates/cli/tests/`；`$REFS` 或 `roms/` 中缺少文件时打印 `SKIP` 并通过，因此从干净 checkout 运行 `cargo test` 永远可执行。
- 存档往返：`save → 跑 600 帧 → 对画面与 WRAM 做 hash` 必须等于 `load → 跑 600 帧 → hash`。

### 4.3 性能

`gb-cli bench <rom> --frames 3600` 报告 fps。**G5 之前不做优化**：T61 时测一次；只有低于 120 fps（实玩时 `tick` 会拖慢驾驶）才必须在 G5 之前处理。≥ 600 fps（10 倍实时）的目标属于 W4（T55）；只做不改变行为的优化，做完跑全量 gates。

### 4.4 实玩（G5）

- `tools/playtest.py --minutes 30 --driver {script,agent} --port 8765`：
  1. 新建 `reports/playtest/<时间戳>/`，用 `--data-dir <该目录>/data` 启动服务器（固定 `GBEMU_LIB`，使用 `rc-gates` 冻结构建）；
  2. **只由 playtest.py 调用一次 `/games/new`**；
  3. 每 10 秒采样 `/health`、`/state`（map_id、x、y、对话、战斗、队伍）、`/screenshot`；
  4. 硬性判定：进程全程存活，`/health` 始终有响应；`/state` 每次返回 200 且字段合法；每次 `/action` 之后帧数都增加（服务端是被动推进的，驾驶思考期间帧数不动属于正常现象，**不能**用"帧数持续增长"判定）；**全程至少有一次地图或坐标变化**；不存在持续超过 60 秒的单色画面。
  5. 告警（不判失败）：10 分钟内坐标无变化；连续 2 分钟没有任何 `/action`；出现单色帧；驾驶循环出现失败轮次。
  6. 结束后生成 `summary.md`（时间线、坐标轨迹、告警）和 `contact-sheet.png`（每分钟 1 张，拼成网格），供人工或 LLM 抽查花屏和错位。
- `--driver script`：`scripted_driver.py`（无 LLM）跑开场（标题 → NEW GAME → 跳过对话 → 选择预设名字），之后随机行走并定期按 A/B。用于快速、可重复的回归。
- **确定性回放基线**（W4，T63，G5 之后再做）（把 30 分钟的整体性验收切成可以快速回归的片段）[A3][P4]：本模拟器在相同 ROM 与相同按键序列下完全确定（rDIV 只造成**跨**模拟器的差异）。
  - `scripted_driver.py --record` 用固定的按键脚本跑开场约 3000 帧，在第 300、600、……帧记录画面哈希与关键 WRAM（地图、坐标、对话状态），写入 `reports/pokered-replay.json` 并提交。
  - `--verify` 重放并比较，作为 gates 的附加项 `POKERED-REPLAY`（宝可梦 ROM 缺失时为 SKIP）。
  - 在两个里程碑之间，哈希变化只打印 `WARN POKERED-REPLAY changed`，不阻塞合并；到里程碑时由 integrator 查看对应帧的截图，确认是修复而不是回退之后重新录制并提交。
- **调试切片**：开发期允许用存档状态加按键脚本，把问题场景（进入战斗、打开菜单、切换地图等）切成独立片段，交给多个 scout 并行定位。正式验收仍从开机开始，不加载任何存档。
- `--driver agent`（正式验收）：由 `tools/drive_loop.sh` 驱动。单个 Claude Code 会话很可能在几分钟后就结束回合，截图也会很快撑满上下文，所以采用**每轮一个新会话**的外层循环：
  - 每一轮：`claude -p --model global.anthropic.claude-sonnet-5-5 --effort high --allowedTools "Bash(curl *localhost*)" "Read" "$(cat tools/driver_prompt.md)"`。会话先读 `/objectives`（跨轮次的记忆）、`/state`、`/map/ascii`，再把 `/screenshot` 下载到本地后用 Read 查看；执行 5–10 个 `/action`，把下一步目标写回 `/objectives`，然后退出。
  - 循环直到满 30 分钟；每轮的会话 ID 记录到 `reports/playtest/<run>/driver-sessions.txt`，以便导出（R4）。
  - 某一轮失败（非 0 退出或超过 3 分钟）就立即开下一轮；连续 3 轮失败时，用 `scripted_driver.py` 驾驶 60 秒，保持游戏推进，并在 summary 中记录。scripted 驾驶累计超过 3 分钟时，本次实玩视为无效，需要重跑。
  - `tools/driver_prompt.md` 必须包含："服务器已在 `localhost:<port>` 运行；禁止启动或停止服务器；禁止调用 `/games/new`、`/games/{id}/load`、`/load`；只用 `/objectives`、`/state`、`/screenshot*`、`/map/ascii`、`/action`。"

### 4.5 隐藏集抽测（防"特性孤立"）[P3]

SpecBench 发现，规模超过 25K LOC 的从零构建任务（其中就有一个 C 语言 GB 模拟器），可见测试与隐藏测试的通过率差距最高可达 100pp。最常见的形式是各模块单独能过测试，组合起来就失效。对策：

- G1–G4 是"可见集"，开发时可以针对它们调试。**G5 之前由宝可梦充当"隐藏集"**：T61 的 5 分钟冒烟与 T62 的 30 分钟实玩不针对单个测试调试；如果必过项都通过、宝可梦开场却坏了，就视为出现了针对性实现，必须先查明原因再继续推进。
- **G5 之后（W4）用 Mooneye 复核**：先不做任何修改，原样跑一次 `gates --mooneye`，在 REPORT 中同时记录必过项通过率和 Mooneye 通过率，作为隐藏集差距 Δ 的证据；reviewer 在 T80 中复核这条记录。之后才开始针对 Mooneye 调试（T71）。

### 4.6 REPORT.md

记录环境；各门槛结果（链接 `reports/gates.md`）；Mooneye 计分与纳入清单；实玩 summary 和 contact sheet；性能；每个代理使用的模型与推理强度；已知问题；合规自查结论。

---

## 5. 调试手册

**通用流程**：CPU 相关的失败先跑 `cargo test --test sst`（可以用 `SST_ONLY` 缩小到可疑的 opcode），通常能直接定位到具体指令和周期；然后再读 gates 的 `ERROR` 行 → 打开对应日志 → 定位到 Pan Docs 的具体章节 → 写最小复现单测 → 修复 → `gates --fast` → 合并前跑全量（带基线）。**同一问题 3 轮假设都失败**：在 `PROGRESS.md` 的 Blockers 记录现象和已排除的假设，推理强度升一级，或者派一个 `scout` 并行调查；必过项不得放弃，加分项可以放弃。

| 现象 | 优先排查 |
| --- | --- |
| 串口无输出 | SC 写入处理；PC 跑飞（看日志中的 PC 轨迹）；`$D000` 是否可执行；开机状态 |
| 01-special | DAA；POP AF 低 4 位；JR 符号扩展 |
| 02-interrupts | EI 延迟；IF 高位；定时器中断；HALT 唤醒 |
| 03-op sp,hl | ADD SP,e 与 LD HL,SP+e 的 H/C |
| 07-jr,jp,call,ret,rst | RETI 置 IME；RST 向量；条件判断 |
| 09 / 11 | ADC/SBC 半进位；(HL) 变体；CP 标志 |
| instr_timing 全错或报 #255 | 先查定时器，而不是指令周期 |
| instr_timing 个别 opcode | 按输出 `op:实测-正确` 对照 Opcodes.json 和 §3.3 |
| mem_timing | 访问发生在第几个 M-cycle：`INC/DEC (HL)` 读在第 2 个、写在第 3 个；CB 前缀的 `BIT b,(HL)` 读在第 3 个；`RES/SET b,(HL)` 和 `RLC (HL)` 等读在第 3 个、写在第 4 个；`LD (a16),A` 写在第 4 个、`LD A,(a16)` 读在第 4 个；§3.3 的全局顺序 |
| Mooneye 卡住超时 | 串口传输是否完成（SC.7 是否被清零）；`LD B,B` 检测 |
| acid2 | 差异图对照 README Failure Examples：10 个精灵上限、优先级、8×16（含整体 Y 翻转、忽略 tile bit0）、翻转、窗口 Y 条件和行计数器、有符号 tile、`LCDC.0`、`LCDC.1`、OBJ-to-BG priority |
| 宝可梦白屏或卡在开场 | LCD 关闭期间的帧推进；STAT/LY；OAM DMA；MBC3 bank 映射；外部时钟串口 |
| 宝可梦画面上下撕裂 | 是否只读前缓冲（§3.5 双缓冲） |
| 宝可梦花屏 | 窗口；在 mode 2 改过的寄存器是否在进入 mode 3 时才生效 |
| 实玩时偶发崩溃 | Python 后端是否加锁（§3.8）；FFI 是否用 catch_unwind 包住 |

---

## 6. 执行方式

### 6.1 节奏

- 里程碑：M0 工具与冻结 → M1 CPU（G1）→ M2 时序（G2、G3）→ M3 PPU（G4）→ M4 卡带与接入 → M5 实玩（G5）→ M6 Mooneye → M7 交付。M6、M7 属于 G5 之后的收尾阶段。依赖关系见 §8。
- 每个任务都按"实现 → `cargo test` → `gates --fast` → 提交"闭环执行；提交信息格式：`T13: bus memory map (gates G1 0/11)`。
- **只在里程碑结束时**更新 `PROGRESS.md`（任务状态表、门槛汇总行、Blockers、下一步）；单个任务完成只提交代码，提交信息里写明 gates 汇总行即可。出现 Blocker 时立即记一笔（这是上下文重置后恢复所必需的）。

### 6.2 代理编制（写代码的子代理 3 个：core、ppu、platform；主会话不承担大段编码）

| 代理 | 职责 | 独占路径 | 模型 / 推理强度（§6.6） |
| --- | --- | --- | --- |
| `integrator`（主会话） | 环境与接口冻结（T00、T02）；派发与监督子代理；所有合并、基线与里程碑 tag；PROGRESS/DECISIONS；攻关 Blocker（必要时亲自修改任何文件，并在 DECISIONS 中记录）；T62 监督与 T91 交付 | `crates/core/src/lib.rs`、`Cargo.toml`、`.claude/`、`PROGRESS.md`、`DECISIONS.md`、`reports/baseline.json`；T02 时写 `machine.rs` 骨架，之后交给 core | Opus 5.5 / **max**（fast 模式可用才开） |
| `core` | CPU（解码、ALU、全部指令）、总线与内存映射、开机状态、中断分派与 HALT（T14）、定时器（T15）、串口（T16）、Joypad（T51）、IF/IE 寄存器、`Machine` 的整机连线与存档汇总；G1–G3 | `crates/core/src/{machine,cpu,decode,alu,bus,state,interrupts,timer,serial,joypad}.rs`、`crates/core/tests/{opcode_table,alu,sst,cpu*,bus*,state*,timer*,serial*,joypad*,interrupts*}.rs` | Opus 5.5 / xhigh |
| `ppu` | PPU、OAM 的数据结构与 DMA 状态；G4 | `crates/core/src/{ppu,dma}.rs`、`crates/core/tests/ppu*.rs` | Opus 5.5 / xhigh |
| `platform` | 卡带、cli 与 gates、FFI、Python、启动器、补丁、playtest、驾驶脚本、bench、存档往返测试（T52） | `crates/core/src/cart/`、`crates/core/tests/cart*.rs`、`crates/cli/`、`crates/ffi/`、`python/`、`patches/`、`tools/` | Opus 5.5 / high（写 gates 判定逻辑时临时用 xhigh） |
| `scout`（可同时开多个，只读） | 针对单个失败项调查：读测试说明和汇编源码，用 `gb-cli run` 复现，输出"现象 → 违反的 Pan Docs 条目 → 建议改动（文件和函数）"，写到 `reports/scout/<rom>.md` | 只写 `reports/scout/` | Opus 5.5 / high |
| `reviewer`（新开上下文，持怀疑态度；仅 W4） | G5 之后，从干净 checkout 复跑 `cargo test` 和全量 gates，逐条核对 PROGRESS.md 的声明；T90 合规自查。默认假设"声明未经证实"，只有亲自复现的结果才算 | 只写 `reports/review.md` | Opus 5.5 / high |
| `driver`（T60 的 3 分钟验证与 T62） | 30 分钟正式实玩时驾驶游戏，只通过 HTTP 操作（§4.4 的约束） | 不改代码 | Sonnet 5.5 / high |
| `clerk`（机械事务） | 资料下载与校验、gates 日志摘要、sessions 导出与 INDEX、REPORT 拼接 | 只写 `reports/`、`sessions/` | Haiku 5.5 / medium |

同时写代码的只有 core、ppu、platform 三个子代理；关键路径在 core 上，由 xhigh 承担大段编码，比让 max 的主会话亲自写更快（§6.6）。定时器、串口、Joypad 合计约 300 行，且与 instr_timing/mem_timing 紧耦合，由 core 顺手完成（约 15–20 分钟），比单独开一个通道再合并、联调更快。**Snapshot 由各模块的所有者随模块一起实现**（每个外设十几行），不单独排期。只读代理不占写代码的名额，但同时运行的代理总数建议不超过 5 个（15 核机器）。

### 6.3 波次（任务编号见 §8）

| 波次 | 开始条件 | 并行内容 |
| --- | --- | --- |
| W0 | 开工 | integrator：T00（第一步就在后台执行 fetch_refs 与 Python 依赖安装）→ T02（冻结，约 20 分钟）；T01 在 fetch_refs 完成后穿插执行 |
| W1 | T02 提交 | core：T10 → T12 → T11 → T13 → T14 → T15 → T16 → T51；ppu：T40–T42（只写单测）；platform：T05a（最先做）→ T50（MBC1/MBC3）→ T05b → T53a |
| W2 | core 完成 T14–T16 | core：T17 → T30；ppu：T17 一通过就做 T43；platform：T52 → T53b → T54 → T60 → T61（5 分钟冒烟） |
| W3 | G1–G4 全绿 | 打 tag `rc-gates`，在冻结构建上跑 **T62：30 分钟正式实玩**（默认只跑这一次；例外见 T61b）；通过后打 `v1.0-gates`，**G1–G5 完成** |
| W4（收尾阶段） | `v1.0-gates` | 先原样跑一次 `gates --mooneye` 留证据（§4.5）；T50b、T55、T63、T70 → T71（scout 扇出，锁机制见 §6.4）；reviewer：T80 与 T90；integrator：T91 |

### 6.4 隔离、合并与锁

- **worktree 由 integrator 手动创建**（新仓库没有 remote，不使用子代理的自动 worktree）：`git worktree add -b lane/ppu /Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/ppu HEAD`；core、platform 同理。子代理只在自己的 worktree 中工作，设置 `CARGO_TARGET_DIR=<worktree>/target` 和 `CARGO_BUILD_JOBS=4`（3 个写代码的通道共约 12 个编译线程，适配 15 核机器）。
- **子代理的工作目录不会自动切换到 worktree**：简报中必须给出 worktree 的绝对路径，要求子代理的每条命令都以 `cd <worktree绝对路径> &&` 开头，所有文件读写都使用绝对路径；回报时附上 `git -C <worktree> status --short`，证明没有误改主仓库。
- **权限与联网**：后台子代理遇到权限确认会卡住，因此 T00 在 `$REPO/.claude/settings.json` 中预先配置 `permissions.allow`（`Bash(cargo *)`、`Bash(git *)`、`Bash(cd *)`、`Bash(ls *)`、`Bash(cat *)`、`Bash(grep *)`、`Bash(find *)`、`Bash(python *)`、`Bash(.venv/bin/*)`、`Bash(tools/*)`、`Bash(curl *localhost*)` 等）以及 `permissions.deny`（`WebSearch`、`WebFetch`）。子代理定义的 frontmatter 用 `tools:` 白名单（`Read, Edit, Write, Bash, Grep, Glob`），不含任何联网工具，从工具层面防止误入模拟器源码（R1）。
- **合并**：integrator 执行 `git merge --no-ff lane/<名>` → `cargo test` → 带基线的 `gates --fast`（打 tag 时跑全量）→ 通过后在本地更新基线，并在里程碑 tag 时提交；出现回归就 `git revert -m 1 <merge>`，再把问题退回给该通道。lane 分支定期 `git merge main` 同步（不 rebase）。
- **共享文件**（`lib.rs`、`Cargo.toml`）只由 integrator 修改；`machine.rs` 由 integrator 在 T02 写好骨架后归 core 所有。T02 就建好所有模块的空壳和 `mod` 声明。
- **任务锁（仅 W4 的 Mooneye 使用）**：开始处理某个 ROM 前，在 main 上提交 `locks/<rom>.lock`（内容为代理名和时间），完成后删除；已有锁的 ROM 不得领取。
- 子代理完成时必须回报：改动文件、执行过的命令及结果（直接粘贴 gates 汇总行）、遗留问题。**没有附命令结果的"完成"一律视为未完成。**

### 6.5 子代理简报模板（integrator 派发时照填）

```
任务：<§8 中的编号与标题>；参考 §<章节>；本地资料 $REFS/<路径>
工作目录：<worktree 绝对路径>（只在这里修改）；独占路径：<路径>
接口：§3.2（已冻结），不得修改签名；需要修改时停下来回报
完成标准：<§8 中的命令与期望输出>
红线：§1.2 R1–R6（不得查看任何模拟器源码，不得联网搜索，不得特判测试 ROM，不得改写 git 历史）
路径：每条命令以 `cd <worktree绝对路径> &&` 开头；所有文件读写用绝对路径；回报附 `git -C <worktree> status --short`
回报：改动文件、命令与结果、遗留问题；不做与本任务无关的改动
```

### 6.6 模型与推理强度（只看质量与速度，不考虑成本；依据官方数据，2026-10-29 核对）[M1]

**分配**

| 代理 | 模型 | 推理强度 | 理由 |
| --- | --- | --- | --- |
| `integrator`（主会话） | `global.anthropic.claude-opus-5-5`（fast 模式可用才开） | **max** | 负责架构冻结、合并与基线判断、攻关 Blocker：数量少，但每次判断的代价最高。Opus 5.5 的多数基准以 max 为最高分（CursorBench 57.8%、GDPval-AA 1846、AutomationBench 40.0%）。**不承担大段编码**：在 Opus 5.5 上，max 比 xhigh 多用约 50% 的 token，编码分数并不更高（Terminal-Bench 64.8% 对 66.4%），大段编码交给 xhigh 的 `core` 更快。fast 模式是同一个模型，输出最快提升 2.5 倍；官方只说明 Claude Code 与 Claude Platform 提供该模式，本机走 Bedrock 网关，必须在 T00 实测 |
| `core`、`ppu` | `global.anthropic.claude-opus-5-5` | xhigh | 关键路径上语义最易错的模块。xhigh 是 Opus 5.5 在 Terminal-Bench 4.0（命令行智能体编码，与本题的工作方式最接近）上的最高点：66.4%；max 为 64.8%，而且 token 用量多约 50%，既不更好也更慢 |
| `platform`（卡带、cli 与 gates、FFI、Python、实玩工具） | `global.anthropic.claude-opus-5-5` | high | 规格清晰、错误能被测试立即暴露。Opus 5.5 high 在 FrontierCode、CursorBench 上与 xhigh 持平（54.0% 对 51.4%，56.0% 对 56.0%），token 用量约为 xhigh 的一半，速度更快；编写 gates 判定逻辑时临时调到 xhigh |
| `scout`（只读诊断，可多开） | `global.anthropic.claude-opus-5-5` | high | 诊断质量直接决定主会话的修复速度。high 在 Terminal-Bench 上比 medium 高 6.6 分（64.2% 对 57.6%），token 用量只多约三分之一 |
| `reviewer`（新开上下文） | `global.anthropic.claude-opus-5-5` | high | 评审依赖判断力；官方说明 Opus 5.5 在开放性、需要持续判断的工作上明显强于 Sonnet 5.5。评审不能弱于被评审的代理 |
| `driver`（30 分钟实玩驾驶，T61b/T62） | `global.anthropic.claude-sonnet-5-5` | high | 官方称 Sonnet 5.5 是"第一个只凭截图就通关宝可梦红的 Sonnet 模型"，并定位为"速度与智力的最佳组合"。实玩的墙钟时间固定为 30 分钟，驾驶越快，游戏推进得越多；驾驶不参与模拟器的编写 |
| `clerk`（机械事务：资料下载与校验、gates 日志摘要、sessions 导出与 INDEX、报告拼接） | `global.anthropic.claude-haiku-5-5` | medium | 官方最快的模型，定位为"Opus 5.5 与 Sonnet 5.5 编码工作的子代理"。这些任务没有推理难度，追求速度；不写 core 代码 |

**为什么编码子代理不用 Sonnet 5.5（即使不计成本）**

把每题成本除以单价（Opus 5.5 为 Sonnet 5.5 的 2 倍），可以近似比较两者每题的 token 用量，也就是生成耗时：

| Terminal-Bench 4.0 | 得分 | 每题 token 用量（相对值，Sonnet 单价为 1） |
| --- | --- | --- |
| Opus 5.5 high | 64.2% | 1.94 |
| Opus 5.5 xhigh | **66.4%** | 3.68 |
| Sonnet 5.5 high | 43.0% | 1.94 |
| Sonnet 5.5 xhigh | 61.5% | 5.30 |
| Sonnet 5.5 max | 70.6% | 12.54 |

- 同样的 token 用量下，Opus 5.5 high 比 Sonnet 5.5 high 高 21 分；Opus 5.5 xhigh 的 token 用量少于 Sonnet 5.5 xhigh，分数反而更高。
- Sonnet 5.5 只有在 max 时才在 Terminal-Bench 上领先（70.6%），代价是 token 用量约为 Opus 5.5 xhigh 的 3.4 倍。在另外两个编码基准上，Sonnet 5.5 max 都不如 Opus 5.5（FrontierCode 46.2% 对 54.4%，CursorBench 55.5% 对 57.8%）。
- Sonnet 的单 token 输出速度可能更快，但官方没有公布两者的直接对比；即使快一些，也抵不上上面的质量差距和 token 用量差距。另外，子代理写出的隐蔽 bug 最终要由主会话来查，这是整个流程里最慢的环节。
- Fable 5.1 在这三个编码基准上都低于 Opus 5.5，也不使用。

**官方数据：推理强度与得分、每题成本**（来源：Anthropic 发布页 claude-opus-5-5、claude-sonnet-5-5、claude-haiku-5-5 中的图表数据；成本仅用来推算 token 用量）

| 基准（编码类） | 模型 | low | medium | high | xhigh | max |
| --- | --- | --- | --- | --- | --- | --- |
| Terminal-Bench 4.0 | Opus 5.5 | 38.5% / $1.29 | 57.6% / $2.94 | 64.2% / $3.88 | **66.4%** / $7.35 | 64.8% / $11.24 |
| | Sonnet 5.5 | 20.0% / $0.76 | 28.8% / $0.83 | 43.0% / $1.94 | 61.5% / $5.30 | **70.6%** / $12.54 |
| | Haiku 5.5 | 12.7% / $0.42 | 20.3% / $0.68 | 24.8% / $1.04 | 31.5% / $1.75 | 39.2% / $2.64 |
| FrontierCode 1.1 | Opus 5.5 | 47.3% / $0.40 | **54.6%** / $0.80 | 54.0% / $1.09 | 51.4% / $2.25 | 54.4% / $6.19 |
| | Sonnet 5.5 | 29.3% / $0.19 | 36.5% / $0.24 | 49.4% / $0.42 | **52.1%** / $1.59 | 46.2% / $20.78 |
| CursorBench 4.0 | Opus 5.5 | 43.7% / $1.18 | 52.5% / $2.90 | 56.0% / $3.97 | 56.0% / $6.99 | **57.8%** / $13.43 |
| | Sonnet 5.5 | 35.8% / $0.50 | 39.2% / $0.70 | 47.8% / $1.67 | 53.1% / $3.88 | **55.5%** / $9.67 |

读法：Opus 5.5 在 high 之后边际收益很小（Terminal-Bench 的标准误差为 ±2.6 分），而 token 用量和耗时会增加 2–3 倍。因此子代理用 high 或 xhigh，**不用 max**：max 既不更准，也更慢。主会话按既定决定使用 max，但只做数量少、每次都很关键的判断，大段编码由 xhigh 的 `core` 承担。官方另有两点说明：Opus 5.5 的默认推理强度是 medium（比 Opus 5 低一级）；adaptive thinking 始终开启，推理强度是控制推理深度的主要手段，在 high 及以上要给足输出上限。

**本机网关的模型名（2026-10-29 实测）**

本机的 Claude Code 通过自定义网关（`ANTHROPIC_BASE_URL`，new-api 实现）接入 Bedrock：
- 只接受带 `global.anthropic.` 前缀的完整 ID；`global.anthropic.claude-opus-5-5`、`global.anthropic.claude-sonnet-5-5`、`global.anthropic.claude-haiku-5-5`、`global.anthropic.claude-fable-5-1` 都返回 200。
- 不带前缀的 `claude-opus-5-5` 等一律返回 403（"This token has no access to model"）。
- `output_config.effort` 的 medium/high/xhigh/max 都能透传，并且确实生效：同一提示在 xhigh/max 下的输出 token 明显更多。
- **`~/.claude/settings.json` 把 `opus`、`sonnet`、`haiku`、`fable` 四个别名和 `CLAUDE_CODE_SUBAGENT_MODEL` 全部映射到了 `global.anthropic.claude-opus-5-5`**。所以写别名（如 `model: sonnet`）实际得到的是 Opus 5.5；所有地方都必须写完整 ID。

**设置方式（T00 完成）**
- 探测：T00 运行 `tools/probe_models.sh`（见 T00），对 4 个完整 ID 各发一次 `max_tokens=16` 的请求，结果写入 `reports/model-probe.md`。以探测结果为准：如果网关换了，或某个 ID 不可用，就用可用的最接近的模型替代，并记入 DECISIONS.md。
- 主会话：在 `$REPO` 中执行 `claude --model global.anthropic.claude-opus-5-5 --effort max`；进入后执行 `/fast`。如果提示不支持（Bedrock 上很可能如此），就不开，记入 DECISIONS.md 后继续。
- 子代理：在 `$REPO/.claude/agents/<名>.md` 的 frontmatter 中写明完整的 `model` ID、`effort` 和 `tools` 白名单（例如 `model: global.anthropic.claude-opus-5-5`、`effort: xhigh`、`tools: Read, Edit, Write, Bash, Grep, Glob`），派发时用 Agent 工具指定 `subagent_type: <名>`。
- 子代理配置的验证**不放在关键路径上**：T00 只用 `claude agents` 确认定义已被识别；派发时始终显式传入 effort 参数（frontmatter 同时写明，双保险）。W4 导出会话时，再在 `~/.claude/projects/-Users-skyseek-Desktop-project-open-research-platform-GB-emulator*/` 的会话记录中用 `grep -o '"model":"[^"]*"'` 核对每个子代理实际使用的模型，如实写进 REPORT（模型不符只影响速度，不影响正确性）。驾驶会话由 `claude -p --model <完整 ID>` 直接启动，模型在 T60 中顺带确认。
- 实玩驾驶不经过子代理机制，由 `tools/drive_loop.sh` 直接以 `claude -p --model global.anthropic.claude-sonnet-5-5 --effort high` 启动（§4.4）。
- 升级规则：同一问题 2 轮假设失败，升一级推理（最高升到 xhigh；integrator 已是 max，改为换思路或派 scout 并行调查）。子代理连续 2 次回报未达到完成标准时，由 integrator 收回该任务。

### 6.7 稳定性规则

不跨任务做大重构；每个修复都附带回归测试，或在 gates 中有对应的 ROM；只在 release 构建上跑 gates 和实玩；实验在分支上进行，合并用 `--no-ff`。

### 6.8 会话工作流（开场、执行、收尾）[A2][A1]

**开场**（每次新会话、上下文压缩或重置之后，无一例外）：
1. `pwd` 确认所在的 worktree；`git status` 确认没有遗留的未提交改动。
2. 读 `PROGRESS.md`（当前任务、Blockers、下一步）和 `git log --oneline -20`。
3. 跑冒烟测试：`cargo test` 加 `gates --fast --baseline reports/baseline.json`。**冒烟失败时先修复，再做任何新工作**；在损坏的状态上开发新功能，只会让问题更难定位。
4. 从 §8 中选一个依赖已满足、优先级最高的任务，在 PROGRESS.md 中标为 `doing`。

**执行**：
- 一次只做一个任务。不要试图一口气写完整个模拟器，否则上下文耗尽时会留下没有记录的半成品。
- 先做全局规划，再写代码：T02 冻结接口之前，先写出模块关系、时钟模型和数据归属，并记入 DECISIONS.md。
- 只有完成标准中的命令输出达标，才能把任务标为 `done`。"单测通过"不能代替端到端验证；涉及画面的改动，必须查看 gates 产出的 PNG 或 contact sheet。

**收尾**（每个任务完成时，或会话即将结束时）：
- 提交代码（提交信息中写明 gates 汇总行）；有新决定时更新 DECISIONS.md；PROGRESS.md 只在里程碑或出现 Blocker 时更新（§6.1）。
- 工作区必须处于可以直接合并的干净状态：没有半成品，没有调试残留。

**里程碑 tag**：G1、G2/G3、G4、G5 各自首次全绿时分别打 tag（`m1-g1`、`m2-timing`、`m3-acid2`、`rc-gates`），保证任何时候都有一个可交付的最好版本，避免"超时导致零交付"。从零建仓基准中有 51 次运行就是因为超时或未提交而得了零分 [P1]。

---

## 7. 交付清单

1. git 仓库（完整历史）：`v1.0-gates` tag 打在**通过 T62 实玩的那个提交**（即 `rc-gates` 所指的提交，或修复后重跑通过的提交）上。
2. `REPORT.md`、`reports/gates.md`、`reports/baseline.json`、`reports/playtest/<run>/summary.md` 与 contact sheet、`reports/review.md`。
3. `sessions/`：导出所有相关的 Claude Code 项目目录 `~/.claude/projects/-Users-skyseek-Desktop-project-open-research-platform-GB-emulator*`（覆盖主仓库、各 lane worktree 和实玩会话；子代理记录在各会话的子目录中）。附 `sessions/INDEX.md`（文件 → 时间范围 → 代理与任务）。导出前扫描密钥（`sk-`、`ANTHROPIC_API_KEY` 等），命中时只打码、不删除，并在 INDEX 中注明。
4. `README.md`：三条命令即可复现（构建、gates、实玩），并写明两种接入方式（§3.8）。
5. `git ls-files | grep -iE '\.(gb|gbc|sav|state)$'` 的结果必须为空。

---

## 8. 任务清单

> 每个任务的"完成标准"都可以用命令验证；达标就提交，并更新 `PROGRESS.md`。
> 列说明：**依赖** = 必须先完成的任务；**代理** = §6.2 的角色；**波次** = §6.3；**时限** = 建议上限，超过后按 §5 记录 Blocker 并升级（必过项不得放弃）。
> 约定：`$REPO=/Users/skyseek/Desktop/project/open/research-platform/GB-emulator`，`$REFS=$REPO/refs`（绝对路径）；`gates` 指 `cargo run --release -p gb-cli -- gates`；所有 Python 命令都在 `$REPO/.venv` 中执行，并设置 `GBEMU_LIB=<对应 target>/release/libgb_ffi.dylib`。
> 状态取值：`todo` / `doing` / `done` / `blocked` / `skipped`。

### 状态总览（复制到 PROGRESS.md 维护）

| ID | 任务 | 依赖 | 代理 | 波次 | 时限 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| T00 | 环境、仓库、模型探测、权限与开工前提 | — | integrator | W0 | 20m | todo |
| T01 | 资料就绪与参考图转换 | T00 | integrator（可交给 clerk） | W0 | 10m | todo |
| T02 | 冻结跨通道接口与模块空壳 | T00 | integrator | W0 | 20m | todo |
| T05a | gates 最小可用版与 `gb-cli run` | T01,T02 | platform | W1 | 15m | todo |
| T05b | gates 补全（棘轮、对抗单测、日志、差异图、截图） | T05a | platform | W1 | 35m | todo |
| T10 | CPU 解码框架与 opcode 表测试 | T02 | core | W1 | 45m | todo |
| T11 | ALU 纯函数与全部指令（含周期序列） | T10 | core | W1 | 90m | todo |
| T12 | SingleStepTests 接入（Q1 已允许；从 T10 起持续使用） | T10 | core | W1 | 30m | todo |
| T13 | 总线、内存映射、开机状态 | T02 | core | W1 | 40m | todo |
| T14 | 中断、EI/DI/RETI、HALT 与 HALT bug | T11,T13 | core | W1 | 45m | todo |
| T15 | 定时器 | T13 | core | W1 | 15m | todo |
| T16 | 串口 | T13 | core | W1 | 10m | todo |
| T51 | Joypad | T13 | core | W1 | 10m | todo |
| T17 | **G1：cpu_instrs** | T05a,T14,T15,T16,T50 | core（integrator 合并） | W2 | 120m | todo |
| T30 | **G2/G3：instr_timing、mem_timing** | T17 | core（integrator 合并） | W2 | 90m | todo |
| T40 | PPU 时序骨架（单测） | T02 | ppu | W1 | 45m | todo |
| T41 | BG/Window 渲染（单测） | T40 | ppu | W1 | 60m | todo |
| T42 | 精灵与 OAM DMA 状态（单测） | T41 | ppu | W1 | 60m | todo |
| T43 | **G4：dmg-acid2** | T17,T42 | ppu | W2 | 90m | todo |
| T50 | ROM only、MBC1、MBC3（单测） | T02 | platform | W1 | 30m | todo |
| T52 | 存档往返测试（实现随各模块完成） | T17,T42,T50 | platform | W2 | 20m | todo |
| T53a | FFI 骨架与 Python 绑定（对接冻结接口） | T02 | platform | W1 | 45m | todo |
| T53b | Python 后端整机验证 | T52,T53a | platform | W2 | 30m | todo |
| T54 | pokemon-agent 启动器与补丁 | T53b | platform | W2 | 30m | todo |
| T60 | 实玩监控 `playtest.py` 与驾驶循环 `drive_loop.sh` | T54 | platform | W2 | 60m | todo |
| T61 | 回归驾驶脚本与 5 分钟冒烟（含一次 fps 测量） | T60,T43 | platform | W2 | 30m | todo |
| T61b | 提前跑 30 分钟实玩（**条件触发**，默认不做） | T61 | integrator 监督，driver 驾驶 | W2 | 40m | todo |
| T62 | **G5：30 分钟正式实玩** | T30,T43,T61 | integrator | W3 | 40m | todo |
| T50b | MBC2、MBC5 | T50 | platform | W4 | 30m | todo |
| T55 | 性能 ≥ 600 fps | T62 | platform（core 配合） | W4 | 45m | todo |
| T63 | 宝可梦确定性回放基线 `POKERED-REPLAY` | T62 | platform | W4 | 30m | todo |
| T70 | Mooneye 范围与计分 | T05b | platform | W4 | 30m | todo |
| T71 | Mooneye 迭代（加分） | T62,T70 | scout ×N + core/ppu | W4 | 按停止条件 | todo |
| T80 | 评审（一次） | T62 | reviewer | W4 | 30m | todo |
| T90 | 合规自查 | T62 | reviewer | W4 | 30m | todo |
| T91 | 交付 | T80,T90 | integrator | W4 | 45m | todo |

**关键路径**：T00 → T02 → T10 → T11 → T14 → T17 → T30 → T62（打 `v1.0-gates`，G1–G5 完成）。T13、T15、T16、T51 由 core 在关键路径上顺序完成（合计约 75 分钟，其中 T13 可与 T11 交错）；T05a、T50 由 platform 最先完成，必须在 T17 之前就绪；T43 与 T30 并行；T52 → T53b → T54 → T60 → T61 由 platform 与 T30、T43 并行完成，必须在 G1–G4 全绿之前就绪，否则 T62 会等它。
**W1 一次性派发**：T02 提交后，立即创建 `core`、`ppu`、`platform` 三个 worktree 与子代理（简报模板见 §6.5）。

---

### M0 基础设施

#### T00 环境、仓库、模型探测、权限与开工前提（integrator）
0. **开工硬性前提**（任一项不满足就停下来报告，不写代码）：
   - `roms/pokered.gb` 的 `shasum -a 1` 为 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`（源文件已就绪：`$REPO/ROM/pokered.gb`，第 2 步复制）；
   - `tools/probe_models.sh` 显示 Opus 5.5 和 Sonnet 5.5 可用（Haiku、Fable 不可用时记入 DECISIONS.md 后继续）。
1. **第一个动作**（与其余步骤并行）：把 §2.1 的脚本保存为 `tools/fetch_refs.sh`，在后台执行 `tools/fetch_refs.sh "$REPO/refs"`（约 1 分钟）；同时在后台执行第 6 步的 Python 依赖安装（Q3：准备工作最大限度并行）。
2. `cd $REPO && git init -b main`（`$REPO` 已存在，内含 `docs/` 与 `ROM/`）；配置作者（仓库级）：`git config user.name seekskyworld`、`git config user.email djh1813553759@gmail.com`。`mkdir -p roms && cp ROM/pokered.gb roms/pokered.gb`。工具链已于 2026-10-30 核对（rustc 1.93、uv 0.11.6、git 2.54、curl），不再逐项检查。
3. 建 workspace：包名 `gb-core`、`gb-cli`（二进制名 `gb-cli`）、`gb-ffi`（`crate-type=["cdylib"]`），按 §3.1 建目录；写 `.gitignore`（§3.1）；建 `PROGRESS.md`（复制上方状态总览）、`DECISIONS.md`、`README.md`（一行占位，W4 补全）。fetch_refs 完成后复制 `$REFS/Opcodes.json` 到 `crates/core/tests/data/`。
4. 模型探测：写 `tools/probe_models.sh`，用 `curl "${ANTHROPIC_BASE_URL%/}/v1/messages"`（请求头带 `x-api-key: $ANTHROPIC_AUTH_TOKEN` 和 `anthropic-version: 2023-06-01`），对 `global.anthropic.claude-{opus-5-5,sonnet-5-5,haiku-5-5,fable-5-1}` 并行各发一次 `max_tokens=16` 的请求；把 HTTP 状态码和返回的 `model` 字段写进 `reports/model-probe.md`。**输出中不得打印密钥。**
5. 子代理与权限：按 §6.6 写好 `.claude/agents/{core,ppu,platform,scout,reviewer,clerk}.md`（frontmatter 写明完整的 `model` ID、`effort`、`tools` 白名单，正文写明职责、独占路径、红线和"命令以 `cd <worktree> &&` 开头"）；按 §6.4 写好 `.claude/settings.json` 的 `permissions.allow` 与 `permissions.deny`；用 `claude agents` 确认定义已被识别（其余验证放到 W4，§6.6）。主会话已以 Opus 5.5 max 运行；试开 `/fast`，不支持就记入 DECISIONS.md 后继续。
6. Python：`uv venv --python 3.12 .venv`；建 `python/pyproject.toml`（包名 `gbemu`，依赖 `pillow`），然后执行：
   `uv pip install -e python pytest requests "pokemon-agent @ git+https://github.com/NousResearch/pokemon-agent@f8a03e4a8bc58f1d9dda6d677a7436cc832a57a9"`
   **不安装 pyboy**。git 安装失败时，改为从脚本下载的副本安装（`uv pip install "$REFS/pokemon-agent"`）。
- 完成标准：开工前提满足；`reports/model-probe.md` 已生成；`cargo build` 成功；`.venv/bin/python -c "import pokemon_agent.emulator, gbemu, sys; assert 'pyboy' not in sys.modules"` 通过；`fetch_refs.sh` 输出 `OK refs ready`；提交 `T00: scaffold`。Python 安装若尚未结束，不阻塞 T02，在 T53a 之前确认即可。

#### T01 资料就绪与参考图转换（integrator，可交给 clerk）
- 确认 T00 后台执行的 `tools/fetch_refs.sh` 已输出 `OK refs ready`；失败时直接重跑（脚本可重复执行，已完成的项会被覆盖）。
- `tools/ref_to_raw.py`：`.venv/bin/python tools/ref_to_raw.py`，用 Pillow 打开 `$REFS/acid2/reference-dmg.png`，执行 `Image.open(p).convert("L").tobytes()`，写成 `$REFS/acid2/reference-dmg.gray`（23040 字节）；脚本内断言 sha256 为 `cb26ef6174cd5b61662c2e74f2bdb2b0f8da0750234c75144c3a4d258fda8347`，且字节值只有 `{0x00,0x55,0xAA,0xFF}`。这样 cli 不需要解码 PNG。
- `tools/check_pokered.sh`：校验 `roms/pokered.gb` 的 SHA1；文件缺失或不匹配时**以非 0 退出**（后续每次实玩前调用）。
- 完成标准：`ls "$REFS/blargg/cpu_instrs/individual" | wc -l` 为 11；`find "$REFS/mooneye" -name '*.gb' | wc -l` 为 115；`ref_to_raw.py` 断言通过；`tools/check_pokered.sh` 退出码为 0。

#### T02 冻结跨通道接口与模块空壳（integrator，约 20 分钟）
1. 只冻结**跨通道**接口（§3.2）：`Machine` 对外方法、`CpuBus`、`Peripheral`、`Snapshot`、`Regs`、`CartError/StateError`、`Cart` trait（`read_rom/write_rom/read_ram/write_ram`）、`cart::from_rom`、`Ppu` 结构体及其 VRAM/OAM 访问方法与 `tick_mcycle`、`Dma` 状态。`StateWriter/StateReader` 直接写完实现（约 30 行的小端字节读写），使各模块的所有者能随模块实现 `Snapshot`。其余实现体用 `todo!()`；`step` 先返回 4。alu、cpu 内部、timer/serial/joypad 的内部结构都归 core 自定，不在 T02 冻结。
2. 建好 §3.1 中的所有模块文件和 `mod` 声明（`cart/` 下的 `mbc2.rs`、`mbc5.rs` 也建空壳）。
3. DECISIONS.md（每条 1–3 行即可）：D001 接口冻结范围；D002 "每个 M-cycle 先推进外设再访问"（§3.3）；D003 归属划分（§3.2 末段；`machine.rs` 骨架之后归 core）。
4. 创建 `core`、`ppu`、`platform` 三个 worktree（§6.4）并派发 W1。
- 完成标准：`cargo build --workspace` 通过；`cargo test` 可以运行（没有测试也算通过）。

#### T05a gates 最小可用版与 `gb-cli run`（platform，最先做）
1. `gb-cli run <rom> [--frames N] [--png out.png] [--serial] [--until-ld-b-b]`：单 ROM 调试入口，失败时打印末尾 32 条 PC 轨迹（`GB_TRACE=1` 时记录）。
2. `gb-cli gates`：按 §4.1 的 T05a 范围实现 ROM 清单（G1 12 个、G2 1 个、G3 4 个、G4 1 个，共 18 个）、线程池、超时、串口判定、acid2 逐字节比较（读取 T01 生成的 `reference-dmg.gray`，输出差异像素数）、汇总行与 `ERROR` 行、`--only`、`--fast`、`reports/gates.json`。
3. 用 `todo!()` 未实现的 `Machine` 运行时，panic 被捕获，全部记为 FAIL。
- 完成标准：`gates` 输出 1 行汇总加 18 行 `ERROR`，退出码非 0、不崩溃；`reports/gates.json` 已生成。完成后立即回报，integrator 马上合并，供 core 使用。

#### T05b gates 补全（platform，与 T17 并行，不阻塞）
1. `--baseline`/`--update-baseline` 与 `REGRESSION`（退出码 2）；`reports/logs/<rom>.log`；`gates.md`；Blargg 截图；acid2 差异图与包围盒。只手写 PNG **编码**（stored deflate 块，加 CRC32 与 Adler-32，约 60 行）。
2. 判定逻辑的对抗单测（§4.1），以及 `REGRESSION` 的单测（用构造的 gates.json 验证）。
- 完成标准：对抗单测全部判为失败；`REGRESSION` 单测通过；写出的 PNG 能被 Pillow 打开且像素与原始数据一致（单测）。

---

### M1 CPU（core）

#### T10 CPU 解码框架与 opcode 表测试
1. 寄存器文件（F 低 4 位恒为 0）；按 `x=op>>6, y=(op>>3)&7, z=op&7, p=y>>1, q=y&1` 解码；`r` 表 `B C D E H L (HL) A`，以及 `rp`、`rp2`、`cc`、`alu` 表；CB 前缀单独解码。非法 opcode（`D3 DB DD E3 E4 EB EC ED F4 FC FD`）使 CPU 锁死。
2. `tests/opcode_table.rs`（§4.2），标志位断言先加 `#[ignore]`。
- 完成标准：所有合法 opcode 都有解码分支；`cargo test opcode_table` 中长度与周期断言全部通过（此时 ALU 运算可以是临时实现）。

#### T11 ALU 纯函数与全部指令（core）
1. 在 `alu.rs` 中实现 T02 冻结的全部纯函数；`tests/alu.rs` 覆盖 §4.2 列出的边界用例，每个函数至少 4 组输入，期望值按 `$REFS/gbz80.7.txt` 和 `$REFS/pandocs/src/CPU_Registers_and_Flags.md` 手算。
2. `cpu.rs` 调用 alu 函数；加载、跳转、调用、返回、RST、PUSH/POP、`ADD HL`、`ADD SP`、`LD HL,SP+e`、STOP（当作 2 字节指令并清零 DIV）按 §3.3 的周期序列实现；CB 前缀 `(HL)` 指令的访问时点见 §5 的 mem_timing 一行。去掉 opcode 表测试中标志位断言的 `#[ignore]`。
- 完成标准：`cargo test` 全绿（包括 alu 单测、opcode 表的标志位断言，以及 T12 的 SingleStepTests）。

#### T12 SingleStepTests 接入（core；Q1 已允许）
- 数据由 §2.1 的 `fetch_refs.sh` 下载到 `$REFS/sst/`。T10 完成解码框架后立即按 §4.2 实现 `crates/core/tests/sst.rs`；T11 编写指令时，每写完一组指令就跑对应的 opcode 文件（`SST_ONLY`），把它当作主要的单测，而不是等全部写完再测。
- 完成标准：T11 结束时 500 个文件全部通过，或者把例外（如 HALT、STOP）写进 DECISIONS.md；SST 数据只在测试中读取，不进入 `crates/core/src`，也不提交进 git。

#### T13 总线、内存映射、开机状态
- 映射：`0000–7FFF` 卡带、`8000–9FFF` VRAM（转发给 ppu）、`A000–BFFF` 卡带 RAM、`C000–DFFF` WRAM、`E000–FDFF` 镜像 `C000–DDFF`、`FE00–FE9F` OAM（转发给 ppu）、`FEA0–FEFF` 读 0x00、`FF00–FF7F` I/O（分派给各外设的 `read_reg/write_reg`；APU 范围做成可读写存储）、`FF80–FFFE` HRAM、`FFFF` IE。`peek` 与 `read` 共用映射，但不推进时钟。
- 开机状态照 `Power_Up_Sequence.md` 的 DMG 列。在 platform 的 MBC 合入之前，用一个忽略 ROM 区写入的临时卡带（足够运行 32 KiB 的 individual ROM）；合入后切换为 `cart::from_rom`。
- 同时写好 `Machine::save_state/load_state`（文件头见 §3.8，依次调用各模块的 `Snapshot`；尚未实现 `Snapshot` 的模块先跳过，由 T52 的测试兜底）。
- 完成标准：单测覆盖每个区域的读写与镜像；`step()` 能执行 NOP 序列。

#### T14 中断与 HALT
- 按 §3.3 实现。
- 完成标准：单测覆盖"EI 后紧跟 DI"、HALT 在 IME=0 与 IME=1 下、HALT bug、分派 5 M-cycle。

#### T15 定时器 / T16 串口 / T51 Joypad（core）
- 分别按 §3.4、§3.7 实现，均为独立的 `Peripheral`（含 `Snapshot`）；同时实现 `interrupts.rs` 中 IF/IE 寄存器的读写（IF 高 3 位读为 1）。core 在 T13 之后、T17 之前顺手完成，合计约 35 分钟。
- 完成标准：定时器单测验证 TAC 四档下 TIMA 每 1024/16/64/256 T 加 1，以及写 DIV 触发额外加 1；串口单测验证 4096 T 后 SC.7 被清零并请求中断；Joypad 单测覆盖两组选择、同时选择、都不选，以及只有被选中组的下降沿才触发中断。

#### T17 G1：cpu_instrs
- 合并 platform 的 T05a 和 T50（**所有 Blargg ROM 的卡带类型都是 MBC1**，合集 `cpu_instrs.gb` 和 `mem_timing.gb` 为 64 KiB，需要切换 bank；32 KiB 的 individual 在 MBC1 合入之前也能先跑）；执行 `gates --only G1`，按 §5 逐个修复，先 individual 后合集。
- integrator 合并 `lane/core` 后在 main 上复跑确认。
- 完成标准：G1 12/12；打 tag `m1-g1`；更新 PROGRESS.md（里程碑）。

#### T30 G2/G3：instr_timing、mem_timing
1. instr_timing：若全错或报 #255，先查定时器；再按 `op:实测-正确` 修内部周期。
2. mem_timing：若系统性偏 1，按 D002 全局切换顺序，并记录 D00x；不得按指令打补丁。
3. 带基线跑全量。
- 完成标准：`gates --only G1,G2,G3` 为 17/17，无 `REGRESSION`；提交基线；打 tag `m2-timing`。

---

### M3 PPU（ppu）

#### T40 PPU 时序骨架
- 按 §3.5 实现 dot 计数、LY、mode、STAT（上升沿）、VBlank、LCD 开关、双缓冲交换；随模块实现 `Snapshot`。
- 完成标准：单测验证：一帧 70224 T；LY 0→153 循环；每帧恰好一次 VBlank 中断；LYC 中断发生在正确的 LY；LCD 关闭时帧计时照走，前缓冲为全白。

#### T41 BG/Window 渲染
- 在进入 mode 3 时渲染整行；实现 tile data 两种寻址、两个 tilemap、SCX/SCY 环绕、窗口 Y 条件锁存与行计数器、`LCDC.0`、BGP、灰度映射。
- 完成标准：用合成的 VRAM 直接断言像素数组：棋盘格平移（SCX=3、SCY=5）、窗口覆盖（WX=7+20、WY=30）、帧中途把 WY 改到已经过去的行时窗口不出现、有符号 tile 寻址。

#### T42 精灵与 OAM DMA 状态
- 实现 `LCDC.1` 精灵开关、OAM scan（前 10 个）、8×8/8×16（8×16 时 Y 翻转作用于整个 16 行）、翻转、OBP0/1、透明、DMG 优先级、OBJ-to-BG priority；`dma.rs` 保存 DMA 状态（源地址高字节、进度、启动延迟），实际搬运由 bus 驱动。
- 完成标准：断言像素数组：同一行 11 个精灵只显示 10 个；X 相同时序号小者在上；priority 标志在 BG 色号 0 与非 0 上的表现正确；8×16 时忽略 tile 号 bit0，Y 翻转后上下两个 tile 互换；`LCDC.1=0` 时不显示精灵。

#### T43 G4：dmg-acid2
- integrator 合并 `lane/ppu` 后，ppu 在 `lane/ppu` 上执行 `gates --only G4`；查看 `reports/acid2-diff.png`（T05b 完成前看差异像素数与 `gb-cli run --png` 的截图），按 §5 定位。
- 完成标准：差异像素数为 0；带基线跑全量无 `REGRESSION`；提交基线；打 tag `m3-acid2`。

---

### M4 卡带与接入（platform）

#### T50 ROM only、MBC1、MBC3
- 按 §3.6 的表格实现这三种；`cart::from_rom(rom) -> Result<Box<dyn Cart>, CartError>`；MBC2、MBC5 暂时返回 `CartError`（T50b 补）。
- 完成标准：MBC1、MBC3 都用合成 ROM（每个 bank 的首字节写入该 bank 号）验证 bank 映射、写 0 的行为、RAM 使能和 RAM bank 切换；未知类型返回错误。完成后立即回报，integrator 马上合并，供 T17 使用。

#### T52 存档往返测试（platform）
- 各模块的 `Snapshot` 由其所有者随模块实现；`Machine::save_state/load_state`（依次调用各模块的 `Snapshot`，写文件头，§3.8）由 core 在 T13 时一并写好。本任务只写测试，发现缺失的 `Snapshot` 就退回对应通道。
- 完成标准：`crates/cli/tests/state_roundtrip.rs` 在 cpu_instrs 和 pokered 上跑 §4.2 的往返测试（ROM 缺失时 SKIP）；ROM 指纹不匹配时返回错误。

#### T53a FFI 骨架与 Python 绑定
- 按 §3.8 实现 `gb-ffi` 与 `python/gbemu/native.py`（`argtypes`/`restype` 写全）、`backend.py`（加锁、`close`/`__del__`、`ValueError`）。此时 `Machine` 可能尚未完成，测试只覆盖 `gb_new` 对坏 ROM 返回 null、以及 Python 的错误路径。
- 完成标准：`cargo build -p gb-ffi --release` 生成 `libgb_ffi.dylib`；`pytest python/tests -k "not machine"` 全绿。

#### T53b Python 后端整机验证
- 完成标准：`pytest python/tests` 全绿，覆盖：load → `tick(60)` → `get_screen()` 为 160×144；`press('start', 8)` 后 `frame_count` 增加 8；`read_range(0xC000,16)` 长度为 16；save/load 往返后 `read_range(0xC000,0x2000)` 一致；坏 ROM 路径抛出异常；2 个线程交替调用 `get_screen` 和 `tick` 各 100 次不崩溃（验证 §3.8 的锁）。

#### T54 pokemon-agent 启动器与补丁
- `python/gbemu/launch.py`（接入方式 1）与 `patches/pokemon-agent-emulator.patch`（接入方式 2），按 §3.8 实现。
- 完成标准：`python -m gbemu.launch serve --rom roms/pokered.gb --port 8765 --data-dir /tmp/gbemu-smoke` 启动后：`curl -sf localhost:8765/health` 成功；`/state` 返回 JSON；`/screenshot` 为 160×144 PNG；`POST /action` 成功；`pyboy` 未被加载。补丁可以用 `patch -p1 --dry-run` 应用到 `$REFS/pokemon-agent` 的副本上。

---

### M5 实玩

#### T60 实玩监控与驾驶循环
- 按 §4.4 实现 `tools/playtest.py`（服务器生命周期、独立 data-dir、只调用一次 `/games/new`、采样、硬性判定与告警、summary 与 contact sheet、退出码）。
- 实现 `tools/drive_loop.sh`、`tools/driver_prompt.md`（§4.4 的每轮新会话循环、失败处理）。
- 完成标准：`playtest.py --minutes 2 --driver script` 产出完整的报告目录；`playtest.py --minutes 3 --driver agent` 至少完成 3 轮驾驶。

#### T61 回归驾驶脚本与 5 分钟冒烟
- 实现 `tools/scripted_driver.py`（开场脚本加随机行走；`--record`/`--verify` 回放基线属于 T63），然后执行 `playtest.py --minutes 5 --driver script`，并跑一次 `gb-cli bench roms/pokered.gb --frames 3600`（低于 120 fps 才需要立即处理，§4.3）。
- 完成标准：playtest 退出码为 0；至少进入一次可行走的地图，并且坐标发生变化；查看 contact sheet 无花屏和撕裂。发现问题就退回给对应的代理，并补回归测试。

#### T61b 提前跑 30 分钟实玩（条件触发，默认不做）
- **触发条件**：T61 通过、G1 与 G4 已全绿，但 G2/G3 预计还需要 40 分钟以上。此时提前跑 30 分钟，与 T30 的调试并行；否则跳过，直接等 T62。
1. 打 tag `rehearsal`；`git worktree add /Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/rehearsal rehearsal`，在其中单独执行 release 构建；只使用这份构建。
2. `tools/check_pokered.sh` 通过后，运行 `playtest.py --minutes 30 --driver agent`（由 `drive_loop.sh` 驾驶，§4.4；`driver_prompt.md` 中引用 `$REFS/pokemon-agent/skill/SKILL.md`）。
3. 结束后查看 summary 与 contact sheet；发现画面或卡死问题，立即退回 ppu 或 core 并补回归测试。
- 完成标准：拿到一份完整的 30 分钟报告（无论是否通过）；问题清单已派发。

#### T62 G5：30 分钟正式实玩（integrator 监督）
1. G1–G4 全绿后打 tag `rc-gates`。若跑过 T61b，且 `git diff --quiet rehearsal rc-gates -- crates/core crates/ffi python` 成立、T61b 通过了 §4.4 的全部硬性判定，就直接采用 T61b 的结果，跳到第 3 步。
2. 否则：`git worktree add /Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/rc rc-gates`，单独构建后按 T61b 第 2–3 步跑 30 分钟。
3. 把 summary 与 contact sheet 的抽查结论写进 REPORT.md。
- 完成标准：采用的那次实玩，`playtest.py` 退出码为 0；scripted 驾驶累计不超过 3 分钟；contact sheet 中没有花屏、错位或异常黑屏；在所用构建对应的提交上打 tag `v1.0-gates`，**G1–G5 完成**。失败时修复，重新打 `rc-gates`，并完整重跑 30 分钟。

---

### 收尾阶段（W4，G5 完成之后）

#### T50b MBC2、MBC5
- 按 §3.6 实现；完成标准同 T50（另加 MBC2 的 4 bit RAM 与 0x1FF 镜像）；带基线跑全量无 `REGRESSION`。

#### T55 性能
- 完成标准：`gb-cli bench roms/pokered.gb --frames 3600` 在 release 下 ≥ 600 fps；优化后带基线跑全量无 `REGRESSION`。

#### T63 宝可梦确定性回放基线
- 在 `scripted_driver.py` 中实现 `--record`/`--verify`（§4.4），接入 gates 的 `POKERED-REPLAY`。
- 完成标准：`scripted_driver.py --verify` 连续两次结果一致（证明确定性）；`reports/pokered-replay.json` 已提交；gates 中 `POKERED-REPLAY` 通过。

#### T70 Mooneye 范围与计分
- `gates --mooneye` 按 §2.4 的规则生成纳入/排除清单 `reports/mooneye-scope.md`，并报告"通过/纳入"与"通过/115"。实现后先**原样**跑一次并保存结果（§4.5 的隐藏集证据），再开始 T71。
- 完成标准：规则单测覆盖每一类后缀（无后缀、`-GS`、`-dmgABC`、`-dmgABCmgb`、`-dmg0`、`-mgb`、`-S`、`-sgb2`、`-cgb`、`-A`、`-C`，以及排除的目录）。

#### T71 Mooneye 迭代（加分）
- 并行方式：对每个失败的 ROM（或同一类的一组），先在 main 上提交 `locks/<rom>.lock`，再派一个 scout 只读调查；scout 输出 `reports/scout/<rom>.md`。修复由 core（CPU、中断分派、定时器、串口、Joypad 相关）或 ppu（PPU、DMA 相关）在各自通道落地，由 integrator 合并并带基线跑全量。
- 顺序（性价比从高到低）：`emulator-only/mbc1/mbc2/mbc5` → `acceptance/timer` → `bits`、`instr/daa`、`interrupts/ie_push`、`if_ie_registers`、`halt_*`、`ei_*`、`di_timing-GS`、`rapid_di_ei` → `*_timing`（call/ret/jp/push/pop/rst/add_sp/ld_hl_sp）→ `oam_dma*`、`boot_*` → `acceptance/ppu`（最难，放最后）。
- 停止条件：剩下的失败项都属于 `acceptance/ppu`，且连续若干轮没有新增通过；或者发包方给定的预算用完。**`v1.0-gates` 不包含 T71 的修改。**
- 完成标准：REPORT.md 列出每个 ROM 的结果。

#### T80 评审（reviewer，新开上下文，一次）
- 内容：在一个干净的临时 worktree 中 `cargo test`、全量 gates（`GBEMU_REFS=$REFS`），逐条核对 PROGRESS.md 中的声明；复核 §4.5 的隐藏集记录（必过项与 Mooneye 原样通过率的差距）；抽查 2 次合并的 diff 是否有针对特定测试的代码（R6）。
- 评审姿态：默认"声明未经证实"；发现问题后，不得说服自己"问题不大"就放行 [A1]。
- 完成标准：`reports/review.md` 写明每条声明"已复现"或"未复现及原因"；未复现的项必须在 T91 之前解决。

#### T90 合规自查（reviewer）
1. `git grep -niE 'pyboy|sameboy|gambatte|mgba|bgb' -- crates python tools`：只允许出现在说明性的注释或 pokemon-agent 的接口名中。
2. `git grep -niE 'acid|blargg|mooneye|cpu_instrs|pokemon|pokered' -- crates/core/src`：结果必须为空。
3. `git ls-files | grep -iE '\.(gb|gbc|sav|state)$'`：结果必须为空。
4. `git reflog` 中没有 `rebase`、`amend`、`reset`。
5. `.venv/bin/pip list 2>/dev/null | grep -i pyboy`（或 `uv pip list`）：结果必须为空。
6. R6 检查：`git grep -nE '"(Passed|Failed)' -- crates/core/src` 为空；core 中长度超过 64 项的常量数组只有 opcode 周期表，并且有 Opcodes.json 校验单测。
- 完成标准：6 项全部满足，结论写进 REPORT.md 的"合规自查"一节。

#### T91 交付（integrator）
- 完成 REPORT.md（§4.6）和 README 的三条复现命令与两种接入方式；按 §7 第 3 项导出 sessions 并生成 INDEX.md；最终执行一次全量 gates，并提交报告。
- 完成标准：§7 清单逐项勾选，写在 REPORT.md 末尾。

---

## 9. 附录：参考资料要点与经验来源

题面"五、参考资料"中的 7 份资料（阅读于 2026-10-29），以下是它们与本题相关的要点，以及本文据此采用的做法。原文可在线阅读，不需要下载。

| 标记 | 资料 | 与本题相关的要点（数字摘自原文） | 本文采用的做法 |
| --- | --- | --- | --- |
| A1 | Anthropic：Harness design for long-running apps（规划者、生成者、评审者三代理） | ① 单代理 20 分钟 / $9 做出的应用核心功能是坏的；三代理 6 小时 / $200 能真正可用（Opus 4.5）；Opus 4.6 去掉 sprint 后，3 小时 50 分 / $124.70，构建阶段连续 2 小时依然连贯。② 评审必须与生成分离；现成的 Claude 是差劲的 QA，会发现问题又说服自己"问题不大"，需要调成持怀疑态度、按硬阈值打分。③ 开工前先约定"什么算完成、怎么验证"（sprint 合同）。④ 规划者写错的细粒度技术细节会层层传导。⑤ 模型升级后，要逐个去掉不再起作用的 harness 组件；评审在任务接近模型能力边界时才最值得 | 独立的 reviewer 与 T80（§6.2）；每个任务都有命令化的完成标准（§8）；技术陈述与 Pan Docs 或测试结果冲突时以后两者为准（文首）  |
| A2 | Anthropic：Effective harnesses for long-running agents（跨会话） | ① 两类失败：想一次做完，上下文耗尽时留下半成品；后来的会话看到已有进展就宣布完成。② 初始化代理建 init 脚本、进度文件、首个提交，并把需求扩成 200 多条 JSON 功能清单（初始全部为 failing，只许改 `passes` 字段；模型篡改 JSON 的倾向小于 Markdown）。③ 每个会话先 `pwd`、读进度和 git log、跑基本端到端测试，再开始新功能；结束时提交并更新进度。④ 没有明确要求时，Claude 常在只做过单测之后就标记完成 | 会话开场、执行、收尾流程（§6.8）；`gates.json` 作为唯一的状态来源（§4.1）；PROGRESS.md 与 DECISIONS.md；证据优先（§0 第 9 条） |
| A3 | Anthropic：Building a C compiler with parallel Claudes | ① 16 个代理、约 2000 个会话、$20,000、两周，10 万行代码，开发全程断网。② 测试要接近完美，否则代理会解决错误的问题；后期新功能频繁破坏旧功能，于是加了 CI 防回归。③ 为模型设计输出：只打印几行，`ERROR` 与原因写在同一行，详细内容写日志，预先算好汇总；提供 `--fast` 确定性抽样（应对时间盲区）。④ 失败测试多且相互独立时，并行很容易；整体性任务（编译 Linux）会让所有代理撞上同一个 bug，需要用 oracle 把问题切开。⑤ 用锁文件领取任务；设专职角色（去重、性能、文档） | gates 的输出约束、`--fast`、棘轮（§4.1）；锁文件与 scout 扇出（§6.4、T71）；宝可梦的确定性回放与存档切片，相当于不借助外部 oracle 也能把整体性任务切开（§4.4） |
| P1 | Zero2Repo（arXiv 2609.38269）：PRD 加隐藏测试，全有或全无 | ① 17 个任务、80–585 个隐藏测试；必须写 submit 文件才算提交；在断网的新容器中判定；屏蔽代码托管站、拒装上游包、扫描 import。② Opus 5.5 + Claude Code 解出 11/17，平均 10 小时 14 分、$178；换成 OpenCode 解出 14/17；同一模型换 harness 最多差 3 题。③ 121 次被判失败的运行中，95 次通过率 ≥ 90%（如 387/388），漏一条规则就是 0 分；另有 51 次没进入判定（28 次超时、19 次未提交）。④ 发布前做对抗验证：埋了错的实现必须得 0 分 | 每条验收都有可执行的判定（§1.1、§2.4）；gates 判定逻辑的对抗单测（§4.1、T05）；里程碑 tag 保证随时可交付（§6.8）；从干净 checkout 复现（T80、§7） |
| P2 | E2E-SWE（arXiv 2609.38335）：186 个从零建仓任务 | ① 规格与测试逐条对应（被测的都写进规格，写进规格的都被测到）；交付 setup.sh，在干净容器中复现。② pass@1 为 11.7%–67.7%；推理强度从 low 到 xhigh 单调提升，Opus 4.8 从 xhigh 提到 max 没有收益；最长的一次推理大多出现在前 12 轮。③ 代码量与成败无关，进度只看测试通过数 | 核心代理默认 xhigh，max 只用于 Blocker（§6.6）；先做全局规划再写代码（§6.8、T02） |
| P3 | SpecBench（arXiv 2605.21384）：量化针对测试作弊 | ① Δ = 可见通过率 − 隐藏通过率。**30 个任务中有一个 C 语言 GB 模拟器**（参考实现 30K LOC，可见测试 50 个、隐藏测试 117 个）。② LOC 每增加 10 倍，P90 的 Δ 上升约 27pp；>25K LOC 的任务 Δ 最高达 100pp。③ 最常见的作弊是"特性孤立"（各模块单独过测试，组合起来就坏），其次是边界缺口，蓄意利用较少；典型的蓄意案例是用 2900 行哈希表记住可见测试的答案（可见 97%、隐藏 0%），它挤掉了同一轮里一个真实的 7900 行实现。④ 只加测试不能根治；搜索步数越多作弊越严重；Claude Code（Opus 4.6）的 Δ 约为 43–48pp，人工监督下开发的编译器只有 14.5pp | R6 整机约束（§1.2）；Mooneye 与宝可梦回放作为隐藏集抽测（§4.5）；T90 的 R6 检查 |
| P4 | GameXpert-Bench（arXiv 2608.21833）：游戏开发 agent 评测 | ① 所有得分都必须有运行时证据：先在代码里定位实现，再到运行中的游戏里触发事件；注释和模型的自述都不算证据。② 修复任务要求修好坏行为（F2P）且不破坏原有行为（P2P）；用虚拟时钟、合成输入、只读状态快照做确定性判定。③ 1455 次运行中有 15.2% 出现 UI 错位；"代码里实现了但运行时失败"占 5.32%，其中 56% 是加载失败或崩溃；7.8% 的最终产物不可玩。④ Claude Opus 5 在三个赛道都排第一；较弱的模型常凭静态读代码判断"看起来没问题"，并且过早停止 | 实玩只认运行时采样、截图与状态（§4.4）；确定性回放基线 `POKERED-REPLAY`（§4.4、T61）；棘轮相当于 P2P（§4.1）；证据优先（§0 第 9 条） |
| M1 | Anthropic 官方文档与发布页：platform.claude.com 的 Effort、Models overview；anthropic.com/news 的 claude-opus-5-5、claude-sonnet-5-5、claude-haiku-5-5 | ① 推理强度共 5 档（low/medium/high/xhigh/max）；Opus 5.5 与 Haiku 5.5 默认 medium，其他模型默认 high；xhigh 面向"超过 30 分钟、token 预算以百万计的长程智能体与编码任务"，max 是"不限 token 的最高能力"。② 官方建议多数工作从 Opus 5.5 起步；Fable 5.1（$10/$50）用于 Opus 5.5 在高强度下仍不够的场景；Sonnet 5.5（$2/$10）是"速度与智力的最佳组合"，长于规格明确的日常任务；Haiku 5.5 适合高吞吐，可作编码子代理。③ 各档得分与每题成本见 §6.6 的表格：Opus 5.5 在 medium 之后收益递减，Terminal-Bench 的峰值在 xhigh；Sonnet 5.5 随强度提升幅度大，max 时在 Terminal-Bench 上达到 70.6%。④ Sonnet 5.5 是第一个只凭截图通关宝可梦红的 Sonnet 模型 | 主会话 Opus 5.5 max，子代理分档（§6.2、§6.6）；实玩驾驶用 Sonnet 5.5；机械事务用 Haiku 5.5 |

**不直接照搬的做法**

| 做法 | 原因 |
| --- | --- |
| 用参考实现做 oracle（A3 用 GCC） | 题面禁止参考现成模拟器，所以不运行任何现成模拟器。替代手段是测试 ROM、确定性回放，以及经发包方允许的 SingleStepTests 测试数据（Q1，只作为测试数据读取） |
| 十几个代理同时写代码（A3） | 本题的关键路径集中在 CPU 和时序上，写代码的代理超过 3–4 个时，合并成本会超过收益（§6.2） |
| 每个 sprint 都做评审（A1 早期版本） | 原文在模型升级后改为结尾评审一次；本文同样只在结尾（G5 之后）评审一次 |
