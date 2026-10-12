# 交接文档：Go-Gui Markdown 查看器原型验证

> 编写：当前会话结束时的完整状态。供新 agent 在新对话中无缝继续。
> 敏感信息已脱敏：GitHub token 见下方【凭据】一节，仅限本人仓库使用。

## 任务背景
按 `docs/input.md` 的规划，验证「Go-Gui Markdown 查看器」的技术可行性，分 P0（空闲资源）/P1（超大文件）/P2（磁盘体积）三阶段，目标是产出"继续开发 or 更换方案"的决策依据。

**目标栈**（input.md 里的 "Go-Gui / go-edit / npad" 实为 go-gui-org 生态）：
- `go-gui`：纯 Go GUI 框架（Win32+WGL，Windows CGO-free，需 GL 3.3）
- `go-edit`：纯 Go 编辑器 widget（chroma 高亮、goldmark）
- **版本墙**：go-edit v0.11.0 锁定 go-gui **v0.78.0** + go-glyph **v1.25.2**（用 latest v0.87.0 会因 `Border` 类型 API 变化编译失败）。本地 `go.work` + `replace` 已对齐。

## 项目结构（D:\work\wild-edit）
- `ref/proto/cmd/`：原型与基准程序
  - `mdproto/`：左右分栏查看器原型（核心，已含 IME fix + 2MB 降级预览 + maxContentW 修复）
  - `p0_blank` / `p0_framecount` / `p0_pprof` / `p0_probe`：P0 空闲 CPU/内存探针
  - `p1_load`（`-raw` 用 FromBytes 绕 32MiB 上限）/ `p1_mdparse`：无头加载/解析基准
  - `gen_testdata`：生成 testdata/large 的下 15 个测试文件（1/10/32/64/128MiB × norm/struct/code）
- `ref/go-gui-v0.78.0`：go-gui v0.78.0 本地源码（replace 目标，**已打 2 处 patch**）
- `ref/go-edit-v0.11.0`：go-edit v0.11.0 本地源码（replace 目标，**已打 1 处 patch**）
- `ref/proto/build/`：交付 exe（`mdproto_fixed.exe`、`npad_fixed.exe`、`npad_official_editor.exe`）
- `scripts/`：`run_bench.sh`（一键基准）、`measure_mdproto.sh`（首帧+WSS 采样）
- `docs/`：全部报告（详见下文）

## 已完成并已验证的结论（详见各报告 doc）
| 项 | 结论 | 报告 |
|---|---|---|
| P0 空闲 CPU | ✅ ~0%（阻塞式事件循环，无后台帧，光标可停车） | `docs/P0_report.md` |
| P0 内存基线 M0 | Go 堆仅 12MB，但 WSS ~180MB（GPU/字体占 ~90%，headless 软渲染下） | `docs/P0_report.md` |
| P1 32MiB 限制 | ✅ 定位在 `buffer.Load` 的 `io.ReadAll`（`MaxLoadBytes=1<<25`）；`os.ReadFile`+`buffer.FromBytes` 可绕过 | `docs/P1_report.md` |
| P1 加载性能 | ✅ 线性快速：128MiB 266ms；内存 ~2.9-4.6x 文件 | `docs/P1_report.md` |
| P1 预览渲染 | ❌ go-gui `Markdown` 视图对整篇建 RTF 树，无虚拟化 → 10MiB 首帧 5.2s、128MiB >25s/2.5GB | `docs/P1_report.md` |
| P2 磁盘体积 | ✅ 单 EXE 18.7MB(stripped)/zip 6.4MB，CGO-free 自包含 | `docs/P2_report.md` |

## 本会话新完成的 3 件事（重点）

### 1. 中文输入法（IME）—— 已修复并验证 ✅（用户已在真实桌面确认"可以了"）
- **根因**：go-edit 编辑器用 `DrawCanvas` 自绘，无 TextChunk(TC)；go-gui v0.78 的 IME 激活判定 `shapeIsIMEEditTarget` 只看 `s.TC != nil` → 恒 false → `IMEStart()`(关联 IMM32) 永不调用。官方 npad 同样失效。
- **修复**（方向 1）：在 `ref/go-gui-v0.78.0/gui/ime_context.go` 的 `shapeIsIMEEditTarget` 增加：`A11YRole == AccessRoleTextArea && A11YState 无 ReadOnly` 时也视为 IME 目标（见上方代码）。探针实测 `imeAssociate(…, iaceDefault)` 现在会被调用。
- **配套**：`ref/proto/cmd/mdproto/main.go` OnInit 里 `w.SetFocus("mdproto-editor")`（编辑器需先获得焦点）。
- **已向官方提交 issue**：go-gui-org/go-gui **#999**（英文，含完整根因 + 双修复方案）。
- 报告：`docs/IME_report.md`

