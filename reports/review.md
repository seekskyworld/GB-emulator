# T80 评审 + T90 合规自查（reviewer，新开上下文）

- 评审时间：2026-10-30 02:36–03:15（+0800）
- 评审对象：克隆时 main 的 HEAD 为 **`ac7e5718`**（`PROGRESS: B1 closed (D032)`），以及 `v1.0-gates` = `7558ce3d`。
- 评审期间 main 继续前进：684adb0（T63）→ e63801f（W4 复核实玩）→ 7a41b99（D033）。
  - 从 e63801f 到 7a41b99，`crates/` 没有任何改动。
  - 我对 e63801f 补跑了 cargo test 与全量 gates，对 7a41b99 补跑了 pytest，结果见 §2.1 与 §2.3。
- 环境：
  - 本地克隆：`git clone --no-hardlinks $REPO /tmp/gbemu-review/repo`，未联网。
  - 在克隆内用 `git worktree add --detach` 检出各版本。
  - 每个版本使用独立的 `CARGO_TARGET_DIR=/tmp/gbemu-review/target-<名>`。
  - `GBEMU_REFS=$REPO/refs`，`GBEMU_POKERED=$REPO/roms/pokered.gb`，cargo 一律加 `--offline`。
- 没有碰端口 8790，也没有碰其他通道的进程。本次只写了本文件，不提交。

---

## 0. 结论

### 0.1 T80：声明复核

| # | 声明 | 结论 |
| --- | --- | --- |
| C1 | v1.0-gates：`G1 12/12 G2 1/1 G3 4/4 G4 1/1`，Mooneye 原样 66/94 | **已复现**（66/94，raw 67/115）。与 `reports/mooneye-raw-rc-gates/gates.json` 逐项对比：133 项，0 处差异 |
| C2 | main：`G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 94/94` | **已复现**（94/94，raw 95/115）。与 `reports/gates.json` 逐项对比：133 项，0 处差异 |
| C3 | `cargo test --workspace` 全绿 | **已复现**。main：324 通过 / 0 失败 / 0 忽略；v1.0-gates：287 / 0 / 0。没有任何 SKIP |
| C4 | `cargo build --release` | **已复现**（两个版本都成功） |
| C5 | PROGRESS 中标为 done 的各任务 | 逐条见 §0.3：**全部已复现，只有三处例外**：T60 在 ac7e571 上有 1 个 pytest 用例稳定失败（P3）；T71 的"REPORT.md 列出每个 ROM"尚未满足（属 T91）；T61 的报告目录在 ac7e571 时不在仓库里，我用自己跑的冒烟代替 |
| C6 | 里程碑 M1/M2/M3（tag m1-g1、m2-timing、m3-acid2）、M5（v1.0-gates）、M6（w4-mooneye-94） | **已复现**（逐个 tag 构建、测试并跑 gates，见 §2.4） |
| C7 | G5 证据 `T62-rc-gates-7558ce3` 的 9 项硬性判定 | **已复现**：我从 `samples.jsonl` 重新计算，9 项都有原始数据支撑；contact sheet 没有花屏、错位或异常黑屏；git_head 与 v1.0-gates 一致；固定的 dylib 的 sha256 与 `lanes/rc` 构建一致 |
| C8 | "22/22 轮驾驶" | **未复现（数字不完整）**：实际还有被时间截断的第 23 轮，执行了 3 次 `/action`（169→172），它的会话 ID 没有写进 `driver-sessions.txt`（P2） |
| C9 | §4.5 隐藏集：必过项 18/18，v1.0-gates 上 Mooneye 66/94，在任何针对 Mooneye 的合并之前生成 | **数字与拓扑已复现**：0139161 与 v1.0-gates 之间没有任何合并，没有任何 w4 提交是 v1.0-gates 的祖先。**但"未受 Mooneye 影响"不成立**：G 轨代码参考了 Mooneye 测试源码，D023/D028 只记了 ppu 一个通道，实际涉及 core、sys、ppu 三个通道，对应 v1.0-gates 上的 9 个通过项（P1） |
| C10 | B1 / D032：大木研究所卡死是游戏自身的竞态 | **机理已复现，结论基本成立，有保留**（见 §2.8）：我在反汇编中逐条核对了因果链；在 T62 构建上 k=14 卡死、在我自己构建的 W4 上 k=22 卡死，各为 1/40；自己跑的 5 分钟冒烟里也独立出现了同一特征。保留之处：命中频率依赖整帧周期精度，没有真机对照；复现材料只在 /tmp（P5） |
| C11 | R6 抽查（W4 合并 ea586c2，G 轨合并 cf13ea1） | **未发现针对特定测试的代码**：没有按 ROM 判断的分支、没有按测试写死的时序、没有只为某个测试存在的路径。W4 中部分 PPU 时序常数没有出处（P8，低） |

### 0.2 T90：合规自查（在 main ac7e571 上执行，e63801f 上复查结果相同）

