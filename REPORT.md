# REPORT：从零实现的 GB（DMG）模拟器

> 题目：酒馆 Problem「Claude Opus 5.5 高速编程研究：从零写 GB 模拟器」（`docs/GB 模拟器高速编程研究 · 技术文档与资料.md`）。
> 计划与任务：`docs/PLAN.md`（v2）；决定记录：`DECISIONS.md`（D001–D0xx）；进度：`PROGRESS.md`。
> 所有数字都来自仓库中提交的运行证据（`reports/`），可以用 README 中的三条命令复现。

## 1. 结论

| 编号 | 项目 | 结果 | 证据 |
| --- | --- | --- | --- |
| G1 | Blargg `cpu_instrs`（11 个 individual 加合集） | **12/12** | `reports/gates.md`、`reports/blargg/*.png`（屏幕显示 `Passed`/`Passed all tests`） |
| G2 | Blargg `instr_timing` | **1/1** | 同上 |
| G3 | Blargg `mem_timing`（合集加 3 个 individual） | **4/4** | 同上 |
| G4 | `dmg-acid2` 与参考图逐像素比较 | **1/1**（差异 0 px） | `reports/acid2-pass.png` |
| G5 | 宝可梦红 30 分钟实玩（Sonnet 5.5 驾驶） | **PASS** | `reports/playtest/T62-rc-gates-7558ce3/`（summary、contact sheet、181 张采样截图） |
| B1 | Mooneye（DMG-ABC 适用的 acceptance/ 与 emulator-only/，94 个） | **94/94**（115 个原始结果中 95 个通过） | `reports/gates.md`、`reports/mooneye-scope.md` |
| — | 宝可梦确定性回放基线 `POKERED-REPLAY` | ok（15 个检查点，4500 帧） | `reports/pokered-replay.json`、`tools/pokered-replay-actions.json` |

- **`v1.0-gates`（= `rc-gates` = 7558ce3）**：G1–G5 全部完成。G5 的 30 分钟实玩就是在这个冻结构建上跑的。
- **main（最终）**：`G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 94/94 REPLAY ok`；`cargo test --workspace` 332 项通过，0 失败，0 忽略；`pytest python/tests` 54 项通过，包括需要整机的测试。
- **隐藏集差距（§4.5）**：在 v1.0-gates 上，改动前原样跑 Mooneye 为 **66/94**（raw 67/115，`reports/mooneye-raw-rc-gates/`，提交 0139161 早于任何针对 Mooneye 的合并）；同期必过项为 18/18。可见集 100%，隐藏集 70.2%，Δ ≈ 29.8pp。三个通道在 G 轨开发时都读过 Mooneye 测试源码（D034）；剔除直接对应的 9 项后，Δ = 100% − 57/85 ≈ 32.9pp。W4 之后 Mooneye 为 94/94。

## 2. 环境

