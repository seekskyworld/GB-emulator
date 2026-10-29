# 大木研究所卡死（主角消失、输入失效）：从游戏逻辑一侧诊断

scout，只读诊断。依据：`ROM/pokered-src`（pret/pokered，ROM sha1 `ea9bcae6…` 与 `roms/pokered.gb` 一致）、`pokered.sym`，以及在两个构建上的确定性回放。没有阅读任何第三方模拟器的源码。

## 结论摘要

**选 (a)：这是游戏代码自身的竞态缺陷，在这串输入的帧时序下被触发，不是模拟器偏离硬件。**

一句话说明：强制走回 (4,5) 后，`_UpdateSprites` 逐个精灵重新做可见性裁剪，Pokédex（槽 6、7）先被设为可见，研究员和女孩（槽 9–11）后被裁掉。如果恰好在这两步之间进入 VBlank，`PrepareOAMData` 会看到 11 个可见精灵，生成 44 个 OAM 项。它没有越界检查，于是写出 `wShadowOAM`（C300–C39F），覆盖紧随其后的 `wTileMap`（C3A0）。随后的"清除未用 OAM"循环又从 C3B0 起每 4 字节写一个 `$A0`，一直写到 C49C。被破坏的 `wTileMap` 让以下三件事同时成立，形成自锁：

1. 主角精灵被隐藏；
2. 主角朝向被冻结；
3. 前方图块永远不可通行。

两个构建都能触发：pre-W4 的 rc-gates 7558ce3（Mooneye 66/94），以及当前 main 的 W4 ea586c2（Mooneye 94/94，中断和 PPU 时序已改成精确实现）。在扰动输入时序的扫描里，两者的命中率都约为 1/28 帧。所以这不是某个具体时序 bug 造成的。另外，按 START 再按 B 可以完全恢复，这也证明卡死的根因是 `wTileMap` 被污染，而不是 CPU 或中断停摆。

## 1. 现象（实测）

- 位置一直停在 (map 40, x 4, y 5)，`wOaksLabCurScript`（D5F0）= 6。
- 主角 image index `wSpritePlayerStateData1ImageIndex`（C102）= `$FF`，朝向 `wSpritePlayerStateData1FacingDirection`（C109）一直是 `$04`（UP）。
- 桌上的精灵球从 3 个变成 1 个：槽 2、4 的 image index 变成 `$FF`，同样是因为它们脚下的 `wTileMap` 被写成了 `$A0`。
- `wTileMap` 的内容（卡死后）：
  - 第 0 行前 16 字节是 OAM 格式数据 `9c 88 38 00 | 9c 90 39 00 | a4 88 3a 02 | a4 90 3b 03`，即一个 2×2 精灵的 4 个 OAM 项。
  - 从 C3B0 到 C49C，每第 4 个字节都是 `$A0`。这正是 `PrepareOAMData` 用来隐藏 OAM 项的值：Y = `SCREEN_HEIGHT_PX + 16` = 160。
- `hOAMBufferOffset`（FF90）在出事那一帧是 `$B0`，即 44 项 × 4 字节，大于 `$A0`。

## 2. 游戏在等的条件（引用 asm）

### 2.1 Oak's Lab 状态机

- `scripts/OaksLab.asm:154-157`，`OaksLabPlayerDontGoAwayScript`（脚本 6）：
  - 逻辑是 `ld a,[wYCoord] / cp 6 / ret nz`。
  - 也就是说，它每帧只做一件事：等主角走到 y = 6。
- `scripts/OaksLab.asm:184-192`，`OaksLabPlayerForcedToWalkBackScript`（脚本 7）：
  - 等 `wSimulatedJoypadStatesIndex` 归零，然后执行 `Delay3`，再回到脚本 6。
  - 实测：脚本 7 → 6 的切换已经正常完成，模拟摇杆也已正常结束（`home/overworld.asm:1864` 的 `.doneSimulating` 清掉了 `wStatusFlags5` 的 bit 7，`wJoyIgnore` = 0）。
- 所以从脚本的角度看，游戏只是在等"玩家走到 y = 6"。卡住的原因是玩家永远走不动。