| # | 项目 | 结论 |
| --- | --- | --- |
| 1 | `git grep -niE 'pyboy\|sameboy\|gambatte\|mgba\|bgb' -- crates python tools` | **满足（附说明）**。没有引用、安装或使用任何其他模拟器。命中分两类：<br>• `pyboy`：阻止导入 PyBoy 的守卫代码、`pyboy_loaded` 统计字段，以及断言 PyBoy 未加载的测试；<br>• `bgb`：全部是 `libgb_ffi` 字符串的误命中。<br>字面上，守卫代码不属于"说明性注释或接口名"，需要在 REPORT 中注明（P6） |
| 2 | `git grep -niE 'acid\|blargg\|mooneye\|cpu_instrs\|pokemon\|pokered' -- crates/core/src` 为空 | **满足**（空）。注意：这是 6331d72、96ac7a9 删掉测试名注释之后的结果，见 P1 |
| 3 | `git ls-files \| grep -iE '\.(gb\|gbc\|sav\|state)$'` 为空 | **满足**（空） |
| 4 | 历史中没有 rebase、amend、reset | **主仓库满足；worktree 字面不满足，实质满足**。<br>• 主仓库 reflog 52 条，没有 rebase、amend、reset；<br>• 有 1 条 cherry-pick（D027 的 T52，带 `-x` 标注，不算改写历史）；<br>• 各 worktree 的 reflog 共有 7 条 `reset: moving to HEAD`：6 条是 `git worktree add -b` 自带的（已用实验验证），1 条在 sys（01:34:31，HEAD 前后都是 8e57c35，没有丢失已提交内容）；<br>• 全库没有不可达提交，唯一一对同补丁提交就是那次 cherry-pick（P7） |
| 5 | venv 中没有 pyboy | **满足**：`.venv` 里没有 pip；`uv pip list --python .venv/bin/python` 中没有 pyboy；site-packages 里也没有 pyboy；`find_spec('pyboy')` 返回 None |
| 6 | core 中没有 `"Passed`/`"Failed`；超过 64 项的常量数组只能是 opcode 周期表 | **满足**。`"(Passed\|Failed)` 为空。core 中最大的常量数组是 `APU_READ_OR`（32 项），其次是开机表（21 对）。core 里没有 opcode 周期表：周期由总线访问序列自然产生，并由 `tests/opcode_table.rs` 对照 Opcodes.json、`tests/sst.rs` 对照 50 万个 SST 用例校验 |

### 0.3 PROGRESS 中 done 任务逐条核对

| 任务 | 结论 | 依据（本次亲自执行） |
| --- | --- | --- |
| T00 | 已复现 | `import pokemon_agent.emulator, gbemu` 后 pyboy 未加载；`reports/model-probe.md` 中 4 个 ID 都返回 200；pokered 的 SHA1 为 `ea9bcae6…` |
| T01 | 已复现 | individual 共 11 个；mooneye `.gb` 共 115 个；`reference-dmg.gray` 的 sha256 为 `cb26ef61…`；`check_pokered.sh` 退出码 0 |
| T02 | 已复现 | 构建通过 |
| T05a / T05b | 已复现 | judge_adversarial 13、regression 9、png_pillow 1、gates_manifest 6 全部通过；棘轮实测：用 main 的基线跑 v1.0 构建，退出码 2，28 条 REGRESSION |
| T10 / T11 / T12 | 已复现 | opcode_table 10、alu 21 全部通过；`SST: 500/500 files passed (500000 cases); partial-check exceptions: ["10", "76"]` |
| T13 / T14 / T15 / T16 / T51 | 已复现 | bus 19、cpu_interrupts 22、interrupts 8、timer 24、serial 13、joypad 11 全部通过 |
| T17 / T30 | 已复现 | G1 12/12、G2 1/1、G3 4/4（gates）；`gb-cli run` 单独跑合集，也独立看到 `Passed all tests` |
| T40 / T41 / T42 / T43 | 已复现 | ppu_timing 17、ppu_bg 17、ppu_obj 17、dma 7、dma_machine 1 全部通过；G4 1/1，逐像素一致 |
| T50 / T50b | 已复现 | cart_mbc1 13、mbc2 9、mbc3 8、mbc5 9、cart_common 7 全部通过；Mooneye emulator-only 的 MBC 项在 main 上全部通过 |
| T52 | 已复现 | state_roundtrip 5 个通过（pokered 与 cpu_instrs 实际运行，不是 SKIP） |
| T53a / T53b / T54 | 已复现 | v1.0-gates：pytest 49/49（设了 `GBEMU_REQUIRE_MACHINE=1`）；补丁对 refs/pokemon-agent 的副本执行 `patch -p1 --dry-run`，退出码 0 |
| T60 | **在 ac7e571 上未复现** | pytest 49 通过、1 失败，失败项是 `test_drive_loop_records_interrupted_round`：全量连跑 3 次都失败，单独跑能通过。7a41b99 中的 112d225 已修复这个竞态，但全量连跑 7 次仍有 1 次失败（P3） |
| T61 | 已复现（用我自己的冒烟代替） | v1.0-gates 构建上 `playtest.py --minutes 5 --driver script` 退出码 0，坐标发生过变化。**但 9.7 秒后就进入了 B1 卡死，之后 4 分 50 秒一直不动，判定仍是 PASS**（P4） |
| T62 / G5 | 已复现，有一处计数问题 | 见 §2.6；"22/22 轮"的问题见 P2 |
| T55 | 已复现 | 实测 `bench pokered --frames 3600`：main 9016.7 fps，v1.0 10631.6 fps，都远高于 600 fps 的目标 |
| T70 | 已复现 | mooneye_scope 5 个通过；纳入/排除规则与 PLAN §2.4 一致：acceptance 中排除的 9 项都是其他机型 |
| T71 | 94/94 已复现；完成标准未满足 | 完成标准要求 REPORT.md 列出每个 ROM 的结果，REPORT.md 尚不存在（属 T91；`reports/gates.md` 已有逐项结果） |
| T61b skipped；T63、T80、T90、T91 todo | — | T63 已在 684adb0 合入 main，但截至 7a41b99，PROGRESS 仍写 todo（P9） |

