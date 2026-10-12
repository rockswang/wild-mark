# 原型验证总结（阶段 A —— 技术验证）

三阶段验证（P0 空闲资源 / P1 超大文件 / P2 磁盘体积）已完成。目标栈：go-gui + go-edit（go-gui-org 生态，CGO-free 原生 GPU）。

## 逐项结果

| 验证项 | 结论 | 关键数据 |
|---|---|---|
| **P0 空闲 CPU** | ✅ 达标 ~0% | 阻塞式事件循环，无后台帧，光标闪烁可停车；pprof 10s 采样 0 |
| P0 空闲内存(M0) | ⚠️ 注意 | Go 堆仅 12MB，但进程 WSS ~170-186MB（GPU/字体占 ~90%） |
| **P1 32 MiB 限制** | ✅ 可绕过 | `buffer.Load` 限制于 2^25；`os.ReadFile` + `FromBytes` 可加载 128 MiB |
| P1 buffer 加载 | ✅ 线性快速 | 128 MiB 266ms；内存 ~2.9-4.6x 文件 |
| P1 markdown 解析 | ⚠️ 线性但内存高 | 128 MiB 3s / 987MB 堆 |
| **P1 预览渲染** | ❌ **不达标** | 10MiB 首帧 5.2s WSS 575MB；32MiB 16.8s/900MB；128MiB >25s 未完成/2.5GB |
| **P2 磁盘体积** | ✅ 轻量 | 单 EXE 18.7MB(stripped)，zip 6.4MB，CGO-free 无浏览器运行时 |
| CGO-free 可行性 | ✅ | Windows 整链 Win32+WGL 无 CGO，构建/运行均通过 |

## 核心结论
1. **框架轻量化底线达标**：空闲 CPU ~0%、编译/运行 CGO-free、单文件轻量发布。
2. **大文件编辑可行**：32 MiB 限制可绕过，buffer 加载线性快速、内存可控。
3. **大文件预览是硬伤**：go-gui 原生 Markdown 预览对整篇文档**逐块转 view + 全量 RTF 文本 shaping 布局**，无虚拟化/可视区优先/分块，导致二次方级耗时与 GB 级内存。10 MiB 即超首屏目标（5.2s），128 MiB 完全不可用。

## 建议路线（对齐 input.md 阶段 A 的决策门槛）
- **继续推进整体方案可行**，但必须把"**自建首屏优先 + 可视区虚拟化 + 分块解析/排版**的 Markdown 预览"列为阶段 B 前置攻坚，而不是用 go-gui 现成 `Markdown` 视图。
- 备选：预览极简化为"按需分块渲染"（例如先渲染可视区 heading+正文摘要，滚动时按块懒加载），规避全量 RTF 布局。编辑器侧已验证可用。
- 内存注意：M0 WSS ~180MB 来自 GPU/字体，若交互会话真 GPU 下应显著下降；对比"系统字体 vs 打包字体"再定发布策略。

## 交付物
- 可运行原型：`ref/proto/cmd/p0_blank`、`ref/proto/cmd/p0_probe`、`ref/proto/cmd/p0_pprof`、`ref/proto/cmd/p0_framecount`、`ref/proto/cmd/p1_load`、`ref/proto/cmd/p1_mdparse`、`ref/proto/cmd/mdproto`（分栏预览原型）
- 测试数据生成：`ref/proto/cmd/gen_testdata`
- 基准脚本：`scripts/measure_mdproto.sh`（首帧 + WSS 采样）
- 报告：`docs/P0_report.md`、`docs/P1_report.md`、`docs/P2_report.md`、`docs/research.md`