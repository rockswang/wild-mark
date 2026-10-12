# 原型研究笔记

## 项目事实（来自 README / go.mod，2026-10-12 核实）

### go-gui-org/go-gui（GUI 框架）
- 仓库：`github.com/go-gui-org/go-gui`，最新版 **v0.87.0**（2026-10-11）
- 纯 Go GUI 框架，无浏览器运行时 / JS / HTML / CSS
- 要求 **Go 1.26+**
- **Windows 全 CGO-free**：桌面后端 = Win32 + WGL（OpenGL）；macOS 需 CGo（Metal），Linux = X11+EGL，网页 = WASM
- 混合即时模式 UI + retained widget 树；每帧由 view 函数重建 layout
- 渲染管线：`View fn → generateViewLayout() → Layout tree → layoutArrange() → renderLayout() → Backend`
- 内置：虚拟化 `ListBox/Table/Tree/VirtualList`、`Keyframe/spring/tween` 动画、`Markdown view`、`RTF`、`SVG`、DataGrid(CSV/XLSX/PDF 导出)
- 生态：go-glyph(文本)、go-charts、go-edit(编辑器)、go-map、go-term
- 调试：`GOGUI_DEBUG=1`、`GOGUI_DEBUG_REBUILDS=1`、`GOGUI_DEVICE_SCALE=2`
- 徽章：go-edit README 中的 npad 截图

### go-gui-org/go-edit（编辑器 widget）
- 仓库：`github.com/go-gui-org/go-edit`，最新版 **v0.11.0**
- **纯 Go，无 CGO**；语法高亮用 chroma v2；文本 shaping 用 go-glyph
- **已知文件大小上限：32 MiB**（README 明确标注）——P1 必须核实来源
- 缓冲区：per-line byte store，UTF-8 aware，grapheme-cluster 光标
- 编辑模型：`Buffer.Apply(Edit)` 是唯一 mutation 通道；filters/observers/undo 都经它
- DrawCanvas 不带 `ID` 以绕开 go-gui 的 render cache（缓冲区/光标/滚动每帧变化）
- 附带 `examples/npad` 示例（README 声称需 CGO 后端 SDL2/FreeType —— 待核实；go-gui Windows 后端是 Win32+WGL，可能不改用 SDL2）
- 头less 可测；核心单测无需后端
- 依赖：chroma/v2 v2.27.0、go-glyph v1.26.3、go-gui v0.87.0、goldmark v1.8.5、go-text/typesetting、uniseg、tdewolff/parse

## 关键待验证风险（对应 input.md 的 P0/P1/P2）
1. **P0 空闲 CPU**：即时模式每帧重建 UI，需验证静止/最小化窗口下是否还持续重绘、空闲 CPU 是否 ≤1%
2. **P1 32 MiB 上限**：限制在哪个环节（读写? 缓冲区?），能否解除或替换
3. **P1 大文件首屏**：goldmark 解析整篇 + 布局是否阻塞，能否首屏优先 / 分块
4. **P2 磁盘体积**：Release EXE、发布目录、依赖；CGO-free 应偏小

## 模块版本墙（proxy.golang.org 已核实）
- go-gui  v0.87.0
- go-edit v0.11.0
- go-glyph v1.26.3

## 32 MiB 限制来源（已定位，P1 关键）
- 位置：`go-edit/edit/buffer/buffer.go` 顶部
  - `MaxLoadBytes = 1 << 25 // 32 MiB`
  - `MaxLineBytes = 1 << 20 // 1 MiB`
- 作用：`Buffer.Load` / `Buffer.LoadFile` 用 `io.LimitReader(r, MaxLoadBytes+1)` 读全文，超限即报错 `"buffer: input exceeds %d bytes"`。
- **源码注释明确给出出路**：">大文件可手动流式读入 buffer"（Callers that need to load larger files can stream into a buffer manually）。
- 结论：限制来自加载器的一次性 `io.ReadAll`，而非底层存储能力，**理论可解除**。两条路：
  1. 本仓库自建 loader：读文件 → 转码 → 分块喂给 Buffer（不动依赖）
  2. 用 `replace` 指向本地 fork，把 const 调大（深度改造，风险大，先不选）
- 内存模型：`[]*line` 按行存储，注释称约 2x 文件大小的 Go 堆开销。128 MiB 文件 → 预计 >256 MiB 堆，需实测。
- `MaxLineBytes` 只影响单行超长文件（minified JS 等），遇到超长行会硬拆分，不影响普通 Markdown。

## npad 示例说明（P2 后端核实）
- `go-edit/examples/npad/main.go` 使用 `github.com/go-gui-org/go-gui/gui/backend`（**不是 SDL2**）。
- README 里 npad 需 SDL2/FreeType 的说法已过时；go-gui Windows 后端是 Win32+WGL，**纯 Go 无 CGO**。验证这点很重要——意味着整链在 Windows 上可 CGO-free 构建。

## 固件：go-gui 是 Go 1.26+，Windows 后端 Win32+WGL，混合即时模式。
- go-edit Buffer.Apply(Edit) 是唯一变更通道。
- 渲染缓存：DrawCanvas 用空 ID 绕开 go-gui render cache（内容每帧变）。