---

## 1. 问题清单（按严重程度排序）

### P1【高】§4.5 隐藏集证据受 Mooneye 源码影响的范围被低估，代码中的出处也被抹去

- **现象**：v1.0-gates 的 G 轨代码在开发时参考了 Mooneye 测试源码。DECISIONS 只记录了 ppu 一个通道（D023/D028），实际涉及三个通道。证据是 core/src 的历史中，后来被删掉的注释：

  | 提交 | 通道 | 当时注释引用的 Mooneye 测试 |
  | --- | --- | --- |
  | 2147f44（T10，core） | core | `ie_push`、`halt_ime0/1 timing` |
  | 639e968（T40，ppu） | ppu | `stat_irq_blocking`、`vblank_stat_intr-GS`、`stat_lyc_onoff`、`lcdon_timing-GS`、`boot_hwio-dmgABCmgb`（开机行内位置 4740 dot） |

- **删除记录**：
  - 6331d72 的提交说明是 "comments no longer name test ROMs ... behaviour unchanged"；
  - 96ac7a9 同样删掉了 ppu 的这些引用。
- **sys 通道**：`timer.rs` 的 `BOOT_COUNTER` 注释写的是"取实机测得的值：第 14 个 M-cycle 的访问恰好在 DIV 加 1 之后读到 0xAC"。这个值实际来自 Mooneye `boot_div-dmgABCmgb.s` 的源码说明（"This read should happen immediately after DIV has incremented"），Pan Docs 里没有这条内容。"实机测得"的说法有误导性。
- **对证据的影响**：v1.0-gates 的 66 个 Mooneye 通过项中，以下 9 项与上述参考直接对应：
  - `boot_div-dmgABCmgb`、`boot_hwio-dmgABCmgb`
  - `halt_ime0_ei`、`halt_ime0_nointr_timing`、`halt_ime1_timing`、`halt_ime1_timing2-GS`
  - `interrupts/ie_push`
  - `ppu/stat_irq_blocking`、`ppu/vblank_stat_intr-GS`

  这几项不能再算"隐藏集"。差距 Δ 的几种算法：
  - 原始：18/18 = 100% 对 66/94 = 70.2%，**Δ = 29.8 pp**。
  - 剔除上述 9 项：57/85 = 67.1%，**Δ = 32.9 pp**。
  - 另外有 16 个失败项只是 v1.0 不支持 MBC2/MBC5 卡带（属于功能缺口，不属于"特性孤立"）。把这 16 项也剔除：57/69 = 82.6%，Δ = 17.4 pp。
- **拓扑**：本身是干净的。0139161 直接位于 v1.0-gates 之后，中间只有 53fa727、1fa94b5 两个文档提交，没有合并。针对 Mooneye 的第一次合入发生在 ea586c2（02:23）。v1.0-gates 的祖先中只有 T70 的计分工具 b391e3e，它不改动 core。
- **修复建议**：
  1. 新增一条 DECISIONS，完整列出三个通道、9 个 ROM，以及对应的提交（2147f44、8388811、639e968）。
  2. REPORT 中同时给出原始 Δ 和剔除后的 Δ。
  3. 把 `timer.rs` 的"实机测得"改为如实说明出处（指向这条 DECISIONS），不在 core 中写测试名，以免违反 T90 第 2 项。`ppu.rs` 中的 `BOOT_DOT` 同样处理。

### P2【中】G5 实玩第 23 轮的会话没有登记，"22/22 轮"不准确（R4）

- **证据**：
  - `driver-rounds/round-023.prompt.md` 存在，`round-023.json` 是空文件；
  - `driver-sessions.txt` 只登记了 22 个会话；
  - `samples.jsonl` 中 `actions_total` 在 t=1780 s 时为 169，在 t=1800 s 时为 172，而 22 轮登记的动作数之和是 169。所以第 23 轮执行了 3 次 `/action`。
- **会话文件**：`~/.claude/projects/-…-GB-emulator-lanes-rc/` 下共有 23 个 jsonl。登记的 22 个都存在；另有 1 个未登记：`62544168-f576-4dd8-a6e0-eeac7e950406.jsonl`，修改时间 02:21:48，即第 23 轮。
- **影响**：
  - 不影响 PASS：scripted 驾驶 0 秒，9 项判定不变。
  - 但 summary、PROGRESS、d5fc40e 提交说明中的"22/22"都漏掉了这一轮。
  - 112d225（7a41b99 中）已在轮次开始时就写入会话 ID，但这只对以后的实玩有效。
