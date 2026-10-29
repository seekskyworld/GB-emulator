# DECISIONS

> 每条 1–3 行。改动冻结接口（D001）前必须先在这里记一条。

- **D001 接口冻结范围（T02）**：`lib.rs`（`Peripheral`、`Frame`/`FRAME_T`/`irq`/`buttons` 常量）、`state.rs`（`Snapshot`、`StateWriter/StateReader`、`StateError`，已完整实现）、`cpu.rs` 的 `CpuBus` 与 `Cpu` 公开字段/`new`/`step`、`machine.rs` 的 `Machine` 公开方法与 `Regs`、`cart/mod.rs` 的 `Cart`/`from_rom`/`CartError`、`ppu.rs` 的 `Ppu` 公开方法、`dma.rs` 的 `Dma` 公开方法。其余（alu、cpu 内部、bus 内部、timer/serial/joypad/interrupts 内部结构）由各通道自定。新增公开方法或字段不算改签名，但须在回报中说明。
- **D002 时序顺序**：每个 M-cycle 内 Bus 先把外设推进 4 T（timer → serial → joypad → ppu → dma），再执行这次访问。T30 若证明需要反过来，只在 Bus 一处全局切换，不得按指令打补丁。
- **D003 归属**：IF/IE 在 `interrupts.rs`；VRAM/OAM 数组在 `ppu.rs`（Bus 转发）；OAM DMA 由 `bus.rs` 驱动、`dma.rs` 只存状态；卡带 ROM/RAM 在 `cart/`；WRAM/HRAM/APU 寄存器存储在 `bus.rs`。`machine.rs` 骨架之后归 sys。
- **D004 通道拆分（用户 2026-10-30 指令"最优的方式多开子代理并行实施"）**：v2 的 3 个写代码通道拆成 5 个：`core`（CPU：cpu/decode/alu，T10–T12、T14 的 CPU 侧）与 `sys`（bus/machine/interrupts/timer/serial/joypad，T13、T14 的 IF/IE、T15、T16、T51）以冻结的 `CpuBus` 为接缝；`platform`（cart/cli/gates，T05a/T05b/T50/T52/T70）与 `play`（ffi/python/patches/实玩工具，T53a/T53b/T54/T60/T61）以 `Machine` 公共接口为接缝。关键路径由"CPU→总线→外设"串行变为并行。编译线程每通道 `CARGO_BUILD_JOBS=3`（5×3=15 核）。
- **D005 不开 `/fast`**（用户 2026-10-30 决定）。
- **D006 Mooneye 加分不设预算**（用户决定），按 T71 的停止条件执行。
- **D007 交付形式**：只交付本地仓库，不建 remote、不 push（用户决定）。
- **D008 子代理定义的验证**：Claude Code 2.1.296 的 `claude agents` 是后台 agent 管理命令，不列出定义；改为直接用 Agent 工具按 `subagent_type` 派发来验证（不可用时退回 `general-purpose` 并在简报中写入定义正文与无联网约束）。
- **D009 模型别名**：`~/.claude/settings.json` 把 opus/sonnet/haiku/fable 别名和 `CLAUDE_CODE_SUBAGENT_MODEL` 都映射到 `global.anthropic.claude-opus-5-5[1M]`。Opus 类子代理派发时显式传 `model: opus`（即 Opus 5.5 1M 上下文）并显式传 effort；frontmatter 同时写完整 ID。clerk 的 Haiku 5.5 只能靠 frontmatter 完整 ID，不生效时由 Opus 代做（只影响速度）。驾驶会话用 `claude -p --model global.anthropic.claude-sonnet-5-5` 直接指定，不受别名影响。探测结果见 `reports/model-probe.md`（4 个完整 ID 均 200）。
- **D010 依据文件**：任务书已由用户改为 Markdown（`docs/GB 模拟器高速编程研究 · 技术文档与资料.md`，内容与原 RTF 一致）；计划以仓库内 `docs/PLAN.md` v2 为准，上级目录 `research-platform/docs/PLAN.md`（v1，路径为旧的 `open/gb-emu`）不采用。
- **D011 T02 临时卡带**：`cart::from_rom` 在 T50 之前把 ≤32 KiB 的 ROM only/MBC1/MBC3 一律当作 ROM only（忽略 MBC 寄存器写入），使 core/sys 能先跑 32 KiB 的 Blargg individual；T50 替换为真实实现。
- **D012 Cargo profile**：`[profile.dev] opt-level=1`、`[profile.test] opt-level=3`，使 SingleStepTests（约 50 万用例）与整机单测在 `cargo test` 下足够快。
- **D013 PPU/DMA 空壳不 panic**：T02 的 `Ppu::tick_mcycle` 返回 0、寄存器为可读写存储、`Dma::tick_mcycle` 返回 `None`，避免 core/sys 在 ppu 合入前跑 ROM 时 panic（Blargg 控制台在 LY 不变时会超时后继续，见 `refs/blargg/cpu_instrs/source/common/console.s`）。
- **D014 路径环境变量**：测试 ROM 与资料从 `$GBEMU_REFS`（默认 `<repo>/refs`）读取；宝可梦 ROM 从 `$GBEMU_POKERED`（默认 `<repo>/roms/pokered.gb`）读取。lane worktree 中没有 `refs/`、`roms/`，必须设置这两个变量为主仓库的绝对路径。