### 2. 大文件降级渲染 —— 已实施 ✅
- `ref/proto/cmd/mdproto/main.go` 加了阈值：`buf.Len() > 2<<20` 时右侧不调 `w.Markdown`，改为纯文本摘要（`gui.Text`，Mode=TextModeMultiline，提示 + 前 60 行）。
- 验证：128MiB 内存 2.5GB→**646MB**（降 4 倍）；1MiB(≤2MB) 仍完整渲染。
- 报告：`docs/P1_report.md` 的 P1-5

### 3. 修复 go-edit 第二个性能 bug —— 编辑器每帧全扫最大行宽 ✅
- **现象**：降级后稳态 CPU 仍 80%+ 空转。
- **根因**：`computeMaxContentWidth` 每帧对全部行做文本测量（供水平滚动条），因缓存键 `maxContentW/CacheLines/Dirty` 错放在**每帧丢弃的 `editorFrameData`** 里，每帧失效重扫。
- **修复**（`ref/go-edit-v0.11.0/edit/editor_state.go` + `editor_wrap_cache.go`）：缓存键移到跨帧持久的 `editorState`（`maxContentW/MaxContentCacheLines/MaxContentDirty`），frame 只作快照。
- **验证**：32MiB 稳态 CPU 从 81%→**0%**（computeMaxContentWidth 彻底消失）。
- **残余代价**：首帧仍要全扫一次最大行宽（一次性冷启动，10MiB≈5s、32MiB≈15s）。
- 报告：`docs/P1_report.md` 的 P1-6

## 风险评估 / 待完成项（按 input.md 阶段 A+B 准入门槛）
- ✅ 空闲 CPU、32MiB 解除、磁盘体积 —— 达标
- ⚠️ 长文档首帧：≤2MB 完整渲染；>2MB 降级后首帧受编辑器一次性全扫影响（数秒）。若需更快，做采样/异步计算最大行宽。
- ❌ 交互类验证未做（headless Session 0 无法）：最小化窗口、拖动、滚动/跳末尾、编辑输入延迟(P95)、预览更新延迟。**需在真实桌面会话补测**。
- ⚠️ 环境局限：本会话在 Session 0 headless（无真实桌面/输入法/真 GPU）。内存基线 M0≈180MB 是软件渲染下测的，真 GPU/RDP 下需重测。
- 错误/已知：go-edit `TestAtomicWritePreservesMode` 在 Windows 失败（file-mode 语义，与本会话修改无关，平台既有失败）。

## 建议技能（下一会话可调用）
- **prototype**：搭建可运行原型、探索 UI 设计选项
- **frontend-design** / **algorithmic-art**：美化查看器界面（若阶段 C 做正式的 UI 视觉）
- **grill-me** / **grill-with-docs**：对阶段 B/C 方案做压力测试与取舍
- **tdd**：阶段 C 功能（打开/保存/搜索）用测试驱动
- **webapp-testing**：若需要验证（但本项目是原生 GUI，更需真实桌面手动测试）
- **lark-* 系列**：若要把查看器/文档分享到飞书

## 下一步建议（按价值排序）
1. **阶段 C MVP**：把 mdproto 演进为正式查看器——文件树、打开/保存、外部变更提示、快捷键、主题（对应 input.md §3 架构的 `internal/` 模块化）。注意把 go-edit 封装在 `internal/editor` 后，勿直接改供应商。
2. **真机桌面验证**：补齐最小化/拖动/滚动/滚动等交互指标 + 真 GPU 下重测内存基线 + IME 在 Real 桌面最终确认。
3. **优化首帧全扫**（可选）：采样/异步算最大行宽，改善大文件打开体验。
4. 若跟进上游：go-gui issue #999 可能会有维护者回应；注意我们的本地 patch 尚未推 PR。

## 环境与凭据
- 构建环境：中文 Windows 10 64-bit，Go 1.26.6，git-bash。
- 本机可用：`D:/dev/Git/cmd/jq.exe`（JSON 优先）、`D:/dev/uv/ve/sys312/Scripts/python.exe`（venv）、curl 用 `C:/Windows/System32/curl.exe`，海外访问走本地代理 `http://127.0.0.1:10809`（不稳定，go 模块下载失败可重试或改用 curl 直下 zip）。
- **注意**：GUI 在本会话跑在 Session 0（headless，无可见桌面/输入法），真机验证需用户在桌面双击 exe。
- **GitHub 凭据**（仅本人仓库 rockswang 使用，勿外泄）：token 见项目 AGENTS.md 中的个人凭据节（`ghp_...`），用于 API/issue（本会话用它查/建了 go-gui issue #999，token 权限仅 pull/issue，无 push）。