- macOS（Darwin 27.0.0），15 核，48 GB 内存；rustc/cargo 1.93.0；uv 0.11.6；Python 3.12（`.venv`）；Claude Code 2.1.296。
- 模拟器核心与 cli 都是零依赖的 Rust。Python 侧只装了 pokemon-agent@f8a03e4 及其依赖（fastapi、uvicorn、Pillow 等），以及 pytest、requests。**没有安装 pyboy**。
- 参考资料由 `tools/fetch_refs.sh` 按固定提交下载，并校验 25 个 sha256（PLAN §2.1）。宝可梦红 ROM 的 SHA1 为 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`（`tools/check_pokered.sh`），ROM 不入库。

## 3. 实现概要

- **CPU**（`crates/core/src/cpu.rs`、`decode.rs`、`alu.rs`）
  - 按 M-cycle 驱动，每次总线访问或内部周期为 1 个 M-cycle，每个 M-cycle 先推进外设 4 T（D002）。
  - SingleStepTests 500/500 文件、50 万用例通过（HALT/STOP 两项部分比对，D021）。
  - 中断在取指之后检查（D030）；HALT bug、EI 延迟、ie_push 都已实现。
- **总线与外设**（`bus.rs`、`timer.rs`、`serial.rs`、`joypad.rs`、`interrupts.rs`）
  - 定时器按下降沿计数，写 DIV/TAC 也会触发计数，溢出后的重载流水线也已实现。
  - 串口由系统计数器 bit 8 驱动。
  - I/O 未使用位读 1。
  - `peek` 不受 PPU 访问封锁影响（D031）。
- **PPU**（`ppu.rs`、`ppu/render.rs`）
  - 逐行渲染，双缓冲；STAT 中断按上升沿触发；窗口 Y 锁存与内部行计数器；10 个精灵上限；8×16 精灵；DMG 优先级规则。
  - mode 3 长度可变，有 VRAM/OAM 访问封锁，LCD 开启后的第 0 行按硬件处理。
- **OAM DMA**（`dma.rs`）：1 个 M-cycle 启动延迟后传输 160 个 M-cycle，支持重启。
- **卡带**（`cart/`）：ROM only、MBC1（含 multicart）、MBC2、MBC3（RTC 寄存器只做存储）、MBC5。
- **存档**：magic `GBEMUST`，版本 3，含 ROM 指纹（PLAN §3.8）。读档失败时机器状态不变。
- **gb-cli**：`run`、`gates`（棘轮、日志、截图、对抗单测、Mooneye 计分、`POKERED-REPLAY`）、`bench`、`replay`。
- **接入**：`gb-ffi`（C ABI，每个入口都有 `catch_unwind`）→ `python/gbemu`（ctypes，全部原生调用加锁）→ pokemon-agent。提供两种接入方式（README）。

## 4. 实玩（G5）

**正式验收：T62，冻结构建 v1.0-gates（7558ce3）**（`reports/playtest/T62-rc-gates-7558ce3/`）

- 从开机、新建游戏开始，不加载存档，只调用一次 `/games/new`。`tools/drive_loop.sh` 每一轮开一个新的 `claude -p` 会话，用 Sonnet 5.5 high 驾驶，只允许 `curl localhost` 和 `Read`，共 30 分钟。
- **硬性判定 9/9**：
  - 共 181 次采样，服务器全程存活，`/health`、`/state`（200 且字段合法）、`/screenshot`（160×144 PNG）全部正常；
  - 172 次 `/action` 每次都推进了帧；
  - 经过 34 个不同的（地图, 坐标）；
  - 最长单色画面 10 秒（开机）。
- **驾驶**：共 23 个会话。22 轮完成，全部成功；第 23 轮在 30 分钟截止时被中断，rc-gates 版的 drive_loop 当时不登记这种轮次，已在 `driver-sessions-addendum.md` 补登。没有失败轮次，scripted 兜底 0 次。
- **推进路线**：
  1. 起名、自家二楼、一楼；
  2. 真新镇，被大木拦下；
  3. 大木研究所：拿到杰尼龟，战胜劲敌的妙蛙种子；
  4. 1 号道路：与小拉达、波波野外战斗，杰尼龟升到 7 级；
  5. 常青市，最后进入常青市宝可梦中心。
- **contact sheet 抽查**（integrator 逐格查看）：标题与开场、起名键盘、卧室、小镇、研究所、战斗画面（血条、菜单、精灵）、草地、城市、宝可梦中心都正常，没有花屏、错位或异常黑屏。5 张单色截图都是转场：开机、开场淡出、进战斗闪白、回血闪白。

**补充复核：W4 构建（503ffa9）再跑 30 分钟**（`reports/playtest/W4-w4-mooneye-94-503ffa9/`）

- 判定 PASS，硬性判定 9/9。
- 16 轮完整驾驶全部成功；最后 1 轮在结束时被截断，不算失败。经过 22 个不同的（地图, 坐标），拿到杰尼龟，正在与劲敌对战。
- 11 张单色截图全部是开场演讲的白色淡出。这几次采样之间帧数没有变化：服务端只在 `/action` 时推进帧，驾驶思考期间画面就停在淡出帧上。最长 30 秒，低于 60 秒的门槛。
- contact sheet 正常。

**B1：大木研究所卡死（结论：游戏自身的竞态，不是模拟器的问题；D032）**

- 一次 scripted 驾驶在研究所里遇到稳定可复现的"主角消失、输入失效"。
- scout 从游戏反汇编一侧（`reports/scout/oaks-lab-stall.md`）、core 从模拟器一侧，各自逐条追踪，结论一致：
  1. 那一帧音乐引擎开始新音符，VBlank 处理多跑了约 6000 T，`_UpdateSprites` 因此跨过了 VBlank；
  2. 此时有 11 个可见精灵，`PrepareOAMData` 没有上限检查，写穿 shadow OAM，污染了 `wTileMap`；
  3. 主角因此被隐藏，并在四个方向上都判定为碰撞，形成自锁。
- 模拟器侧核实：每帧恰好 70224 T，VBlank 落点在 ±8 T 以内稳定，HALT 到 ISR 为 5 个 M-cycle，DMA 不额外占用 CPU 周期。
- 两个构建都只有约 1/28 的单帧窗口。按 START 再按 B 就能恢复。
- 按 R6，核心代码不做针对性修补，只在驾驶侧加检测与恢复。正式实玩没有触发它。

## 5. Mooneye 计分

- 纳入规则（PLAN §2.4，D026）：只取 `acceptance/` 与 `emulator-only/`，加入无后缀、含 `G`、含 `dmgABC` 的；排除 `-dmg0/-mgb/-S/-sgb/-sgb2/-cgb*/-A/-C`，以及 `manual-only/`、`madness/`、`utils/`、`misc/`。共纳入 94 个，清单与理由见 `reports/mooneye-scope.md`。
- main：**94/94**。115 个原始结果中有 20 个未通过，全部在纳入范围外：
  - 其他机型的 boot_div/boot_hwio/boot_regs（dmg0、mgb、S、sgb、sgb2、A、cgb）；
  - `misc/` 的 CGB 测试；
  - `madness/mgb_oam_dma_halt_sprites`、`manual-only/sprite_priority`、`utils/bootrom_dumper`。
- 过程：
  - v1.0-gates 原样跑分为 66/94；
  - W4 的三个提前分支（w4/core、w4/sys、w4/ppu，D023/D030）与 T50b 合入后为 94/94；
  - 每一步都保持 G1–G4 与已通过项不回退。

## 6. 性能

`gb-cli bench roms/pokered.gb --frames 3600`（release）：

| 构建 | fps | 倍实时 |
| --- | --- | --- |
| main（W4，最终） | 8435–8661 | 141–145× |
| v1.0-gates | 11168 | 187× |

两个构建都超过 T55 的 600 fps 目标。W4 加入精确的 PPU 时序和访问封锁，带来约 22–25% 的开销。最终构建上 acid2 为 9448 fps，cpu_instrs 为 8061 fps。

## 7. 代理、模型与推理强度

模型按会话记录中的 `"model"` 字段核实（`~/.claude/projects/...`）。

| 代理 | 职责 | 模型（实测） | 推理强度 |
| --- | --- | --- | --- |
| integrator（主会话） | 冻结接口、派发、合并、基线、里程碑、攻关 | claude-opus-5-5（1M 上下文） | max |
| core | CPU、OAM DMA（D022）、W4 中断时序；B1 模拟器侧追踪 | claude-opus-5-5 | xhigh |
| sys | 总线、外设、整机与存档；W4 I/O 掩码与串口时钟 | claude-opus-5-5 | xhigh |
| ppu | PPU；W4 精确时序与访问封锁 | claude-opus-5-5 | xhigh |
| platform | 卡带、gb-cli/gates、Mooneye 计分、T52、T63、会话导出工具 | claude-opus-5-5 | high |
| play | FFI、Python 后端、启动器与补丁、实玩工具 | claude-opus-5-5 | high |
| scout | B1 游戏侧诊断（只读） | claude-opus-5-5 | high |
| reviewer | T80 评审与 T90 合规自查（新开上下文） | claude-opus-5-5 | high |
| driver | `claude -p` 每轮新会话驾驶 | claude-sonnet-5-5 | high |

说明：
- 本机网关把所有别名都映射到 Opus 5.5（D009），所以子代理都是派发时显式指定的 Opus。
- 自定义子代理类型在会话中途创建后不能被 Agent 工具识别（D008），因此都按 general-purpose 派发，并在简报里写入角色定义与"无联网"约束。
- clerk 未使用，机械事务由 integrator 和 platform 完成。
- 写代码的通道由 PLAN v2 的 3 个扩为 5 个（用户指令"多开子代理并行"，D004），之后又把 dma.rs 移交给 core（D022）。

## 8. 已知问题与偏离

1. **SST 的两项部分比对（D021）**：HALT（76）与 STOP（10）。SST 的约定与 Pan Docs / Opcodes.json 不一致，本实现以后两者为准，这两项只比较寄存器和内存。
2. **不做的部分**（PLAN §1.1）：声音（APU 寄存器只做存储，读出时加掩码）、CGB/SGB 模式、boot ROM 执行、链接线对传。SGB 头部标志被忽略，按 DMG 运行。
3. **pokemon-agent 的对话检测**：`/state` 里 `dialog.active` 只看 `wd730 & 0x20`，在开场演讲等场景下一直是 false。这是 pokemon-agent 自身的启发式，模拟器读出的内存是正确的；驾驶 prompt 里已经说明。
4. **驾驶效率**：每轮 60–170 秒，30 分钟内大约能推进到常青市。驾驶的推进速度取决于模型，不影响判定。
5. **隐藏集证据的保留意见（D023/D028）**：ppu 通道在 G 轨开发期间读过 Mooneye 的测试源码，当作硬件行为说明。R1 允许这样做，但会让 v1.0-gates 上"原样 66/94"的隐藏集意义打一些折扣。
6. **偏离 PLAN 之处**（都记在 DECISIONS 中）：
   - 写代码的通道由 3 个扩为 5 个（D004）；
   - dma.rs 移交 core（D022）；
   - W4 任务提前在单独分支上并行，v1.0-gates 之后才合并（D023）；
   - T52 用 `cherry-pick -x` 单独取入（D027）；
   - 子代理类型按 general-purpose 派发（D008）；
   - 实玩端口改用 8780 起，因为 8765 被本机其他进程占用（D024）；
   - SST 的实际格式与 PLAN §4.2 的描述不同（D015）。
7. **驾驶会话权限**：`--permission-mode dontAsk` 加 `Bash(curl *localhost*)` 时，Claude Code 仍然放行了一条 `curl …; curl … | head` 复合命令（`head` 是只读命令）。prompt 里已明确禁止，未发现越权的副作用。

## 9. 合规自查（T90）与评审（T80）

评审由新开上下文的 reviewer 完成，全文见 `reports/review.md`。它做了以下工作：
- 在本地克隆中分别构建 main 与 `v1.0-gates`，并跑了 `cargo test`、全量 gates 和 Mooneye；
- 逐条核对 PROGRESS 中的声明；
- 用原始数据重新计算 G5 的硬性判定；
- 抽查 2 次合并的 diff，检查有没有针对特定测试的代码（R6）；
- 执行 T90 的 6 项检查。

**复现结论**
- v1.0-gates：`G1 12/12 G2 1/1 G3 4/4 G4 1/1`，Mooneye 原样 66/94，与提交的证据逐项一致。
- main：Mooneye 94/94，`cargo test` 324 项通过，0 失败，0 忽略。
- 里程碑 tag 逐个复现，结果一致。
- G5 的 9 项硬性判定都有原始数据支撑，固定使用的 dylib 的 sha256 与 rc 构建一致。
- R6 抽查：抽了 W4 合并 ea586c2 与 G 轨合并 cf13ea1，没有发现针对特定测试的代码。

**T90 合规自查**

| # | 检查 | 结论 |
| --- | --- | --- |
| 1 | `git grep -niE 'pyboy\|sameboy\|gambatte\|mgba\|bgb' -- crates python tools` | 满足。`pyboy` 的命中都是"禁止导入 pyboy"的守卫代码和对应测试；`bgb` 的 8 处都是 `libgb_ffi` 的子串误命中。 |
| 2 | `git grep -niE 'acid\|blargg\|mooneye\|cpu_instrs\|pokemon\|pokered' -- crates/core/src` | 满足（结果为空）。 |
| 3 | `git ls-files \| grep -iE '\.(gb\|gbc\|sav\|state)$'` | 满足（结果为空）。 |
| 4 | reflog 中没有 rebase、amend、reset | 主仓库满足；没有改写或丢弃任何已提交的历史。各 worktree 里有 7 条 `reset: moving to HEAD`：6 条是 `git worktree add -b` 自带的记录；1 条是 sys 做试合并后 `--abort` 留下的，前后 HEAD 都是 8e57c35。T52 用了 `cherry-pick -x`（D027），这不改写历史。 |
| 5 | `.venv` 中没有 pyboy | 满足。 |
| 6 | core 中没有 `"Passed"`/`"Failed"`，也没有超过 64 项的大型常量表 | 满足。 |

**评审问题的处理**
1. 【高】三个通道都读过 Mooneye 源码：已补记 D034，`timer.rs` 与 `ppu.rs` 的出处注释已更正，REPORT 改为给出两种 Δ。
2. 【中】T62 第 23 轮未登记：已补 `driver-sessions-addendum.md`，并更正本报告中的轮次说法。
3. 【中】pytest 竞态：由 play 修复，并在空闲机器上全量连跑验证，见第 10 节。
4. 【中】实玩硬性判定抓不住长时间卡死：按 §4.4，判定标准不变；新增醒目的 `STALL` 告警，以及可选的 `--fail-on-stall`（play）。
5. 【中低】B1 复现材料只在 /tmp：已提交 `reports/scout/oaks-lab-stall-repro/`，按其 README 复现结果为 k=14 卡死（1/40），已实测。
6. 【低】T90 第 1 项的命中需要逐条说明：见上表。
7. 【低】worktree 的 reset 记录：见上表第 4 行。
8. 【低】部分 W4 PPU 时序常数没有注明出处：并入 D034 的说明。存档指纹读取 ROM 标题是 PLAN §3.8 的规定，core 不依据标题改变行为，不属于 R2 意义上的"感知"。
9. 【低】PROGRESS 过期：已更新。
10. 【低】G5 用的 dylib 无法逐位重建：构建产物会嵌入绝对路径。T62 固定使用的 `libgb_ffi.dylib` 已入库，路径为 `reports/playtest/T62-rc-gates-7558ce3/gbemu-lib/`，sha256 为 4c1bff22…。

## 10. 交付清单（PLAN §7）

| # | PLAN §7 要求 | 状态 | 验证 |
| --- | --- | --- | --- |
| 1 | git 仓库保留完整历史；`v1.0-gates` 打在通过 T62 实玩的提交上 | ✅ | `git show v1.0-gates` 指向 7558ce3，与 `reports/playtest/T62-rc-gates-7558ce3/summary.md` 中的 `git_head` 一致。全部为小步提交加 `--no-ff` 合并，没有 rebase、amend 或 force（T90 第 4 项） |
| 2 | REPORT.md、`reports/gates.md`、`reports/baseline.json`、实玩 summary 与 contact sheet、`reports/review.md` | ✅ | 均已入库：实玩报告有 T62、W4 复核，以及两次 T61 冒烟 |
| 3 | `sessions/`：导出全部相关会话记录，附 INDEX.md，导出前扫描密钥并打码 | ✅ | 收录本项目实施会话 `0c37ec57`（主会话与 7 个子代理），以及 T62 正式实玩的 23 个 `claude -p` 驾驶会话（04:24 快照，由 `tools/export_sessions.py` 导出）。密钥只打码，不删除，见 `sessions/INDEX.md` |
| 4 | README：三条命令复现（构建、gates、实玩）与两种接入方式 | ✅ | `README.md` |
| 5 | `git ls-files \| grep -iE '\.(gb\|gbc\|sav\|state)$'` 为空 | ✅ | T90 第 3 项 |

主要 tag：
- `m1-g1`、`m2-timing`：9416633
- `m3-acid2`：8803312
- `rc-gates` = `v1.0-gates`：7558ce3
- `w4-mooneye-94`：503ffa9
- `delivery`：交付提交

时间点以 git 历史为准。按 PLAN v2，不另做计时与耗时记录。

## 附录 A：Mooneye 逐 ROM 结果（115 个；94 个纳入）

| ROM | 纳入 | v1.0-gates（原样） | main（最终） |
| --- | --- | --- | --- |
| `acceptance/add_sp_e_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/bits/mem_oam.gb` | 是 | 通过 | 通过 |
| `acceptance/bits/reg_f.gb` | 是 | 通过 | 通过 |
| `acceptance/bits/unused_hwio-GS.gb` | 是 | — | 通过 |
| `acceptance/boot_div-dmgABCmgb.gb` | 是 | 通过 | 通过 |
| `acceptance/boot_hwio-dmgABCmgb.gb` | 是 | 通过 | 通过 |
| `acceptance/boot_regs-dmgABC.gb` | 是 | 通过 | 通过 |
| `acceptance/call_cc_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/call_cc_timing2.gb` | 是 | 通过 | 通过 |
| `acceptance/call_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/call_timing2.gb` | 是 | 通过 | 通过 |
| `acceptance/di_timing-GS.gb` | 是 | 通过 | 通过 |
| `acceptance/div_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/ei_sequence.gb` | 是 | 通过 | 通过 |
| `acceptance/ei_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/halt_ime0_ei.gb` | 是 | 通过 | 通过 |
| `acceptance/halt_ime0_nointr_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/halt_ime1_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/halt_ime1_timing2-GS.gb` | 是 | 通过 | 通过 |
| `acceptance/if_ie_registers.gb` | 是 | 通过 | 通过 |
| `acceptance/instr/daa.gb` | 是 | 通过 | 通过 |
| `acceptance/interrupts/ie_push.gb` | 是 | 通过 | 通过 |
| `acceptance/intr_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/jp_cc_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/jp_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/ld_hl_sp_e_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/oam_dma/basic.gb` | 是 | 通过 | 通过 |
| `acceptance/oam_dma/reg_read.gb` | 是 | 通过 | 通过 |
| `acceptance/oam_dma/sources-GS.gb` | 是 | — | 通过 |
| `acceptance/oam_dma_restart.gb` | 是 | 通过 | 通过 |
| `acceptance/oam_dma_start.gb` | 是 | 通过 | 通过 |
| `acceptance/oam_dma_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/pop_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/ppu/hblank_ly_scx_timing-GS.gb` | 是 | — | 通过 |
| `acceptance/ppu/intr_1_2_timing-GS.gb` | 是 | 通过 | 通过 |
| `acceptance/ppu/intr_2_0_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/ppu/intr_2_mode0_timing.gb` | 是 | — | 通过 |
| `acceptance/ppu/intr_2_mode0_timing_sprites.gb` | 是 | — | 通过 |
| `acceptance/ppu/intr_2_mode3_timing.gb` | 是 | — | 通过 |
| `acceptance/ppu/intr_2_oam_ok_timing.gb` | 是 | — | 通过 |
| `acceptance/ppu/lcdon_timing-GS.gb` | 是 | — | 通过 |
| `acceptance/ppu/lcdon_write_timing-GS.gb` | 是 | — | 通过 |
| `acceptance/ppu/stat_irq_blocking.gb` | 是 | 通过 | 通过 |
| `acceptance/ppu/stat_lyc_onoff.gb` | 是 | — | 通过 |
| `acceptance/ppu/vblank_stat_intr-GS.gb` | 是 | 通过 | 通过 |
| `acceptance/push_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/rapid_di_ei.gb` | 是 | 通过 | 通过 |
| `acceptance/ret_cc_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/ret_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/reti_intr_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/reti_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/rst_timing.gb` | 是 | 通过 | 通过 |
| `acceptance/serial/boot_sclk_align-dmgABCmgb.gb` | 是 | — | 通过 |
| `acceptance/timer/div_write.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/rapid_toggle.gb` | 是 | — | 通过 |
| `acceptance/timer/tim00.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim00_div_trigger.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim01.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim01_div_trigger.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim10.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim10_div_trigger.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim11.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tim11_div_trigger.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tima_reload.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tima_write_reloading.gb` | 是 | 通过 | 通过 |
| `acceptance/timer/tma_write_reloading.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/bits_bank1.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/bits_bank2.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/bits_mode.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/bits_ramg.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/multicart_rom_8Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc1/ram_256kb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/ram_64kb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/rom_16Mb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/rom_1Mb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/rom_2Mb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/rom_4Mb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/rom_512kb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc1/rom_8Mb.gb` | 是 | 通过 | 通过 |
| `emulator-only/mbc2/bits_ramg.gb` | 是 | — | 通过 |
| `emulator-only/mbc2/bits_romb.gb` | 是 | — | 通过 |
| `emulator-only/mbc2/bits_unused.gb` | 是 | — | 通过 |
| `emulator-only/mbc2/ram.gb` | 是 | — | 通过 |
| `emulator-only/mbc2/rom_1Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc2/rom_2Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc2/rom_512kb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_16Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_1Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_2Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_32Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_4Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_512kb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_64Mb.gb` | 是 | — | 通过 |
| `emulator-only/mbc5/rom_8Mb.gb` | 是 | — | 通过 |
| `acceptance/boot_div-S.gb` | 否：suffix -S does not cover DMG-ABC | — | — |
| `acceptance/boot_div-dmg0.gb` | 否：suffix -dmg0 does not cover DMG-ABC | — | — |
| `acceptance/boot_div2-S.gb` | 否：suffix -S does not cover DMG-ABC | — | — |
| `acceptance/boot_hwio-S.gb` | 否：suffix -S does not cover DMG-ABC | — | — |
| `acceptance/boot_hwio-dmg0.gb` | 否：suffix -dmg0 does not cover DMG-ABC | — | — |
| `acceptance/boot_regs-dmg0.gb` | 否：suffix -dmg0 does not cover DMG-ABC | — | — |
| `acceptance/boot_regs-mgb.gb` | 否：suffix -mgb does not cover DMG-ABC | — | — |
| `acceptance/boot_regs-sgb.gb` | 否：suffix -sgb does not cover DMG-ABC | — | — |
| `acceptance/boot_regs-sgb2.gb` | 否：suffix -sgb2 does not cover DMG-ABC | — | — |
| `madness/mgb_oam_dma_halt_sprites.gb` | 否：directory madness/ is not acceptance/ or emulator-only/ | — | — |
| `manual-only/sprite_priority.gb` | 否：directory manual-only/ is not acceptance/ or emulator-only/ | — | — |
| `misc/bits/unused_hwio-C.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/boot_div-A.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/boot_div-cgb0.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/boot_div-cgbABCDE.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/boot_hwio-C.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/boot_regs-A.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/boot_regs-cgb.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `misc/ppu/vblank_stat_intr-C.gb` | 否：directory misc/ is not acceptance/ or emulator-only/ | — | — |
| `utils/bootrom_dumper.gb` | 否：directory utils/ is not acceptance/ or emulator-only/ | — | — |
| `utils/dump_boot_hwio.gb` | 否：directory utils/ is not acceptance/ or emulator-only/ | 通过 | 通过 |