### 2.2 为什么走不动：wTileMap 被污染后的三处读取

1. **主角被隐藏，朝向冻结。**`engine/overworld/movement.asm:1-21`，`UpdatePlayerSprite`：
   - 读取 `lda_coord 8, 9`（第 14 行），即 `wTileMap` 的 (8,9) = C45C。
   - 如果值 ≥ `MAP_TILESET_SIZE`（$60，见 `constants/tileset_constants.asm:1`），就认为"有文本框盖住主角"，跳到 `.disableSprite`（第 18 行），写 image index = `$FF` 后直接 `ret`。
   - 写朝向的代码在第 56 行，这条路径会跳过它，所以朝向永远停在 UP。
   - 实测 (8,9) = `$A0`。
2. **所有方向的前方图块都不可通行。**
   - `engine/overworld/player_state.asm:260-292`，`_GetTileAndCoordsInFrontOfPlayer`：按 C109 的朝向选择前方图块。朝向固定为 UP，所以无论按哪个方向，读到的都是 (8,7)。实测 (8,7) = `$A0`。
   - `home/overworld.asm:1259-1276`，`CheckTilePassable`：`$A0` 不在 `Dojo_Coll` 中（`data/tilesets/collision_tile_ids.asm:22-24`，可通行的只有 $11 $16 $19 $2b $3c $3d $3f $4a $4c $4d $03），判为碰撞。
   - 结果：`home/overworld.asm:1219-1252` 的 `CollisionCheckOnLand` 对四个方向都返回碰撞。实测 `wTileInFrontOfPlayer`（CFC6）= `$A0`。
3. **没有任何东西会重建 `wTileMap`。**
   - 正常重建依赖两条路径：走一步后的 `LoadCurrentMapView`，或文本框关闭时的 `CloseTextDisplay`（`home/text_script.asm:105-139`，第 135 行调用 `LoadCurrentMapView`）。
   - 主角不能移动，前方（按 UP 朝向）也没有可以对话的对象，所以 A 键无效。
   - driver 在 4079 之后共发出 41699 条请求，其中从未按过 START。

### 2.3 污染从哪里来：PrepareOAMData 没有边界检查，和 _UpdateSprites 的逐槽更新构成竞态

- `ram/wram.asm:144-156`：`wShadowOAM`（40 项，C300–C39F）后面紧跟着 `wTileMap`（C3A0）。
- `engine/gfx/sprite_oam.asm:18-145`：
  - VBlank 中调用的 `PrepareOAMData`（`home/vblank.asm:33`）遍历 16 个精灵槽。
  - 只要 picture ID ≠ 0 且 image index ≠ `$FF`，就写 4 个 OAM 项，`hOAMBufferOffset` 按 `inc e` 递增，没有上限检查。
  - 第 147-167 行的清除循环：
    - 条件是 `cp l / ret z`，只在 `l == $A0` 时停止；
    - 每次 `add hl, 4`。
    - 起点 l = `$B0` 时，会一路写到 h 进位后 l 再次等于 `$A0` 的位置，即写到 C49C，下一个位置 C4A0 才停。这与实测的覆盖范围完全一致。
- `engine/overworld/movement.asm:477-536`，`CheckSpriteAvailability`：
  - 可见窗口为 `wYCoord ≤ MAPY ≤ wYCoord + 8`（坐标带 +4 偏移，第 489-499 行）。
  - 在 y = 6 时：研究员（y = 10）、女孩（y = 9 或 10）可见，Pokédex（y = 1）不可见。
  - 在 y = 5 时：Pokédex 可见，研究员不可见；女孩是否可见取决于她当时的 y。
  - 正常情况下，两种位置都是 9 个可见精灵（含主角），不会溢出。
- `engine/overworld/sprite_collisions.asm:1-29`，`_UpdateSprites`：按槽 0 → 15 的顺序依次处理，每个 NPC 都会调用一次 `CheckSpriteAvailability`。
  - 在 y 从 6 变成 5 后的第一次 `UpdateSprites` 中（`home/overworld.asm:132` 的 `.noDirectionButtonsPressed` 路径），槽 6、7 先被设为可见，槽 9、10、11 后被裁掉。
  - 这两步之间存在一个瞬时状态：11 个精灵的 image index 都不是 `$FF`。
  - 只要 VBlank 落在这个窗口里，就会溢出。
