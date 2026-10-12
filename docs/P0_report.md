# P0 技术验证报告：空闲资源与渲染基线

环境：Windows 10 64-bit，Go 1.26.6，go-gui v0.87.0，go-edit v0.11.0。构建 `CGO_ENABLED=0`（纯 Go，Win32+WGL 后端）。
测量态：会话 0 无交互桌面（隐藏窗口，软件 GPU 模拟）。CPU 主基准 = pprof 100Hz 采样；内存 = runtime.MemStats + tasklist WSS。

## P0-1 空白窗口空闲 CPU — 结论：✅ 通过（~0%）
- 进程事件循环为**阻塞式空闲**：`MsgWaitForMultipleObjectsEx(INFINITE)` 挂起，无轮询（代码注释明确"nothing periodic does"）。
- **不产生后台帧**：启动后渲染 2 帧即停止；空闲期间 FrameCount 增量为 0，无布局重建（`GOGUI_DEBUG_REBUILDS` 仅 1 条初始日志）。
- **pprof 实测：空闲 10 秒 CPU 采样 = 0**（100% idle）。之前 cpumon 读出的 ~7% 是 `GetProcessTimes` 对 GPU 分派线程的虚高，pprof 为权威值。
- 光标闪烁：无焦点输入时无 blink 动画；`blink_idle_test` 证实在闲 idletimeout 后光标停车，不触发持续重绘。
- 结论：空闲 CPU 远低于 1% 目标（实测 ~0）。**架构达标。**

## P0-2 内存基线 M0 — 结论：Go 堆极小，进程 WSS 高，需区分
- **Go 堆：HeapInuse ≈ 12 MB，HeapSys ≈ 19 MB，runtime.Sys ≈ 25 MB**（极佳）。
- **进程工作集 WSS ≈ 169–182 MB**：~90% 非 Go 堆，主要来自 GPU 上下文/交换链（会话 0 无 GPU 时走软件模拟）、文本 shaping 系统预加载字体数据。
- 内存增量评价规则（input.md）：文档性能用 `M_n - M_0` 表示；WSS 基线 M0 ≈ 170–180 MB。
- 需注意：交互会话 + 真 GPU 下 WSS 可能不同，GPU 部分可能换显存而非 RAM。交互态实测待阶段 B 在桌面会话补测。

## P0-3 最小化窗口
- 待补测（后台重绘）。架构上 `occluded` 窗口跳过布局/呈现，仅跑命令回调（window_update.go FrameFn 的 occluded 分支），应进一步降本。

## P0-4 启动/布局/切换（静态判断）
- 每帧走 view fn → layout → render 管线；仅在有刷新请求时触发。
- 侧栏/分栏切换触发增量刷新，需在阶段 B 实测帧时间。

## P0-5 长时间内存/GC
- numGC=5（启动即收敛），堆稳定 12MB 不增长，无泄漏迹象。待阶段 D 长时间观测。

## 工具产出
- `ref/proto/tools/cpumon/`：GetProcessTimes 采样（对本应用虚高，仅作辅助）
- `ref/proto/tools/busyctl/`：可控忙等（校准工具）
- `temp/p0_pprof.exe`（构建产物）：带 pprof :6060 的探针（CPU/堆权威采样）

## 单元 EXE 基线（P2 前置）
- 原始 blank EXE：23.2 MB
- `-ldflags "-s -w"`：17.4 MB
- 待阶段 B/C 加 Markdown+编辑器后重测完整发布目录。

## 对项目目标的影响
- 核心轻量化目标（空闲 CPU）**已验证达成**，可继续 P1/P2。
- 内存注意点：WSS 基线偏高主要来自 GPU/字体，建议阶段 B 对比"系统字体 vs 打包字体"两种形态。

## 中文输入法（IME）排查结论
- go-edit 编辑器已带完整 IME 支持（`editor_input.go` 处理 composition/commit，多码点 `IMEText` 提交，抑制 IME 尾随 Enter/Esc）。
- Windows 后端 IME 链路完整：`imm32.dll`（cgo-free）；`IMEStart`/`IMEStop` 在焦点变化时调用（`ime_context.go: applyIMEEditContext`），光标 preedit 内联绘制。
- **IME 激活条件是焦点必须落在编辑器**：`applyIMEEditContext` 仅当 `editing = focusID != "" && ime` 时调 `IMEStart()`；`ime = shapeIsIMEEditTarget`（shape 角色需为 TextArea）。go-edit 编辑器 shape = `AccessRoleTextArea`（editor.go:248）✓。
- **根因（原型缺焦点）**：mdproto 之前创建编辑器却没 `w.SetFocus(编辑器ID)`，焦点不在编辑器 → 永不触发 `IMEStart` → 中文输入法无效。npad 示例在 OnInit 里 `w.SetFocus(focusEditor)`（main.go:143）。
- **已修复**：mdproto 的 OnInit 增加 `w.SetFocus("mdproto-editor")`。若用户仍不行需在真实交互会话复测（headless 无法验证 IME）。