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
- **D015 SST 格式（实测，修正 PLAN §4.2）**：`cycles` 中内部周期编码为 `[地址, 数据, "---"]`（不是 `null`）；读为 `"r-m"`、写为 `"-wm"`。`initial` 含 `ie` 字段（0/1，不是 IE 寄存器），`final` 对 EI 含 `ei`。PC 指向操作码本身，取指是第 1 个 M-cycle。
- **D016 gates 帧循环**（platform T05a）：gates 不直接调用 `run_frame`，而是按与它相同的 70224 T 余量规则逐条 `step()`，以便记录末尾 32 条 PC；`step` 若返回 0 则报错而不是挂起。
- **D017 gates 提前结束**：检测到 "Passed"/"Failed" 后继续跑到该行换行为止（最多再跑 60 帧），避免把 "Passed all tests" 截成 "Passed al"，也能捕获紧随其后的 "Failed"。acid2：每帧边界检查 LD B,B，须在前 119 帧内出现，之后再跑 1 帧（共 120 帧）。
- **D018 卡带细则**（platform T50）：新增 `CartError::UnsupportedRamSize`；ROM 不足 16 KiB 整数倍时补 0xFF（只有不足 0x150 字节才报错）；RAM 大小代码 0x01 按 2 KiB；只有类型含 RAM 才有 RAM；MBC3 RTC 寄存器只在 TIMER 类型（0x0F/0x10）存在。
- **D019 基线只前移**：`--update-baseline` 只把新通过的项并入基线；存在任何 REGRESSION 时拒绝更新。合并时的 gates 输出写到 `/tmp`（`--out`），仓库内的 `reports/gates.*` 只在里程碑重写并提交。
- **D020 bench（PPU 空壳时）**：pokered 22478 fps、cpu_instrs 17122 fps、acid2 21431 fps（release）；PPU 合入后在 T61 重测。
- **D021 SST 例外与 CPU 细则**（core T12/T14）：SST 500/500 文件、50 万用例通过；两项部分检查——`76 HALT`：SST 期望 3 M-cycle，本实现按 Opcodes.json 为 1（之后 halted 期间每次 step 1 个 idle），只比较寄存器/RAM/取指；`10 STOP`：SST 期望 PC+1 与 3 M-cycle，本实现按 Pan Docs/gbz80(7)/Opcodes.json 为 2 字节、1 M-cycle（第二字节跳过不读）并调用 `bus.stop()`（不进入低功耗，等同 2 字节 NOP），不比较 PC 与周期数。`Cpu` 新增公开字段 `ei_pending`/`halt_bug`/`locked` 与 `af/bc/de/hl` 访问器（不改冻结签名）。中断分派会取消挂起的 EI；`EI; HALT` 且已有挂起中断时触发 HALT bug；向量在写完 PCH 后决定（ie_push：IE 被清则 PC=0x0000、不 ack）；HALT 唤醒不额外耗时。
- **D022 dma.rs 移交 core**（2026-10-30，缩短关键路径）：acid2 用循环直接写 OAM，不用 DMA；宝可梦每帧在 VBlank 中调用 HRAM 里的 OAM DMA 例程（`refs/pokered/vblank.asm`）。为与 ppu 的 T41/T42/T43 并行，`crates/core/src/dma.rs` 与 `crates/core/tests/dma*.rs` 改归 core（冻结 API 不变），ppu 的 T42 只做精灵。
- **D023 W4 提前并行与隐藏集**：G 任务完成后空闲的通道可以提前做 W4（Mooneye、MBC2/MBC5 等），但必须放在单独的分支（`w4/<lane>` 或单独提交）上，integrator 在打 `v1.0-gates`、并用 T70 原样跑完 Mooneye（§4.5 隐藏集证据）之后才合并。合入 main 的 G 轨代码不得以 Mooneye ROM 作调试目标。如实记录：ppu 通道在开发中阅读过 Mooneye 的测试源码，把它当作硬件行为说明（R1 允许），REPORT 中要注明这一点。另外，`crates/core/src`（含注释）不得出现测试 ROM 名（T90 第 2 项），注释改为引用 Pan Docs 章节。