- 实测（T62 构建，按请求 4078 回放，逐帧采样，采样点固定在 LY = 56）：

  | 采样帧 | 状态 |
  |---|---|
  | 帧 84 | PC 在 `_Joypad`（bank 3 的 4017），脚本 = 6，可见精灵 = 9，各槽 image index 均正常 |
  | 帧 85 | PC 在 `_UpdateSprites` 内（bank 1 的 4ca5），`hCurrentSpriteOffset` = `$90`（槽 9）；槽 6、7 已经是 `b0`/`b4`，槽 9–11 仍是 `30`/`4c`/`48`；可见 = 11，`hOAMBufferOffset` = `$B0`，(8,9) = `$A0` |
  | 帧 86 | 研究员已变成 `$FF`，但 `wTileMap` 已被破坏，进入自锁 |

  说明：这次 `_UpdateSprites` 从上一个 VBlank 之后开始，一直跨过了下一个 VBlank。中间那次 VBlank ISR 恰好在槽 8、9 处插入。

## 3. 实测变量值（卡死态）

| 变量 | 地址 | 值 | 含义 |
|---|---|---|---|
| wOaksLabCurScript | D5F0 | 6 | 等 wYCoord == 6 |
| wYCoord / wXCoord | D361/D362 | 5 / 4 | |
| wSimulatedJoypadStatesIndex | CD38 | 0 | 模拟摇杆已结束 |
| wStatusFlags5 (旧称 wd730) | D730 | 00 | 无脚本移动 |
| wJoyIgnore | CD6B | 00 | 不屏蔽任何键 |
| wFontLoaded | CFC4 | 00 | 无文本框 |
| wSpritePlayerStateData2WalkAnimationCounter | C200 | 00 | 走 `.checkIfTextBoxInFrontOfSprite` 分支 |
| wTileMap(8,9) | C45C | **A0**（应为 $11） | ≥ $60，触发 `.disableSprite` |
| wTileMap(8,7) | C434 | **A0** | 前方不可通行 |
| wSpritePlayerStateData1ImageIndex | C102 | FF | 主角不绘制 |
| wSpritePlayerStateData1FacingDirection | C109 | 04 | 冻结为 UP |
| wTileInFrontOfPlayer | CFC6 | A0 | |
| wWalkCounter | CFC5 | 0 | |
| hOAMBufferOffset（出事帧） | FF90 | **B0**（上限 A0） | OAM 缓冲溢出 |
| TAC | FF07 | F8 | 定时器关闭，只有 VBlank 中断在起作用（IE = 0D） |

## 4. 为什么判 (a)，而不是 (b)

1. **因果链完全落在游戏代码内。**涉及的只有：
   - `PrepareOAMData` 没有边界检查；
   - `_UpdateSprites` 逐槽更新可见性（不是原子的）；
   - `wShadowOAM` 和 `wTileMap` 在 WRAM 中相邻；
   - 三处代码用 `wTileMap` 上的图块 ≥ $60 来判断"有文本框"或"不可通行"。

   硬件只决定一件事：这一帧的 CPU 工作量（VBlank ISR + `_UpdateSprites` 处理 12 个精灵）会不会跨过下一个 VBlank，以及跨过时落在哪个槽。两个构建都显示这段工作量本来就接近或超过一帧：W4 构建在 LY = 92 采样时，仍常处在 `_UpdateSprites` 内部。
2. **两个时序实现差异很大的构建都能触发。**
   - 从请求 4076 的状态（Oak 的文字已在屏幕上）开始，等待 k 帧后按 A，k = 0..39。
   - 有效样本是 k = 12..39，共 28 个；k < 12 时 A 键还关不掉正在打印的文本。

   | 构建 | Mooneye | 结果 |
   |---|---|---|
   | T62 lib（rc-gates 7558ce3） | 66/94 | 只在 k = 14 卡死 |
   | W4 main（ea586c2） | 94/94 | 只在 k = 22 卡死 |

   两者都是约 1/28 的单帧窗口，只是窗口落在不同的 k 上。这符合"真实竞态，命中与否由帧对齐决定"的特征，不像某个硬件子系统实现错误后导致的必然失败。
