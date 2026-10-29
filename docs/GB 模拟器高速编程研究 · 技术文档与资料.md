GB 模拟器高速编程研究 · 技术文档与资料
本文档是酒馆 Problem「Claude Opus 5.5 高速编程研究：从零写 GB 模拟器」的技术附件。所有链接已于 2026-10-28 核对可访问。
一、规格说明（需求文档）
这道题不另写需求文档，以 Pan Docs 为准。它是 GB 社区维护多年的硬件参考，只描述硬件「应该怎么表现」，不规定「怎么实现」。
	•	Pan Docs 总目录：https://gbdev.io/pandocs/
	•	硬件规格总览：https://gbdev.io/pandocs/Specifications.html
	•	内存映射：https://gbdev.io/pandocs/Memory_Map.html
	•	画面渲染（PPU）：https://gbdev.io/pandocs/Rendering.html
	•	卡带控制器 MBC3（宝可梦红用的就是这一种）：https://gbdev.io/pandocs/MBC3.html
	•	其他章节（定时器、中断、MBC1、MBC5、按键等）都在总目录的左侧导航里
二、CPU 指令参考
	•	指令表（网页版，含编码、周期数和标志位影响）：https://gbdev.io/gb-opcodes/optables/
	•	指令表（JSON 版，适合程序读取）：https://gbdev.io/gb-opcodes/Opcodes.json
	•	指令语义说明（RGBDS 文档）：https://rgbds.gbdev.io/docs/gbz80.7
三、验收用测试 ROM
1. Blargg 测试集（质量门槛，必须通过）
	•	仓库：https://github.com/retrio/gb-test-roms
	•	必须通过：cpu_instrs（11 项全部通过）、instr_timing、mem_timing
	•	判定方式：测试结果会打印到屏幕，同时从串口输出 Passed 或 Failed
	•	不需要做：dmg_sound、cgb_sound（声音不在本题范围内）
2. dmg-acid2（质量门槛，必须通过）
	•	仓库：https://github.com/mattcurrie/dmg-acid2
	•	ROM 下载（v1.0）：https://github.com/mattcurrie/dmg-acid2/releases/download/v1.0/dmg-acid2.gb
	•	判定方式：截图与仓库中的标准图逐像素比对，必须完全一致
3. Mooneye 测试集（加分项）
	•	仓库：https://github.com/Gekkio/mooneye-test-suite
	•	预编译 ROM：https://gekkio.fi/files/mooneye-test-suite/ （取最新一期）
	•	判定方式：测试结束时把结果写进 CPU 寄存器（通过时 B/C/D/E/H/L 依次为 3/5/8/13/21/34），按通过的个数计分
四、宝可梦红版实玩测试
1. 接入框架：pokemon-agent
	•	仓库：https://github.com/NousResearch/pokemon-agent （MIT 许可）
	•	作用：把模拟器包装成 HTTP 服务，让 AI agent 看截图、读内存状态（队伍、地图、坐标、战斗、对话）、发按键
	•	接入方式：实现 pokemon_agent/emulator.py 里的 Emulator 抽象接口（加载 ROM、按键、推进帧、取画面、读内存、存档和读档），用自己的模拟器替换默认的 PyBoy 后端
	•	内存地址解析在 pokemon_agent/memory/red.py，这部分不需要改
2. ROM
	•	由接单人自行合法获取，仅供内部研究使用
	•	版本统一为美版红 v1.0，SHA1 必须是：ea9bcae617fdf159b045185467ae58b2e4a48b9a
	•	校验值来源：pret/pokered 反汇编项目的 roms.sha1 文件：https://github.com/pret/pokered
3. 测试方式
	•	从开机、新建游戏开始（不提供存档）
	•	由 agent 自动玩 30 分钟，要求：
	◦	不崩溃、不卡死
	◦	能正常读取游戏状态
	◦	抽查截图，没有花屏、错位或黑屏
	◦	游戏确实在推进（地图或坐标有变化）
4. 已知注意点
	•	宝可梦红的随机数会读硬件计时器 rDIV，而且每帧都会调用（源码见 pret/pokered 的 engine/math/random.asm、home/vblank.asm）
	•	所以同一串按键放到不同的模拟器上，遇怪和战斗结果可能不一样。这是正常现象，本题验收不要求跨模拟器结果一致
五、参考资料（不是必读，可以用来设计提速方法）
1. Anthropic 关于长时间编程 agent 的实践
	•	用「规划者、生成者、评审者」三个 agent 写应用；同一需求下，单 agent 和多 agent 做法在耗时、成本、质量上的对比：https://www.anthropic.com/research/harness-design-long-running-apps
	•	跨会话工作：功能清单、进度文件、初始化 agent：https://www.anthropic.com/research/effective-harnesses-for-long-running-agents
	•	16 个 Claude 并行从零写 C 编译器：测试设计、并行方式、参考实现做 oracle：https://www.anthropic.com/research/building-c-compiler
2. 从零构建类 benchmark（验收设计参考）
	•	Zero2Repo（给 PRD，隐藏验收测试）：https://arxiv.org/abs/2609.38269
	•	E2E-SWE（从零生成整个代码仓库）：https://arxiv.org/abs/2609.38335
	•	SpecBench（衡量 agent 针对测试作弊的程度）：https://arxiv.org/abs/2605.21384
	•	GameXpert-Bench（游戏开发 agent 评测）：https://arxiv.org/abs/2608.21833
3. GB 开发社区资源汇总
	•	https://gbdev.io/resources.html
六、红线（重申）
	•	必须从零编写。不能拿现成的模拟器代码仓库来修改，也不能把现成代码复制进项目
	•	实现过程中可以查阅上面的文档，以及任何硬件资料
	•	交付时必须附上完整的 git 历史和全部 session 记录，供长老会核查