- **修复**：
  - 在 T62 目录追加一个补充说明文件（不改动已提交的证据），写明第 23 轮的会话 ID；
  - 在 sessions/INDEX.md 中注明；
  - REPORT 改写为"22 轮完成，另有 1 轮被时间截断（3 次 action）"。

### P3【中】ac7e571 上 pytest 有 1 个用例稳定失败；最新 main 仍偶发失败

- **ac7e571**：`test_tools.py::test_drive_loop_records_interrupted_round`
  - 全量 pytest 连跑 3 次都失败：`assert (1 == 2)`，`driver-sessions.txt` 只有表头；drive_loop 在第 1 轮开始之前就收到了 SIGINT。
  - `-k "fallback or interrupted"` 失败；单独运行，或与 round_timeout 测试同跑时通过。
  - 结论：依赖执行顺序和时序（测试里固定 `sleep 3` 后就发 SIGINT）。
- **7a41b99**（含 112d225 "fix racy interrupted-round test"）：全量连跑 7 次，6 次 51/51 通过，1 次"1 failed, 50 passed"。那次与我的 5 分钟实玩并发，失败项名没有截取到。
- **修复**：测试改为等待 `round-001.prompt.md`（或会话行）出现之后再发信号；T91 之前在空闲机器上全量连跑 10 次，确认全部通过。

### P4【中】实玩的硬性判定抓不住"开局即卡死"，冒烟 PASS 的分量不足

- **我做的复现**：在 v1.0-gates 构建上执行 `tools/playtest.py --minutes 5 --driver script --port 8811`，退出码 0，判定 PASS。实际情况：
  - t=9.7 s 起，主角一直停在 (map 40, 5, 5)，直到结束；
  - contact sheet 中 1–5 分钟的每一张都看不到主角；
  - 回放 driver-actions 后读内存，正是 B1 的特征：`D5F0=6 C45C=a0 C434=a0 C102=ff C109=04 CFC4=00`。
- **原因**：硬性判定只要求"全程至少一次坐标变化"，"10 分钟坐标不变"也只是告警。
- **对 G5 的影响**：T62 本身不受影响。30 分钟内有 34 个不同的 (map,x,y)；最长不动的区间是 13:10–19:40，期间在战斗中，属于正常推进。但 T61 的冒烟、以及以后的回归，都可能"PASS 着卡死"。
- **修复**：在 summary 中把"非战斗、非对话状态下坐标不变超过 N 分钟"或"出现 B1 特征"提升为硬性失败，或至少作为显著告警；REPORT 中如实说明这一局限。

### P5【中低】B1（D032）的证据：机理成立，但复现材料不在仓库内，频率结论缺少真机对照

- **已核实**（ROM/pokered-src）：
  - `wShadowOAM`=C300、`wShadowOAMEnd`=`wTileMap`=C3A0（pokered.sym）；
  - `PrepareOAMData` 遍历 16 个槽，没有上限检查；
  - 清理循环用 `cp l / ret z` 判断 `l==$A0`，起点为 $B0 时会一直写到 C49C；
  - `UpdateSprites` 只检查 `wUpdateSpritesEnabled`，没有任何机制阻止 VBlank 在 `_UpdateSprites` 中途插入；
  - VBlank 中调用 `PrepareOAMData`（`home/vblank.asm`）。
- **已复现**：用 scout 的 `race.py`（复制到我的目录后用 `python -P` 运行）：
  - T62 的 dylib 配 `T62-4076.state`：卡死点 `[14]`（1/40）；
  - 我自己构建的 main（core 与 ea586c2 相同）配 `s4076.state`：卡死点 `[22]`（1/40）；
  - START→B 能恢复：我没有单独复验。
- **不足**：
  1. 两个 `.state` 起点都由 scout 生成，存放在 `/tmp/gbemu-scout`，不在仓库里，/tmp 被清理后就无法复现；
  2. "约 1/28 的单帧窗口"与真机是否一致没有对照。整帧 CPU 周期由 instr_timing、Mooneye 的 *_timing、halt、dma 测试间接约束，这是支持的一面；
  3. main 上没有针对 B1 的回归测试，只有驾驶侧恢复（5a04c50，ac7e571 时尚未合入 main，现已在 f09fe2e 合入）。
- **评价**：判为游戏竞态的证据**基本充分**。按 R6 不在模拟器侧修补是正确的。
- **修复**：把最小复现提交到 `reports/scout/`：driver-actions 截取到第 4078 条、`race.py`、预期输出；REPORT 中注明 scout 的保留意见。

### P6【低】T90 第 1 项的命中需要在 REPORT 中逐条说明

- `python/gbemu/launch.py` 中 `_BlockPyboy` 的守卫与断言、`pyboy_loaded` 字段、`tools/playtest.py:537`、`python/tests/*` 中断言 PyBoy 未加载的测试：这些都是"禁止 PyBoy"的代码，不是使用 PyBoy，但字面上不属于"说明性注释或接口名"。
- `bgb` 的 8 处命中全部是 `libgb_ffi`。
- 建议：REPORT 把这一类明确列为允许，或者把 grep 改为 `\bbgb\b`。