3. **START → B 能完整恢复。**`CloseTextDisplay` 调用 `LoadCurrentMapView` 重建 `wTileMap` 后，主角重新出现，也能左右移动。T62 构建在原日志 4080 处、W4 构建在 k = 22 处都验证过。这说明 CPU、中断、joypad、定时器、MBC3 都在正常工作。
4. **反证检查。**
   - 定时器关闭（TAC = F8），不存在额外的中断负载。
   - OAM DMA 不参与溢出：溢出发生在 WRAM 写入阶段。
   - joypad 读数正常（hJoyHeld 和 hJoyPressed 随按键变化）。

**保留意见。**我无法在真机上验证"这一串输入"会不会在同一帧命中。命中依赖的是整帧 CPU 周期数的精确值，还有开机时 DIV 的相位：它通过 `Random` 影响女孩的位置，女孩位置又决定溢出要求的 11 个可见精灵能否凑齐。所以，原日志在真机上逐帧回放不一定会卡在同一处。但这个缺陷本身与模拟器无关，真机上也存在。

## 5. 建议 core 检查的模拟器代码点（优先级低，不是修复项）

core 不需要为这个问题修 bug，也不应该为它做针对性处理（R6）。如果想进一步收窄"命中窗口落在哪一帧"与真机的差异，可以核对以下几项，它们决定整帧 CPU 周期预算：

- `crates/core/src/cpu.rs` 和 `interrupts.rs`：
  - HALT 退出到执行 ISR 第一条指令之间的周期数；
  - 中断分派是否为 5 M-cycle；
  - `reti`、`call`、`ret cc`、`jr cc`、`jp cc` 在条件成立和不成立两种情况下的周期数。
- `crates/core/src/dma.rs`：OAM DMA 期间 CPU 只运行 HRAM 中的 `hDMARoutine`，确认 DMA 的 160 M-cycle 没有额外停顿或少算。
- `crates/core/src/timer.rs`：DIV 相位，它影响 `Random`（`home/vblank.asm:37`），从而影响女孩的移动。

## 6. 给 integrator 和 driver 的建议

- **检测**：在大地图（非战斗）中，如果 `wFontLoaded`（CFC4）bit 0 为 0，但 `wTileMap(8,9)`（C45C）≥ $60，或 C102 = FF，并且位置连续 N 次请求不变，就判定为 OAM 溢出自锁。
- **恢复**：发送 `press_start, wait_30, press_b, wait_30`，即可重建 `wTileMap`。
- **构建说明**：调查过程中，`target/release/libgb_ffi.dylib` 在 02:23 被 W4 合并重建过。用新构建按原日志回放时，4078 处不再卡死，因为帧对齐已经变了。原现象需要用 `reports/playtest/T62-rc-gates-7558ce3/data/gbemu-lib/libgb_ffi.dylib` 复现。

## 附：复现脚本（/tmp/gbemu-scout/）

| 脚本 | 用途 |
|---|---|
| `mkstate.py` | 回放到第 N 条请求后存档 |
| `frames3.py` | 逐帧打印 PC、LY、`hCurrentSpriteOffset`、`hOAMBufferOffset`、各槽 image index |
| `race.py` | 按 k 扫描输入时序，统计卡死命中 |
| `recover.py` / `recover2.py` | 卡死后测试各键，以及 START → B 恢复 |

用法示例：

```
cd $REPO && GBEMU_LIB=/tmp/gbemu-scout/libgb_ffi_T62.dylib PYTHONPATH=python .venv/bin/python \
  /tmp/gbemu-scout/race.py roms/pokered.gb /tmp/gbemu-scout/T62-4076.state 40
```

W4 构建对应的 lib 是 `/tmp/gbemu-scout/libgb_ffi.dylib`，状态文件是 `s4076.state`。
