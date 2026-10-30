# GB（DMG）模拟器：从零实现

从零编写的初代 Game Boy（DMG）模拟器：Rust 核心（`gb-core`，零依赖）→ C ABI（`gb-ffi`）→ Python `ctypes` 后端（`gbemu`）→ [pokemon-agent](https://github.com/NousResearch/pokemon-agent)。
没有参考、复制或安装任何现成模拟器的代码（含 PyBoy）；硬件依据是 Pan Docs、gb-opcodes 指令表与各测试 ROM 的说明和源码。

结果、证据与合规自查见 [REPORT.md](REPORT.md)；计划与任务清单见 [docs/PLAN.md](docs/PLAN.md)；设计决定见 [DECISIONS.md](DECISIONS.md)。

## 准备（一次）

```bash
tools/fetch_refs.sh "$PWD/refs"            # 参考资料与测试 ROM（约 1 分钟，sha256 校验）
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python -e python pytest requests \
  "pokemon-agent @ git+https://github.com/NousResearch/pokemon-agent@f8a03e4a8bc58f1d9dda6d677a7436cc832a57a9"
.venv/bin/python tools/ref_to_raw.py refs  # acid2 参考图 → 原始灰度（gates 用）
mkdir -p roms && cp /path/to/pokered.gb roms/pokered.gb && tools/check_pokered.sh   # 美版红 v1.0，SHA1 ea9bcae6…
```

## 三条命令复现

```bash
# 1. 构建（release；生成 target/release/gb-cli 与 target/release/libgb_ffi.dylib）
cargo build --release

# 2. 全部门槛（G1–G4，加 --mooneye 计入加分项）；输出 ≤30 行，详见 reports/gates.md
GBEMU_REFS=$PWD/refs target/release/gb-cli gates --mooneye --baseline reports/baseline.json

# 3. 宝可梦红 30 分钟实玩（Sonnet 5.5 驾驶；--driver script 为无 LLM 的回归驾驶）
GBEMU_LIB=$PWD/target/release/libgb_ffi.dylib .venv/bin/python tools/playtest.py \
  --minutes 30 --driver agent --port 8780
```

`cargo test` 不依赖 ROM：缺少 `refs/` 或 `roms/` 时相关测试打印 SKIP 并通过。

## 接入 pokemon-agent 的两种方式

1. **启动器（默认）**：覆盖 pokemon-agent 的 `_EXT_MAP`，`.gb/.gbc` 使用本模拟器，并断言未加载 pyboy。
   ```bash
   GBEMU_LIB=$PWD/target/release/libgb_ffi.dylib .venv/bin/python -m gbemu.launch \
     serve --rom roms/pokered.gb --port 8780 --data-dir /tmp/gbemu-data
   ```
2. **补丁**：在 pokemon-agent 源码根目录执行 `patch -p1 < patches/pokemon-agent-emulator.patch`，之后用它自己的 CLI 启动（`pokemon-agent serve --rom … --port …`），仍需设置 `GBEMU_LIB` 并安装 `python/`（`gbemu`）。

## 环境变量

| 变量 | 作用 |
| --- | --- |
| `GBEMU_LIB` | `libgb_ffi.dylib` 的路径（Python 后端只认它） |
| `GBEMU_REFS` | 参考资料目录（默认 `./refs`） |
| `GBEMU_POKERED` | 宝可梦 ROM（默认 `roms/pokered.gb`） |
| `GB_TRACE=1` | `gb-cli run` 记录末尾 32 条 PC |

## 目录

```
crates/core   gb-core：CPU、总线、定时器、串口、Joypad、PPU、OAM DMA、卡带（ROM only/MBC1/MBC2/MBC3/MBC5）、存档
crates/cli    gb-cli：run / gates / bench（零依赖，自带 PNG 编码）
crates/ffi    gb-ffi：C ABI（catch_unwind 包住每个入口）
python/gbemu  ctypes 绑定、pokemon-agent 后端、启动器
patches/      pokemon-agent 接入补丁
tools/        资料下载、实玩监控 playtest.py、驾驶循环 drive_loop.sh、回归驾驶 scripted_driver.py、会话导出
reports/      gates 结果、基线、截图、实玩报告
sessions/     Claude Code 会话记录（实施会话与子代理、T62 驾驶会话；已打码）与 INDEX.md
```
