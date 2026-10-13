# wild-mark

**Windows 原生轻量 Markdown 编辑器 / 查看器**。不使用 WebView2、Chromium 或任何跨平台 GUI 框架——窗口、文本输入、字体排版全部复用 Windows 原生能力。

| 模块 | 选型 | 说明 |
|---|---|---|
| 语言 | Rust stable（msvc） | 主程序与全部业务逻辑 |
| 窗口/布局 | Win32（`windows` crate） | 原生窗口、菜单、分栏、文件对话框 |
| 编辑器 | **Scintilla 5.15**（`Scintilla.dll`） | 文本缓冲、行号、增量语法高亮、撤销、查找、IME |
| Markdown 高亮 | **Lexilla 5.30** `markdown` lexer | 走 `SCI_SETILEXER`；容器自染为降级路径 |
| Markdown 解析 | `pulldown-cmark` 0.13 | 事件流式解析，零拷贝 |
| 预览渲染 | **RichEdit 4.1**（系统 `Msftedit.dll`） | Markdown → RTF → `EM_STREAMIN`，只读预览 |

> 设计要点：预览链路的 RTF 是**内存中间产物**，中文默认以 `ansicpg936` + GBK 原始字节写入（体积 x1.10、生成快 3.9 倍），RichEdit 只读显示时对用户完全无感；异常字符自动回退 `\u` 转义。

## 状态

**原型验证已完成并全部达标**，十条准入门槛对照见验证报告。关键实测：

| 指标 | 实测 |
|---|---|
| 发布体积（空窗口 exe） | **0.24 MiB** |
| 空闲 CPU（600 s 静止段） | **0.009%** 单核等效（median 0.000%） |
| Rust GUI 自身私有工作集 | **< 1 MiB**（可见窗口的 21.7 MiB 中大头是第三方输入法+系统 UI 栈注入） |
| 1 / 10 MiB 打开至可交互 | **27 ms / 37 ms**（128 MiB 569 ms，106 万行） |
| 128 MiB 文档键入延迟 | p50 0.79 ms / p95 3.69 ms |
| 1 MiB 预览（解析+RTF+EM_STREAMIN） | **159 ms** |
| 中文 IME 端到端 | 通过（拼音组合 → “你好”上屏，组合未被父窗口消息处理中断） |

已知需要在正式版落地的修正项（均为工程细节，非架构缺陷）：

1. 预览整篇刷新后**光标/选区会重置**（滚动位置正常保持）→ 刷新前 `EM_EXGETSEL`、刷新后 `EM_EXSETSEL`。
2. 10 MiB 以上文档预览超过 1 s 门槛 → 大文件模式（延迟解析、关闭实时预览），阈值默认 8 MiB。
3. RichEdit 控件首次灌入有 ~200 ms 一次性开销 → 启动时预热控件。
4. TSF 中文输入**不产生 `WM_CHAR`** → 输入处理基于 `SCN_MODIFIED` / `SCN_CHARADDED`。
5. 长代码行在 RichEdit 中会折行（无横向滚动）；如需代码区不折行再评估自绘。

## 构建

```bash
# 1) 原生库（需要 VS2022 + Windows SDK，MSVC nmake）
build_native.bat

# 2) Scintilla 常量绑定（从头文件生成，避免手抄消息号）
python scripts/gen_sci_const.py

# 3) 构建
cargo build --release

# 4) 单元测试（RTF 转义与结构回归，15 项）
cargo test --release

# 5) 测试数据（1~128 MiB，可再生）
target/release/gen_testdata.exe --large
```

产物：`target/release/*.exe`（0.24~0.56 MiB）+ `native/scintilla/bin/Scintilla.dll`（0.78 MiB）+ `native/lexilla/bin/lexilla.dll`（1.18 MiB）。

## 目录

```
src/
  win.rs          Win32 基础：DPI、窗口类、消息循环、菜单、截图
  metrics.rs      进程内资源自采样（私有/共享工作集、CPU、GDI、句柄、模块）
  harness.rs      场景时间线、参数、JSONL 汇总
  scintilla.rs    Scintilla 薄 FFI（LoadLibrary + SendMessageW + 常量绑定）
  md2rtf.rs       Markdown → RTF（转义/Unicode/GBK、表格、代码块、链接 field）
  richedit.rs     RichEdit 4.1 封装（EM_STREAMIN、滚动、选区、剪贴板）
  sci_const.rs    由 scripts/gen_sci_const.py 生成（1070 个常量）
  bin/            p0_blank / p1_scintilla / p2_preview / mem_probe / gen_testdata
tests/            RTF 转义与结构回归测试
scripts/          gen_sci_const.py（常量生成）、run_task.py（交互桌面投递）、bmp_stats.py、rtf_check.py
native/           Scintilla/Lexilla 源码与构建产物（不入库，build_native.bat 可再生）
ref/              Go 路线归档与上游参考源码（不入库）
```

## 重要工程约定

- **GUI 测量必须在交互桌面（session 1）进行**：在服务上下文（session 0）创建窗口不会真正绘制，测出的内存/渲染全失真（实测 5.6 vs 41.9 MiB）。`scripts/run_task.py` 负责经计划任务投递到交互会话。
- **不得直接修改 `native/` 下的上游源码**：Scintilla/Lexilla 用官方源码原样构建；需要改动时通过补丁脚本注入并记录。
- 提交范围见 `.gitignore`：`ref/`、`temp/`、`testdata/`、`docs/`、`artifacts/`、`native/`、`target/` 默认不入库；`docs/` 采用白名单逐篇确认。

## 许可证

[MIT](LICENSE)。第三方组件（Scintilla / Lexilla / windows-rs / pulldown-cmark）许可证与链接方式见 LICENSE 附录。
