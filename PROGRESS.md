# PROGRESS

> 门槛状态的唯一来源是 `reports/gates.json`（PLAN §4.1）；本文件的汇总行只从它生成。只在里程碑或出现 Blocker 时更新（PLAN §6.1）。

## 当前状态

- 里程碑：M1–M5 完成，**G1–G5 完成**（tag `v1.0-gates` = 7558ce3，2026-10-30 02:23）；M6 Mooneye 94/94（tag `w4-mooneye-94`）；M7 交付进行中
- gates 汇总（`reports/gates.json`，main）：`G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 94/94`
- G5 证据：`reports/playtest/T62-rc-gates-7558ce3/`（30 分钟、Sonnet 5.5 驾驶 22/22 轮、0 次 scripted 兜底；真新镇 → 常青市宝可梦中心）
- 隐藏集证据（§4.5）：`reports/mooneye-raw-rc-gates/`（v1.0-gates 原样：MOONEYE 66/94，raw 67/115）
- 通道（D004）：core、sys、ppu、platform、play；worktree 在 `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/<lane>`

## 任务状态

| ID | 任务 | 依赖 | 代理 | 波次 | 状态 |
| --- | --- | --- | --- | --- | --- |
| T00 | 环境、仓库、模型探测、权限与开工前提 | — | integrator | W0 | done |
| T01 | 资料就绪与参考图转换 | T00 | integrator | W0 | done |
| T02 | 冻结跨通道接口与模块空壳 | T00 | integrator | W0 | done |
| T05a | gates 最小可用版与 `gb-cli run` | T01,T02 | platform | W1 | done |
| T05b | gates 补全（棘轮、对抗单测、日志、差异图、截图） | T05a | platform | W1 | done |
| T10 | CPU 解码框架与 opcode 表测试 | T02 | core | W1 | done |
| T11 | ALU 纯函数与全部指令（含周期序列） | T10 | core | W1 | done |
| T12 | SingleStepTests 接入 | T10 | core | W1 | done |
| T13 | 总线、内存映射、开机状态、Machine 存档 | T02 | sys | W1 | done |
| T14 | 中断、EI/DI/RETI、HALT 与 HALT bug（CPU 侧 core，IF/IE 侧 sys） | T11,T13 | core+sys | W1 | done |
| T15 | 定时器 | T13 | sys | W1 | done |
| T16 | 串口 | T13 | sys | W1 | done |
| T51 | Joypad | T13 | sys | W1 | done |
| T17 | **G1：cpu_instrs** | T05a,T14,T15,T16,T50 | core+sys（integrator 合并） | W2 | done |
| T30 | **G2/G3：instr_timing、mem_timing** | T17 | sys+core（integrator 合并） | W2 | done |
| T40 | PPU 时序骨架（单测） | T02 | ppu | W1 | done |
| T41 | BG/Window 渲染（单测） | T40 | ppu | W1 | done |
| T42 | 精灵与 OAM DMA 状态（单测） | T41 | ppu | W1 | done |
| T43 | **G4：dmg-acid2** | T17,T42 | ppu | W2 | done |
| T50 | ROM only、MBC1、MBC3（单测） | T02 | platform | W1 | done |
| T52 | 存档往返测试 | T17,T42,T50 | platform | W2 | done |
| T53a | FFI 骨架与 Python 绑定 | T02 | play | W1 | done |
| T53b | Python 后端整机验证 | T52,T53a | play | W2 | done |
| T54 | pokemon-agent 启动器与补丁 | T53b | play | W2 | done |
| T60 | 实玩监控 `playtest.py` 与驾驶循环 `drive_loop.sh` | T54 | play | W2 | done |
| T61 | 回归驾驶脚本与 5 分钟冒烟（含一次 fps 测量） | T60,T43 | play | W2 | done |
| T61b | 提前跑 30 分钟实玩（条件触发，默认不做） | T61 | integrator 监督，driver 驾驶 | W2 | skipped |
| T62 | **G5：30 分钟正式实玩** | T30,T43,T61 | integrator | W3 | done |
| T50b | MBC2、MBC5 | T50 | platform | W4 | done |
| T55 | 性能 ≥ 600 fps | T62 | platform（core 配合） | W4 | done |
| T63 | 宝可梦确定性回放基线 `POKERED-REPLAY` | T62 | play | W4 | todo |
| T70 | Mooneye 范围与计分 | T05b | platform | W4 | done |
| T71 | Mooneye 迭代（加分） | T62,T70 | scout ×N + core/sys/ppu | W4 | done |
| T80 | 评审（一次） | T62 | reviewer | W4 | todo |
| T90 | 合规自查 | T62 | reviewer | W4 | todo |
| T91 | 交付 | T80,T90 | integrator | W4 | todo |

## Blockers

- **B1（已结案，D032：游戏自身竞态，驾驶侧加恢复）宝可梦在大木研究所卡死**：play 的一次脚本驾驶日志可稳定复现——"Hey! Don't go away" 之后主角被模拟摇杆拉回 (4,5)，随后主角精灵 image index=0xFF、所有输入失效（游戏仍在运行）。v1.0-gates 的 30 分钟正式实玩没有触发。core（模拟器侧）与 scout（游戏反汇编侧）并行调查，复现材料在 `/tmp/gbemu-integrator/stall/`。

## 下一步

W4/M7：B1 根因与修复（G 轨，进 main 并补回归测试）→ W4 构建 30 分钟复核实玩（进行中）→ T63 回放基线 → T80 评审与 T90 合规自查（新开上下文的 reviewer）→ T91 交付（REPORT、sessions 导出）。
