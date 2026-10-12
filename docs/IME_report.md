# 中文输入法（IME）诊断报告 —— 根因已定位

## 现象
- 官方 go-edit 编辑器 `examples/npad`（Windows）中文输入法无效，无法上屏中文。
- 我们的 mdproto 同样无效（此前误判为焦点问题，补了 SetFocus 仍无效 → 证明不是焦点）。

## 根因（代码级定位，铁证）
go-gui v0.78 的 **IME 激活判定只有一条路径**，而 go-edit 编辑器恰好不满足它。

### 1. IME 激活链路
```
syncIMEEditContext()
  → findEditTargets(layout, w)        // 树中找焦点对应的 editable text 节点
      → shapeIsIMEEditTarget(s)       // 关键判定
            return s.TC != nil && s.focusOwner != "" && !s.TC.textReadOnly
  → applyIMEEditContext(ime)
      若 ime==true → np.IMEStart()    // 关联 Windows IMM32 上下文
```
- `np.IMEStart()` 调用 `ImmAssociateContextEx(hwnd, 0, iaceDefault)` —— 这是**让 IME 能工作的前提**。
- 注释明确：上下文在窗口创建时 detach，必须"可编辑文本控件取得焦点"才 IMEStart。

### 2. 断点：go-edit 编辑器没有 TextChunk（TC）
- go-edit 的 `Editor` 根 = Column + **`gui.DrawCanvas`**（自绘，`editor.go:201`），无 TC。
- DrawCanvas 是自定义绘制，**不产生 `TextChunk`**，所以：
  - `s.TC == nil` → `shapeIsIMEEditTarget` 恒为 **false**
  - → `IMEStart()` 从不调用 → IMM32 上下文不关联 → 中文输入法无法激活。
- 编辑器虽设了 `A11YRole: AccessRoleTextArea`，但 **IME 判定不看 A11Y role，只看 TC 实字段**（`ime_context.go:36`）。标了角色也无效。

### 3. 为什么后端看起来没问题
- go-gui Win32 后端 IMM32 链路**完整**（`ime_win32.go`：composition 拉取、commit、候选窗、preedit），且多 issue 已修。
- go-edit 也调用了完整 IME API（`IMEComposing/IMECompText/IMESetRect`）。
- 但**上游都没触发 `IMEStart`**，因为集成点（TC 判定）对 DrawCanvas 失效。
- 仓库 issues 无相关报告：这是 **go-edit 编辑器 × go-gui 渲染模型的集成缺陷**，不是单方 bug。

## 修复方向（已实施方向 1）

### 已实施：放宽 shapeIsIMEEditTarget 判定（方向 1）
- 修改 `ref/go-gui-v0.78.0/gui/ime_context.go` 的 `shapeIsIMEEditTarget`：
  - 保留 TC 路径（TextInput）；
  - **新增**：`s.A11YRole == AccessRoleTextArea && A11YState 无 ReadOnly` 时也视为 IME 目标。
  - 调用方 `findEditTargetsIn` 已保证传入的是焦点 shape，故不依赖 `focusOwner`（composite 容器自身不设 focusOwner）。
- 安全性：A11YRole=TextArea 只用于可编辑文本控件；真只读仍在 A11YState 置 ReadOnly 并被排除。
- 副作用面：`shapeIsIMEEditTarget` 唯一调用点是 IME gate（findEditTargetsIn），隔离无副作用。

### 验证（探针实测）
- 在 `imeAssociate` 加探针日志：修复前 npad 仅见 `flags=0`（IMEStop/detach），**修复后出现 `flags=16`（iaceDefault，IMEStart/附 IMM32 上下文）** —— 证明 IME 上下文已成功关联。
- go-gui `./gui/` 全套单测通过（含 IME/focus 相关）。
- `npad_fixed.exe`、`mdproto_fixed.exe` 已构建供真实桌面复测中文输入。

### 构建物（需用户在真实桌面双击验证）
- `ref/proto/build/npad_fixed.exe`：修复后的官方 go-edit 编辑器
- `ref/proto/build/mdproto_fixed.exe`：修复后的分栏原型
- 对比 `ref/proto/build/npad_official_editor.exe`（修复前，预期仍不可输入）

### 若仍不行（需继续排查）
- commit 是否落地到编辑器（go-edit `editorOnChar` 的 IMEText 多码点分支）。
- 候选窗光标锚定（IMESetRect）。
- 潜在：preedit 内联绘制是否被 DrawCanvas 覆盖。可再实施方向 2/3。

## 需要用户配合
- 由于我在 Session 0 headless（无真实桌面/输入法），**无法直接验证 IME**。
- 请用户在真实桌面双击这些 exe 复测：
  - `ref/proto/build/npad_official_editor.exe`（官方，当前必失败）
  - 应用方向 1 修复后的版本（预期可输入）