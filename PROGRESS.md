# PROGRESS

> 门槛状态的唯一来源是 `reports/gates.json`（PLAN §4.1）；本文件的汇总行只从它生成。只在里程碑或出现 Blocker 时更新（PLAN §6.1）。

## 当前状态

- 里程碑：M1（G1）、M2（G2/G3）已达成（tag `m1-g1`、`m2-timing`）；M3（G4）进行中
- gates 汇总（全量，来自 `reports/gates.json`）：`G1 12/12 G2 1/1 G3 4/4 G4 0/1`（G4 待 ppu 合入）
- 通道（D004）：core（CPU）、sys（总线/外设/整机）、ppu、platform（卡带/cli/gates）、play（FFI/Python/实玩工具）；worktree 在 `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/<lane>`，分支 `lane/<lane>`

## 任务状态

| ID | 任务 | 依赖 | 代理 | 波次 | 状态 |
| --- | --- | --- | --- | --- | --- |
| T00 | 环境、仓库、模型探测、权限与开工前提 | — | integrator | W0 | done |
| T01 | 资料就绪与参考图转换 | T00 | integrator | W0 | done |
| T02 | 冻结跨通道接口与模块空壳 | T00 | integrator | W0 | done |
| T05a | gates 最小可用版与 `gb-cli run` | T01,T02 | platform | W1 | done |
| T05b | gates 补全（棘轮、对抗单测、日志、差异图、截图） | T05a | platform | W1 | doing |
| T10 | CPU 解码框架与 opcode 表测试 | T02 | core | W1 | done |
| T11 | ALU 纯函数与全部指令（含周期序列） | T10 | core | W1 | doing |
| T12 | SingleStepTests 接入 | T10 | core | W1 | doing |
| T13 | 总线、内存映射、开机状态、Machine 存档 | T02 | sys | W1 | done |
| T14 | 中断、EI/DI/RETI、HALT 与 HALT bug（CPU 侧 core，IF/IE 侧 sys） | T11,T13 | core+sys | W1 | doing |
| T15 | 定时器 | T13 | sys | W1 | done |
| T16 | 串口 | T13 | sys | W1 | done |
| T51 | Joypad | T13 | sys | W1 | done |
| T17 | **G1：cpu_instrs** | T05a,T14,T15,T16,T50 | core+sys（integrator 合并） | W2 | done |
| T30 | **G2/G3：instr_timing、mem_timing** | T17 | sys+core（integrator 合并） | W2 | done |
| T40 | PPU 时序骨架（单测） | T02 | ppu | W1 | doing |
| T41 | BG/Window 渲染（单测） | T40 | ppu | W1 | doing |
| T42 | 精灵与 OAM DMA 状态（单测） | T41 | ppu | W1 | doing |
| T43 | **G4：dmg-acid2** | T17,T42 | ppu | W2 | todo |
| T50 | ROM only、MBC1、MBC3（单测） | T02 | platform | W1 | done |
| T52 | 存档往返测试 | T17,T42,T50 | platform | W2 | todo |
| T53a | FFI 骨架与 Python 绑定 | T02 | play | W1 | done |
| T53b | Python 后端整机验证 | T52,T53a | play | W2 | todo |
| T54 | pokemon-agent 启动器与补丁 | T53b | play | W2 | doing |
| T60 | 实玩监控 `playtest.py` 与驾驶循环 `drive_loop.sh` | T54 | play | W2 | doing |
| T61 | 回归驾驶脚本与 5 分钟冒烟（含一次 fps 测量） | T60,T43 | play | W2 | doing |
| T61b | 提前跑 30 分钟实玩（条件触发，默认不做） | T61 | integrator 监督，driver 驾驶 | W2 | todo |
| T62 | **G5：30 分钟正式实玩** | T30,T43,T61 | integrator | W3 | todo |
| T50b | MBC2、MBC5 | T50 | platform | W4 | todo |
| T55 | 性能 ≥ 600 fps | T62 | platform（core 配合） | W4 | todo |
| T63 | 宝可梦确定性回放基线 `POKERED-REPLAY` | T62 | play | W4 | todo |
| T70 | Mooneye 范围与计分 | T05b | platform | W4 | todo |
| T71 | Mooneye 迭代（加分） | T62,T70 | scout ×N + core/sys/ppu | W4 | todo |
| T80 | 评审（一次） | T62 | reviewer | W4 | todo |
| T90 | 合规自查 | T62 | reviewer | W4 | todo |
| T91 | 交付 | T80,T90 | integrator | W4 | todo |

## Blockers

（无）

## 下一步

关键路径转到 G4/G5：ppu T40–T42 → T43（acid2）；play T54/T60/T61 工具就绪；ppu 合入后 platform T52、play T53b/T61 冒烟；G1–G4 全绿后打 `rc-gates` 跑 T62。core/sys 完成收尾单测（T11/T12/T14）。
首次整合即通过 G1–G3（core T10 + sys T13–T16/T51 + platform T05a/T50，2026-10-30 01:2x），证据：`reports/gates.json` 串口原文（合集 `cpu_instrs`：01–11 ok、`Passed all tests`；`mem_timing`：01–03 ok、`Passed all tests`；`instr_timing`：`Passed`）。
