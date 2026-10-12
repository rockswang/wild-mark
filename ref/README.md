# ref/ — 参考源码与原型归档（不参与正式版开发）

本目录存放**参考/历史**内容，正式版代码不从这里 import。除 `proto/` 外，其余目录均被 `.gitignore` 忽略（可本地重建）。

## 目录说明

| 路径 | 内容 | 是否入库 |
|---|---|---|
| `go-gui-v0.78.0/` | go-gui v0.78.0 源码，**已打 2 处本地补丁**（IME 目标判定、编辑器最大行宽缓存） | ❌ 由 `proto/scripts/fetch_refs.sh` 解包，补丁需手动应用 |
| `go-edit-v0.11.0/` | go-edit v0.11.0 源码，**已打 1 处本地补丁**（最大行宽缓存跨帧持久） | ❌ 同上 |
| `go-gui-v0.87.0/` | go-gui v0.87.0 源码，仅作 API 差异对照（与 go-edit v0.11.0 不兼容） | ❌ 同上 |
| `dl/` | 上游模块 zip 下载缓存 | ❌ |
| `proto/cmd/` | 原型与基准程序（mdproto、p0_*、p1_*、gen_testdata） | ✅ 冻结归档 |
| `proto/scripts/` | 基准脚本与 `fetch_refs.sh` | ✅ |
| `proto/tools/` | 原型期临时小工具（cpumon / busyctl / sleepctrl） | ✅ 源码 |
| `proto/build/` | 原型交付 exe | ❌ |
| `proto/artifacts/` | 原型期日志 / 探针 exe / 下载 HTML / issue JSON 等过程产物 | ❌ |

## 版本墙（重要）

`go-edit v0.11.0` 锁定 `go-gui v0.78.0` + `go-glyph v1.25.2`。
若用 go-gui latest（v0.87.0）会因 `Border` 类型 API 变化而**编译失败**。

## 重建参考源码

```bash
# 1) 把模块 zip 放入 ref/dl/（或走代理下载，详见脚本头部注释）
# 2) 解包
bash ref/proto/scripts/fetch_refs.sh
# 3) 手动应用本地补丁：docs/IME_report.md、docs/P1_report.md
```

## 本地补丁摘要

- **go-gui `gui/ime_context.go`**：`shapeIsIMEEditTarget` 额外认可 `A11YRole == AccessRoleTextArea`（且非 ReadOnly）→ 修复中文输入法无法激活。已提上游 issue go-gui-org/go-gui **#999**。
- **go-edit `edit/editor_state.go` + `editor_wrap_cache.go`**：`maxContentW/MaxContentCacheLines/MaxContentDirty` 从每帧丢弃的 `editorFrameData` 移到跨帧持久的 `editorState` → 修复每帧全扫最大行宽导致的空闲 CPU ~80%。

## 原型程序一览（`proto/cmd/`）

| 程序 | 用途 |
|---|---|
| `mdproto` | 左右分栏查看器原型（go-edit 编辑器 + go-gui 预览，含 2MiB 降级预览） |
| `p0_blank` / `p0_framecount` / `p0_pprof` / `p0_probe` | P0 空闲 CPU / 内存 / 帧计数探针 |
| `p1_load` / `p1_mdparse` | P1 无头加载与解析基准（`-raw` 用 `FromBytes` 绕 32MiB 上限） |
| `gen_testdata` | 生成 `testdata/large/` 下 15 个测试文件 |

详见 `docs/HANDOFF.md`、`docs/P1_report.md`、`docs/IME_report.md`。
