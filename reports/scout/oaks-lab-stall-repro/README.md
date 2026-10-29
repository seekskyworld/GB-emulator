# B1 最小复现：大木研究所卡死（游戏自身竞态，D032）

结论与分析见 `../oaks-lab-stall.md`。本目录是复现材料。存档文件（`.state`）不入库（R5），用 `mkstate.py` 现场生成。

## 步骤（在仓库根目录执行）

```bash
# 1. 构建 v1.0-gates（7558ce3）的 libgb_ffi：它固定在 reports/playtest/T62-rc-gates-7558ce3/gbemu-lib/libgb_ffi.dylib（sha256 4c1bff22…），也可以自行 checkout 后构建
export GBEMU_LIB=$PWD/reports/playtest/T62-rc-gates-7558ce3/gbemu-lib/libgb_ffi.dylib
export PYTHONPATH=$PWD/python
D=reports/scout/oaks-lab-stall-repro
gunzip -k $D/driver-actions-first4100.jsonl.gz

# 2. 回放 scripted 驾驶日志的前 4076 条请求（屏幕上是 "OAK: Hey! Don't go away"），存档
.venv/bin/python $D/mkstate.py $D/driver-actions-first4100.jsonl roms/pokered.gb 4076 /tmp/s4076.state

# 3. 从该状态起先等 k 帧再按 A，扫描 k=0..39，统计 wShadowOAM 越界与卡死（预期输出见 race-T62.txt：只有 k=14 卡死）
.venv/bin/python $D/race.py roms/pokered.gb /tmp/s4076.state 40

# 4. 卡死后按 START 再按 B 即可恢复（recover2.py）
```

W4 构建（main）上同样的扫描只在 k=22 卡死（`race-W4.txt`）。两个构建各只有一个单帧窗口，说明这是帧对齐决定的游戏竞态，而不是某个时序实现错误导致的必然失败。