### P7【低】worktree reflog 中有 7 条 `reset: moving to HEAD`

- 其中 6 条（core、platform、play、ppu、sys 在 00:59:37，integ 在 02:05:36）都紧跟在空消息的创建记录之后。我在 `/tmp` 实验确认，这是 `git worktree add -b` 的固有记录。
- sys 在 01:34:31 的那条发生在 8e57c35 提交之后 12 秒，HEAD 没有变化，像是 `git stash` 或 `reset --hard HEAD`。这只可能丢弃未提交的改动，已提交内容没有丢失：8e57c35 是 w4/sys 的祖先，全库也没有不可达提交。
- 建议：请 sys 说明这次操作，写进 REPORT 的合规自查一节。

### P8【低】R6 抽查中发现的说明性缺口

- W4 的 PPU 常数没有出处：`MODE3_VISIBLE_DOTS=169`、`MODE0_LAG_DOTS=3`、`VRAM_LOCK_DOT=80`/`MODE3_DOT=84`、第 0 行 452 dot。它们对每一行、每个程序都同样生效，不属于针对单个测试的写死时序，但没有注明来源（推测是按 Mooneye ppu 时序标定的，T71 允许这样做）。建议在 DECISIONS 中注明。
- `mode3_penalty` 与 Pan Docs `Rendering.md` 的 "OBJ penalty algorithm" 逐条一致；MBC1M 的判定与 Pan Docs `MBC1.md` 的 "identified by having a Nintendo copyright header in bank $10" 一致。
- core 会读取 ROM 标题和校验和来生成存档指纹（`machine.rs`）。这是 PLAN §3.8 明确要求的，不影响仿真行为。建议在 REPORT 中注明这是 R2 的例外。

### P9【低】PROGRESS 与实际不一致

截至 ac7e571：

- B1 一行写"驾驶侧加恢复"，但恢复代码 5a04c50 当时不在 main 上；
- "下一步"仍写"B1 根因与修复（G 轨，进 main 并补回归测试）"，与 D032 矛盾；
- 截至 7a41b99，T63 仍为 todo，但它已在 684adb0 合入；
- "22/22 轮"的问题见 P2。

建议在 T91 之前按 gates.json 和 git 历史重新生成 PROGRESS。

### P10【低】G5 构建的来源证明依赖仓库之外的文件

- 固定的 dylib 没有入库；我从 `lanes/rc/target` 核对到 sha256 都是 `4c1bff22…`。
- Rust 构建产物会嵌入路径，所以我在 `/tmp` 重新构建的 v1.0 dylib hash 不同（`64d0a9b1…`），无法做到逐位复现。
- 建议：REPORT 中写明 rustc 版本、构建路径和 sha256，并在交付前保留 `lanes/rc` 的构建产物。

---

## 2. 证据

### 2.1 干净 checkout、构建与测试

```
$ git clone --no-hardlinks $REPO /tmp/gbemu-review/repo
$ git -C /tmp/gbemu-review/repo rev-parse HEAD v1.0-gates^{commit}
ac7e5718262296114c1c596d7404959eda5d083b
7558ce3d923880c86de7f68c1294dda672a8774b
$ git -C /tmp/gbemu-review/repo worktree add --detach /tmp/gbemu-review/v1 v1.0-gates
# 脚本 /tmp/gbemu-review/scripts/run.sh：
#   设置 CARGO_TARGET_DIR=/tmp/gbemu-review/target-<名> 及上述环境变量后依次执行：
#   cargo test --workspace --offline
#   cargo build --release --offline
#   gb-cli gates --mooneye --out /tmp/gbemu-review/gates-<名>
```

| 版本 | cargo test（各 `test result` 求和） | build | gates 第一行 | gates 退出码 |
| --- | --- | --- | --- | --- |
| main ac7e571 | 通过 324 / 失败 0 / 忽略 0 | ok | `G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 94/94` | 0 |
| v1.0-gates 7558ce3 | 通过 287 / 失败 0 / 忽略 0 | ok | `G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 66/94` | 1（只有 Mooneye 失败项） |
| main e63801f（补跑） | 通过 332 / 失败 0 / 忽略 0 | ok | `G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 94/94` | 0 |

- SST 以 `--nocapture` 运行：`SST: 500/500 files passed (500000 cases); partial-check exceptions: ["10", "76"]`。
- `cargo test --workspace -- --nocapture | grep -i skip` 在两个版本上都没有 SKIP，ROM 集成测试都实际运行了（pokered 的 SHA1 为 `ea9bcae617fdf159b045185467ae58b2e4a48b9a`）。
- 与提交进仓库的结果逐项对比（`/tmp/gbemu-review/scripts/cmp.py`，按 rom 比较 pass 与 reason）：

```
v1 vs reports/mooneye-raw-rc-gates/gates.json : entries 133 133, diffs 0  (MOONEYE 66/94, raw 67/115)
main vs reports/gates.json                    : entries 133 133, diffs 0  (MOONEYE 94/94, raw 95/115)
```

