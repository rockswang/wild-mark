# wild-markdown

基于 **go-gui-org 生态** 的纯 Go Markdown 查看器（CGO-free，Windows 优先）。

- Go module：`github.com/rockswang/wild-markdown`
- 仓库：https://github.com/rockswang/wild-markdown

> 当前状态：**原型验证已完成**（P0 空闲资源 / P1 超大文件 / P2 磁盘体积），结论见
> [`docs/HANDOFF.md`](docs/HANDOFF.md)。**正式版开发即将在根目录按 Go 项目惯例启动。**

## 技术栈与版本墙

| 组件 | 版本 | 说明 |
|---|---|---|
| Go | 1.26+ | CGO-free，单文件交付 |
| [go-gui](https://github.com/go-gui-org/go-gui) | **v0.78.0**（本地补丁） | 纯 Go GUI（Win32 + WGL，需 OpenGL 3.3） |
| [go-edit](https://github.com/go-gui-org/go-edit) | **v0.11.0**（本地补丁） | 编辑器 widget（chroma 高亮、goldmark） |
| go-glyph | v1.25.2 | 字形渲染（由 go-edit 锁定） |

**版本墙**：`go-edit v0.11.0` 锁定 `go-gui v0.78.0`；用 go-gui latest（v0.87.0）会因 `Border` API 变化而编译失败。

## 关键验证结论

| 项 | 结论 |
|---|---|
| P0 空闲 CPU | ✅ 阻塞式事件循环，空闲 ~0%（pprof 10s 零采样） |
| P0 内存基线 | ⚠️ Go 堆仅 12MB，WSS ~180MB（GPU/字体占大半，软渲染下） |
| P1 32MiB 上限 | ✅ 定位在 `buffer.Load` 的 `io.ReadAll`；`os.ReadFile` + `buffer.FromBytes` 可绕过，128MiB 加载 266ms |
| P1 预览渲染 | ❌ go-gui 原生 Markdown 视图无虚拟化 → 需自建可视区虚拟化预览 |
| P2 磁盘体积 | ✅ 单 EXE 18.7MB（stripped），CGO-free 自包含 |
| IME 中文输入 | ✅ 已定位并修复（go-gui 上游 issue #999） |

各阶段详细报告：`docs/P0_report.md`、`docs/P1_report.md`、`docs/P2_report.md`、`docs/IME_report.md`、`docs/B_decisions.md`。

## 目录结构

```
.
├── go.mod / go.sum / go.work   # 根模块 = 正式版；go.work 统一注入本地补丁
├── README.md
├── docs/                       # 需求、研究报告、决策（input.md / *_report.md / HANDOFF.md）
├── testdata/                   # 测试数据（large/ 由生成器重建，不入库）
└── ref/                        # 参考源码与原型归档（详见 ref/README.md）
    ├── go-gui-v0.78.0/         #   [gitignore] 已打补丁的上游源码
    ├── go-edit-v0.11.0/        #   [gitignore] 已打补丁的上游源码
    ├── go-gui-v0.87.0/         #   [gitignore] 仅作 API 差异对照
    ├── dl/                     #   [gitignore] 模块 zip 缓存
    └── proto/                  #   原型模块（独立 go.mod，冻结归档）
        ├── cmd/                #     mdproto / p0_* / p1_* / gen_testdata
        ├── scripts/            #     基准脚本 + fetch_refs.sh
        └── tools/              #     原型期小工具
```

## 环境准备

参考源码不入库，首次克隆后需重建（详见 [`ref/README.md`](ref/README.md)）：

```bash
# 1) 下载模块 zip 到 ref/dl/（URL 见脚本头部注释）
# 2) 解包
bash ref/proto/scripts/fetch_refs.sh
# 3) 手动应用本地补丁（IME #999、编辑器最大行宽缓存），见 docs/IME_report.md / docs/P1_report.md
```

## 构建与运行

```bash
# 正式版（根模块）——开发中
go build ./...
go run ./cmd/wild-markdown <file.md>   # 目标入口（待实现）

# 原型（ref/proto 独立模块，通过 go.work 一并构建）
go run ./ref/proto/cmd/mdproto testdata/large/md_1MiB_norm.md
bash ref/proto/scripts/run_bench.sh
```

## 正式版开发计划（阶段 C）

1. **MVP**：文件打开/保存/另存、最近文件、外部变更提示、快捷键、主题。
2. **大文件**：可视区虚拟化预览（骨架 + 懒加载），替代全篇 RTF 构建。
3. **模块化**（`internal/`）：
   - `internal/app` 应用装配与生命周期
   - `internal/editor` 编辑器封装（**不直接改供应商**，补丁经 go.work 注入）
   - `internal/preview` 预览渲染（虚拟化）
   - `internal/doc` 文档模型（加载/编码/换行/大文件分档）
   - `internal/fsx` 文件监视与原子写
4. **质量**：`go vet` / `golangci-lint` / 单元测试（`go test ./...`，`tdd` 驱动）。

## 备注

- 临时文件统一放仓库根 `temp/`（已在 `.gitignore`，勿用系统临时目录）。
- 本地补丁目前仅存在于 `ref/`，尚未向 go-gui 提交 PR。
