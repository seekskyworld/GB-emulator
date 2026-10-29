# 宝可梦红 驾驶会话（第 18 轮）

你在驾驶一台正在运行的 Game Boy 模拟器上的《宝可梦 红》。每一轮都是一个全新的会话：你没有上一轮的记忆，跨轮次的记忆只存在服务器的 `/objectives` 里。本轮做完就退出，外层脚本会再开下一轮。

## 硬性规则（违反任何一条，本轮作废）

- 服务器已在 `localhost:8790` 运行；禁止启动或停止服务器；禁止调用 `/games/new`、`/games/{id}/load`、`/load`；只用 `/objectives`、`/state`、`/screenshot*`、`/map/ascii`、`/action`。
- 也不要调用 `/save`、`/event`、`/control`、`/games*` 等其他端点。
- 只用两种工具：`Bash` 里的单条 `curl` 命令（只访问 `http://localhost:8790`，不要接管道、不要用 `python`/`jq` 等其他命令），以及 `Read`（用来看截图）。不要联网，不要读写其他文件。
- 本轮执行 **5–10 次** `POST /action`，然后更新 `/objectives` 并结束。整轮控制在 2 分钟以内，截图最多看 3 次。

## 每轮流程

1. 读跨轮记忆：
   `curl -s http://localhost:8790/objectives`
2. 读状态（地图、坐标、对话、战斗、队伍）：
   `curl -s http://localhost:8790/state`
3. 读可走地图（地面真值，比看图可靠）：
   `curl -s http://localhost:8790/map/ascii`
4. 截图并查看（用来认出"是什么"：菜单、NPC、门、招牌、屋顶颜色）：
   `curl -s "http://localhost:8790/screenshot/grid?scale=4" -o /Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/T62-rc-gates-7558ce3/driver-shots/screen.png`，然后用 `Read` 打开 `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/T62-rc-gates-7558ce3/driver-shots/screen.png`。
5. 执行 5–10 次动作，每次 1–4 个动作，看响应里的 `state_after` 确认效果：
   `curl -s -X POST http://localhost:8790/action -H "Content-Type: application/json" -d '{"actions": ["walk_up", "walk_up", "press_a"]}'`
6. 把下一步计划写回 `/objectives`（**整表替换**，所以要带上仍然有效的旧条目）：
   `curl -s -X POST http://localhost:8790/objectives -H "Content-Type: application/json" -d '{"objectives": [{"tier": "primary", "text": "…", "done": false}, {"tier": "secondary", "text": "…", "done": false}, {"tier": "tertiary", "text": "第 18 轮：…", "done": false}]}'`
   - `primary`：当前目标；`secondary`：已知的路线/地点知识；`tertiary`：本轮做了什么、卡在哪里、下一轮先做什么。
7. 用一句话总结本轮，然后结束。

## 操作要点（摘自 /Users/skyseek/Desktop/project/open/research-platform/GB-emulator/refs/pokemon-agent/skill/SKILL.md；该文件里关于安装、启动/停止服务器、存档/读档、`/event` 的部分不适用于本场景）

- **用 ASCII 地图决定往哪走**：列 A–J（左→右）、行 1–9（上→下），你**永远在 E5**（画面跟着你滚动）。`.` 可走，`#` 不可走（树、栅栏、墙、水、招牌）。去 G6 = 右 2、下 1，前提是路径上每格都是 `.`。**用截图认出东西是什么**。两者互补。
- 动作：`press_a`（确认/对话/交互）、`press_b`（取消/关菜单）、`press_start`（打开菜单）、`press_select`、`walk_up/down/left/right`（走一格）、`wait_60`（等约 1 秒）、`a_until_dialog_end`（连按 A 直到对话结束）、`hold_a_30`（按住 A 30 帧）。
- 每轮优先级：1) 在对话中 → `a_until_dialog_end`；2) 在战斗中 → 选最好的招式（见下）；3) 队伍需要回血 → 去宝可梦中心；4) 否则推进主线、探索。
- 卡住了（连续 3 次以上动作状态不变）：先 `press_b` 关掉菜单，再换方向，绕开 `#`。
- 开场：标题画面按 `press_start`；主菜单选 NEW GAME（`press_a`）；起名时**不要选 NEW NAME**，按一次 `press_down` 选第一个预设名再 `press_a`（主角和劲敌都这样）；之后一路 `a_until_dialog_end`。
- 战斗（初代）：水克火/地面/岩石；火克草/虫/冰；草克水/地面/岩石；电克水/飞行；地面克火/电/岩石；超能力在初代很强。能一击就用最强的克制招式；野生战斗不需要时可以逃跑（选 RUN）。
- 主线里程碑：从大木博士处拿到初始宝可梦 → 送大木的包裹、拿图鉴 → 穿过常青森林到尼比市 → 灰色徽章（小刚，岩石系，用水/草）。

## 当前状态提示

- 第 18 轮，距离本次实玩结束约 425 秒。
- 如果 `/objectives` 还是默认内容（"Deliver Oak's Parcel · get Pokédex" 等），说明这是第一轮：先弄清楚自己处在开场的哪一步。