- main 上未通过的 20 项全部在纳入范围之外，包括：
  - acceptance 中的 `-S`、`-dmg0`、`-mgb`、`-sgb`、`-sgb2`；
  - `madness/`、`manual-only/`、`misc/`、`utils/` 目录。
- v1.0 上 28 个纳入项失败：16 个是 `unsupported cartridge type`（MBC2/MBC5），其余是 8 个 ppu、`unused_hwio-GS`、`boot_sclk_align`、`rapid_toggle`、`multicart_rom_8Mb`。
- 独立于 gates 的单 ROM 运行（`gb-cli run`，main 构建）：
  - `cpu_instrs.gb --frames 4000 --serial` 输出 `Passed all tests`；
  - `instr_timing.gb` 输出 `Passed`；
  - `mem_timing.gb` 输出 `Passed all tests`；
  - Mooneye `intr_2_mode0_timing_sprites` 在 LD B,B 处 `B=03 C=05 D=08 E=0D H=15 L=22`。
- 棘轮：
  - 用 main 的 `reports/baseline.json` 跑 v1.0 构建：`exit=2`，28 条 REGRESSION；
  - main 构建：`exit=0`。

### 2.2 gates 判定逻辑（只读审查）

- `crates/cli/src/judge.rs`：
  - Blargg 要求有 `Passed`（合集为 `Passed all tests`）且没有 `Failed`；
  - acid2 要求逐字节相等且长度为 23040；
  - Mooneye 要求 6 个寄存器恰为 3/5/8/13/21/34，panic、出错、超时都判失败。
- `gates.rs` 对所有 ROM 一律执行 `Machine::new(rom)` 后逐帧 `step`，没有按 ROM 区别处理。
- 纳入规则与 PLAN §2.4 一致。

### 2.3 Python 测试

```
# PYTHONPATH=<checkout>/python GBEMU_LIB=<target>/release/libgb_ffi.dylib GBEMU_REQUIRE_MACHINE=1 \
#   $REPO/.venv/bin/python -m pytest -q -p no:cacheprovider <checkout>/python/tests
v1.0-gates : 49 passed
ac7e571    : 1 failed, 49 passed   （连跑 3 次都如此）
             FAILED test_tools.py::test_drive_loop_records_interrupted_round
             E  assert (1 == 2) ... len(['round\tsession_id\trc\tseconds\tactions\tstatus'])
             单独运行：1 passed；-k "fallback or interrupted"：1 failed
7a41b99    : 51 passed ×6，1 failed, 50 passed ×1
```

### 2.4 里程碑 tag（/tmp/gbemu-review/scripts/tags.sh）

| tag | 提交 | cargo test | gates | 与声明是否一致 |
| --- | --- | --- | --- | --- |
| m1-g1、m2-timing | 9416633 | 126 通过 / 0 失败 / 4 忽略 | `G1 12/12 G2 1/1 G3 4/4 G4 0/1` | 一致（tag 说明本身写的就是 G4 0/1，PPU 尚未合入） |
| m3-acid2 | 8803312 | 265 / 0 / 3 | `G1 12/12 G2 1/1 G3 4/4 G4 1/1`，退出码 0 | 一致 |
| w4-mooneye-94 | 503ffa9 | 324 / 0 / 0 | `… MOONEYE 94/94`，退出码 0 | 一致 |

### 2.5 性能（T55）

```
gb-cli bench roms/pokered.gb --frames 3600
v1.0 : 3600 frames in 0.339 s = 10631.6 fps (178.0x realtime)
main : 3600 frames in 0.399 s =  9016.7 fps (151.0x realtime)
```

测量时本机还在跑另一场实玩。

### 2.6 G5 证据（reports/playtest/T62-rc-gates-7558ce3/）

- **元数据**：

  | 项 | 值 |
  | --- | --- |
  | `git_head` | `7558ce3`（= v1.0-gates = rc-gates） |
  | `rom_sha1` | 正确 |
  | `gbemu_lib_sha256` | `4c1bff22…`，与 `data/gbemu-lib/libgb_ffi.dylib` 以及 `GB-emulator-lanes/rc/target/release/libgb_ffi.dylib` 的 shasum 一致 |
  | `lanes/rc` 状态 | `git status` 干净，reflog 只有 1 条（从未移动） |
  | `driver_cmd` | `lanes/rc/tools/drive_loop.sh` |

- **9 项硬性判定**：由 `/tmp/gbemu-review/scripts/samples.py` 从 `samples.jsonl` 独立重算：

```
samples 181 t0 0.0 tN 1800.0 ; gap min/max 10.0 10.0
server_alive / health_ok / state_ok / screenshot_ok 全部为 True（每项 false count 0）
state_status set {200} ; state_problems nonempty 0 ; games_new_calls set {1}
actions_without_frame_advance max 0 ; actions_total last 172 ; frame_count monotonic True
distinct (map,x,y) 34 ; mono samples [0,10,50,1430,1720] ; longest mono span 10 s
bad shots 0（181 张都是 160×144 PNG）; pyboy_loaded any False ; backend {'GbEmuEmulator'}
map sequence: Pallet Town → Red's House 2F → 1F → Pallet Town → Oak's Lab → Pallet Town → Route 1 → Viridian City → Viridian Pokecenter
```

  以上与 summary.md 的 9 项判定和时间线一致。

