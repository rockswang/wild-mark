# 阶段 B 技术决策：渲染降级 与 字体策略

## 1. 大文件降级渲染（暂缓虚拟滚动）
- 目标：让超大文件「能打开、能看、能滚」，放弃实时全量 WYSIWYG。
- 方案分层（映射 input.md 的首屏优先/可视区优先）：
  1. **尺寸分档**：默认走完整 Markdown 视图（WYSIWYG，适合 ≤1MiB）；超过阈值自动降级。
  2. **降级渲染**：大文件时右侧预览改为「**分块纯文本/结构骨架**」：
     - 只渲染 heading 骨架（TOC/导航）+ 当前可视区附近的原文行，滚动时按块懒加载。
     - 不建全量 RTF 树，不做全篇文本 shaping 布局（这正是当前瓶颈，见 P1）。
  3. **编辑遮罩**：编辑器侧已验证 Linear 快速（32MiB 从 bytes 加载 69ms），无需降级。
- 验收线：128MiB 打开 ≤1s 可见首屏、滚动可交互；内存可控（e.g. ≤600MB 增量）。
- 后续阶段再引入真虚拟滚动（可视区裁剪 block → view）。

## 2. 渲染后端可行性：集显 / RDP 虚拟显卡
- go-gui Windows 后端 = **Win32 + WGL**，要求 **OpenGL 3.3**。
- **集显（Intel/AMD UMA）**：WGL 能拿到 GL 3.x context，可行。纹理/字形图集走共享内存（系统 RAM），因此**集显应复用 go-glyph 图集为进程/共享内存而非独显显存**，内存观感与离线 GL 接近。
- **RDP 虚拟显卡 / VM**：RDP 原生 GL 能力弱，可能只有 GL 1.1/GDI 泛型。**go-gui 自带软件回退**：可把 **Mesa 软渲染 `opengl32.dll`（GL3.3 软件实现）与 exe 同目录**，`initOpenGL32` 会优先加载它。这是异构/远程环境的兜底。exe 从 18.7MB 变成 exe+Mesa dll（llvmpipe 约 10–25MB）。
- **风险等级**：集显=低；RDP=中（需捆绑 Mesa dll 或接受性能下降）；务必在真实集显、RDP、VM 三个环境各跑一次 P0 空闲 + 启动冒烟。
- 需实测项：GL3.3 context 是否在混合 RDP posfine 上获得；软渲染下的空闲 CPU 与字图集内存。

## 3. 字体策略：锁定少数系统字体，禁止自定义
- **结论性策略**（响应确定需求）：
  - **Editor 一律用等宽字体（monospace）**；**Preview 用系统黑体/宋体**。多平台分别选系统常见字体。
  - 字体**读系统、不打包**（与 P2 体积目标一致：exe ≤19MB）。
  - **不允许用户改字体** → 避免为每个字体预载字形/缓存，显著降低内存与首启开销。
- 具体映射（跨平台）：
  | 角色 | Windows | macOS | Linux |
  |---|---|---|---|
  | 等宽(编辑器) | Consolas / Cascadia Mono | SF Mono / Menlo | DejaVu Sans Mono |
  | 黑体(预览正文) | Microsoft YaHei UI | PingFang SC | Noto Sans CJK SC |
  | 宋体(预览备选/标题) | SimSun / NSimSun | STSong | Noto Serif CJK SC |
  | 图标 | feathericon.ttf（已内嵌，必须保留） | 同左 | 同左 |
- **懒加载**：只按需加载实际用到的 2~3 个字体族；字形图集随用随建，且可限制最大尺寸（`NewTextSystemAtlasSize`）。
- **收益**：固定字体 → 字形缓存命中率高、图集稳定 → 内存与重绘可预期；也简化各平台回归测试。
- 预留：若某平台缺字体，回退到 `sans-serif`/`monospace` 通用名（go-glyph 系统字体解析支持）。

## 4. 阶段 B 实施清单（优先级）
1. 集显/RDP/VM 三环境各跑一次 P0 空闲 + 启动冒烟（定可行性与 Mesa 需要）。
2. 实现「大文件降级渲染」分档与骨架预览（含可视区懒加载）。
3. 固化字体：Editor=monospace、Preview=系统黑体/宋体，禁用自定义；按系统选择字体，懒加载。
4. 限制字形图集尺寸/缓存，压 WSS 基线。