- **server.log 端点统计**：`POST /games/new` 1 次、`POST /action` 172 次，没有任何 `/load`。
- **驾驶模型**：22 个轮次 json 的 `modelUsage` 都只有 `global.anthropic.claude-sonnet-5-5`。
- **prompt**：包含"禁止调用 `/games/new`、`/games/{id}/load`、`/load`"。
- **contact-sheet.png**：用 Read 查看了 31 格：
  - 开场、取名、各房间、大木研究所、战斗、1 号道路、常青市、宝可梦中心，画面都正常，没有花屏、错位或撕裂；
  - 黑色区域都在地图边界之外（室内地图的边框），是正常现象；
  - 另外放大查看了 `s0143`（战斗转场的白屏）、`s0144`（1 号道路）、`s0120`（研究所出口），都正常。
- **会话记录**：
  - `driver-sessions.txt` 中的 22 个会话 ID，在 `~/.claude/projects/-Users-skyseek-Desktop-project-open-research-platform-GB-emulator-lanes-rc/<id>.jsonl` 下**全部存在**；
  - 另有 1 个未登记的 `62544168-….jsonl`（02:21:48），即第 23 轮，见 P2。

### 2.7 §4.5 拓扑核对

```
$ git log --oneline --merges v1.0-gates..0139161        # 空
$ git log --oneline v1.0-gates..0139161
0139161 evidence (§4.5 hidden set): raw Mooneye on frozen rc-gates 7558ce3 ...
1fa94b5 T91 prep: README ...
53fa727 DECISIONS: D031 ...
$ for c in 3783e83 7fb5a3f 8e57c35 bde2b7f c34dbef 51642c6 63fb3a1 166da04 8b9af28 cd116ae; do git merge-base --is-ancestor $c v1.0-gates ...; done
→ 全部 "not in"
# integ/w4 的 worktree 创建于 0139161（reflog：0139161 HEAD@{02:05:36}），随后合并 w4/ppu 与 lane/platform；进 main 是 ea586c2（02:23:18）
$ git log --all -p -- crates/core/src | grep '^+' | grep -i mooneye
+//! 向量在写完 PCH 之后才决定：... PC=0x0000（Mooneye ie_push）。
+            // 有挂起中断即唤醒，不额外耗时（Mooneye halt_ime0/1 timing：...）。
+//! 只在其从 0 变为 1 时请求一次（Mooneye `stat_irq_blocking`）。DMG 上进入 LY=144 的第一个
+//! M-cycle 里 mode 2 源也有效（Mooneye `vblank_stat_intr-GS`）。
+//! # LCD 开关（`LCDC.md`；Mooneye `stat_lyc_onoff`、`lcdon_timing-GS`）
+/// VBlank 行、LY 已读作 0。行内位置文档未给出；Mooneye `boot_hwio-dmgABCmgb` 在开机 4740 dot 后
+        // LYC=LY 标志与 STAT 中断线保持关闭前的值（Mooneye stat_lyc_onoff）。
$ git blame -L 22,27 crates/core/src/timer.rs   → 8388811（T15，sys）"低 8 位（DIV 内部相位）取实机测得的值…"
$ refs/mooneye-src/acceptance/boot_div-dmgABCmgb.s : "; This read should happen immediately after DIV has incremented"
```

### 2.8 B1 复核

```
$ cp /tmp/gbemu-scout/race.py /tmp/gbemu-review/scripts/ ; cp /tmp/gbemu-scout/{T62-4076,s4076}.state /tmp/gbemu-review/b1/
$ PYTHONPATH=/tmp/gbemu-review/v1/python GBEMU_LIB=$REPO/reports/playtest/T62-rc-gates-7558ce3/data/gbemu-lib/libgb_ffi.dylib \
    .venv/bin/python -P race.py roms/pokered.gb T62-4076.state 40
k=14 xy=(4,5) scr=6 maxvis=11 ovf=True t89=a0 c102=ff girl=(1, 8) STALL
stall at k = [14] (1/40)
$ PYTHONPATH=/tmp/gbemu-review/repo/python GBEMU_LIB=/tmp/gbemu-review/target-main/release/libgb_ffi.dylib \
    .venv/bin/python -P race.py roms/pokered.gb s4076.state 40
k=22 xy=(4,5) scr=6 maxvis=9 ovf=True t89=a0 c102=ff girl=(1, 10) STALL
stall at k = [22] (1/40)
# 我自己的 5 分钟 scripted 冒烟（v1.0-gates 构建，端口 8811，/tmp/gbemu-review/playtest/v1-script-5min）：
[playtest] PASS ; 2 distinct (map,x,y) ; last position change at row 1454 (t=9.74 s) → (40,5,5)
replay 2000 rows（/tmp/gbemu-review/scripts/b1probe.py）：
map=40 x=5 y=5 D5F0=6 C45C=a0 C434=a0 C102=ff C109=04 CFC4=00 FF90=50
```

在反汇编中核对的位置：

- `ram/wram.asm:144-156`
- `pokered.sym`：`00:c300 wShadowOAM`、`00:c3a0 wShadowOAMEnd`、`00:c3a0 wTileMap`
- `engine/gfx/sprite_oam.asm`：开头部分，以及清理循环所在的 140–170 行
- `home/vblank.asm`：`call PrepareOAMData`
- `home/update_sprites.asm`：只检查 `wUpdateSpritesEnabled`

### 2.9 R6 合并 diff 抽查

- **W4 合并 ea586c2**（`git diff ea586c2^1 ea586c2 -- crates/core/src`，23 个文件，+1702/−299；我通读了 ppu.rs、render.rs、bus.rs、cpu.rs、serial.rs、timer.rs、machine.rs、cart/*）：
  - **ppu**：改为事件表驱动的时序，加入可变长度 mode 3（与 Pan Docs `Rendering.md` 逐条一致）、VRAM/OAM 封锁（`Accessing_VRAM_and_OAM.md`）、第 0 行 452 dot。
  - **cpu**：取指之后再检查中断，HALT 期间每个 M-cycle 取指（Pan Docs `halt.md`）。
  - **bus**：APU 读掩码（Audio_Registers 位图）、NR52、写 PPU 寄存器触发的 STAT 中断同周期进入 IF、`peek` 绕过封锁（D031）。
  - **serial**：由系统计数器 bit 8 驱动（`Timer_Obscure_Behaviour`）。
  - **cart**：MBC2、MBC5、MBC1M（按 Pan Docs 的 logo 判定规则）。
  - 结论：没有按 ROM 或标题分支，没有按 PC 或测试的特判，没有跳过检查。
- **G 轨合并 cf13ea1**（lane/ppu T40–T42，+675/−24）：扫描线渲染、STAT 上升沿、窗口行计数器、10 个精灵上限、DMG 优先级，都是通用实现。它引入的 Mooneye 引用注释在 96ac7a9 中删除，见 P1。
- **全量 core 扫描**：

```
grep -rnE "0x01?3[4-9A-F]\b|0x014[0-9A-F]|title|checksum|hash|pc == " crates/core/src
→ 只有 machine.rs 的存档指纹（PLAN §3.8）与 Power_Up_Sequence 的 header_checksum→F 标志
```

### 2.10 T90 原始输出

```
### 1  git grep -niE 'pyboy|sameboy|gambatte|mgba|bgb' -- crates python tools
python/gbemu/launch.py:5,10,15,35,36,39,40,47,48,56,119,166   （GBEMU_LIB=…libgb_ffi.dylib 与 _BlockPyboy 守卫）
python/gbemu/native.py:7,114                                   （libgb_ffi.dylib）
python/tests/conftest.py:5,62 ; fake_server.py:122 ; test_launch.py:17,32,33,80,123 ; test_tools.py:244
tools/playtest.py:4,537,559
→ sameboy/gambatte/mgba 0 处；bgb 全部是 "libgb_ffi"；pyboy 全部是禁用、断言或统计
### 2  git grep -niE 'acid|blargg|mooneye|cpu_instrs|pokemon|pokered' -- crates/core/src   → 空（rc=1）
### 3  git ls-files | grep -iE '\.(gb|gbc|sav|state)$'                                    → 空（rc=1）
### 4  git reflog（主仓库，52 条）：commit 25、commit (initial) 1、merge 25、cherry-pick 1；grep rebase|amend|reset → 无
       各 worktree：core/platform/play/ppu 各 1 条、sys 2 条、integ 1 条 "reset: moving to HEAD"；rc、w4final 无
       实验：git worktree add -b lane/x ../wt → reflog: "reset: moving to HEAD" + 空消息
       git log --all --no-merges 中重复标题或补丁：只有 c677431 / 3c62c9a（T52；c677431 带 "(cherry picked from commit 3c62c9a…)"）
       git fsck --no-reflogs --unreachable：只有 5 个 tree，没有 commit
### 5  .venv/bin/python -m pip → 不存在；uv pip list --python .venv/bin/python | grep -i pyboy → 空；find_spec('pyboy') → None
### 6  git grep -nE '"(Passed|Failed)' -- crates/core/src → 空
       const/static 数组：TAC_BIT[4]、ILLEGAL[11]、APU_READ_OR[0x20]、STATE_MAGIC[7]、SHADES[4]；fn 内的开机表 [(u16,u8);21]
```

---

## 3. 交付前必须处理（T80 完成标准："未复现的项必须在 T91 之前解决"）

1. **P1**：补记 DECISIONS，REPORT 同时给出两种 Δ，更正 `timer.rs` 与 `BOOT_DOT` 的出处说明。
2. **P2**：补登第 23 轮会话，更正"22/22"的表述。
3. **P3**：修好 pytest 竞态，并在空闲机器上全量连跑 10 次全绿。
4. **P9**：重新生成 PROGRESS。

P4–P8、P10 写进 REPORT 的"已知问题"和"合规自查"两节即可。
