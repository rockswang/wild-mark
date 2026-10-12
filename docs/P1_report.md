# P1 技术验证报告：超大文件加载与渲染

环境：Windows 10 64-bit，Go 1.26.6，go-gui **v0.78.0** + go-edit **v0.11.0**（对齐 go-edit go.mod 的版本墙），CGO-free。
测试集：`testdata/large/` 共 15 个文件（1/10/32/64/128 MiB × norm/struct/code 三种分布）。
测量：无头 p1_load（buffer 加载）、p1_mdparse（markdown 纯解析）、mdproto（左右分栏 GUI，首帧时间 + WSS 采样）。

## P1-1 32 MiB 限制 — 结论：✅ 已定位且可绕过
- 限制定位：`go-edit/edit/buffer/buffer.go` 的 **`MaxLoadBytes = 1<<25`**，作用在 `Buffer.Load/LoadFile` 的一次性 `io.ReadAll`。
- 实测：1 MiB✅ / 10 MiB✅ / **32 MiB → `ERROR: buffer: input exceeds 33554432 bytes`**（精确在 2^25）。
- **绕过方案（已跑通）**：`os.ReadFile` 读全文 + **`buffer.FromBytes(raw)`** 直接建 buffer（不走 Load 的 I/O 上限）。
  - 64 MiB、128 MiB 均成功加载（timing 见下）。**无需改依赖。**
- `MaxLineBytes=1MiB` 只影响超长单行（minified JS 类），普通 Markdown 不触发。

## P1-2 编辑器 buffer 加载性能（FromBytes，无头）— 结论：✅ 快且线性
| 尺寸 | 加载耗时 | 行数 | 堆增(norm) | 堆增(code) |
|---|---|---|---|---|
| 10 MiB | 21 ms | 173k | 29 MB | — |
| 32 MiB | 69 ms | 554k | 93 MB | 155 MB |
| 64 MiB | 129 ms | 1109k | 187 MB | — |
| 128 MiB | 266 ms | 2218k | 374 MB | 598 MB |

- 加载本身极快（128 MiB 266ms），线性。内存 ~2.9x (norm) ~4.6x (code) 文件大小（`[]*line` 按行结构开销）。
- 结论：**编辑器缓冲层支持超大文件，无瓶颈。**

## P1-3 Markdown 纯解析性能（无头，gui/markdown.Parse）— 结论：⚠️ 线性但内存大
| 尺寸 | parse 耗时 | blocks | 堆增 |
|---|---|---|---|
| 1 MiB | 19 ms | 8670 | 11 MB |
| 10 MiB | 192 ms | 86.7k | 111 MB |
| 32 MiB | 648 ms | 277k | 317 MB |
| 128 MiB | 2.97 s | 1109k | 987 MB |

- parse 线性（~8ms/MiB），但内存 ~7.7x 文件大小（128MiB→987MB 堆）。

## P1-4 预览渲染路径（GUI，mdproto 左编辑右预览）— 结论：❌ **核心瓶颈，不达标**

| 文档 | 首帧耗时 | 峰值 WSS | 备注 |
|---|---|---|---|
| 空白(基线) | 183 ms | 186 MB | 后端+GPU 初始化 |
| 1 MiB norm | ~1.0 s | ~485 MB | 相对基线 +800ms,+300MB |
| 10 MiB norm | ~5.2 s | ~575 MB | 首屏远超市 500ms 目标 |
| 32 MiB norm | ~16.8 s | ~900 MB | 首屏目标(1s)爆表 |
| 128 MiB norm | **>25 s 未完成** | **~2.5 GB 峰值** | 无法交互 |

- **根因**：go-gui 的 `markdownView.GenerateLayout` → `markdownBuildContent(blocks,...)` 在**每次布局**都把**整篇文档所有 block 转成 RTF/Container view 并做文本 shaping 布局**，无虚拟化、无可视区优先、无分块。1 MiB 就有 ~1300+ 个 block 视图，128 MiB 有 110 万块，每帧重建 → 二次方级耗时 + 数 GB 内存。
- 结论：**go-gui 原生 Markdown 预览对 >1 MiB 文档完全不适用于本产品的大文件目标。**

## 风险裁决（对照 input.md）
- **32 MiB 限制：已解除方案，绿。**
- **编辑器缓冲/加载：绿（线性、快、内存可控）。**
- **Markdown 解析：黄（线性但内存高，可优化）。**
- **预览渲染：红 —— 必须自建"首屏优先 + 可视区虚拟化 + 分块 parse/layout"或换轻量渲染路径。** 这是后续开发的核心工作量。

## 版本对齐备忘
- go-edit v0.11.0 go.mod 需要 go-gui **v0.78.0** + go-glyph **v1.25.2**（比 latest 旧）。用 v0.87.0 会 API 不兼容（Border 类型变了）。
- 本地 replace：`ref/go-gui-v0.78.0`（v0.78.0）、`ref/go-edit-v0.11.0`（v0.11.0）。
- npad 示例用 `gui/backend`（Win32+WGL），非 SDL2；整链 Windows CGO-free 构建 ✓。
## P1-5 降级预览效果（>2MB 不渲染完整 Markdown，已实施）
- 方案：`mdproto` 中 `buf.Len() > 2<<20` 时，右侧预览不调 `w.Markdown`，改为纯文本摘要（提示 + 前 60 行原文，`gui.Text`）。
- 效果对比：
  | 文档 | 未降级首帧 | 降级后首帧 | 未降级 WSS | 降级后 WSS |
  |---|---|---|---|---|
  | 10 MiB | 5.2 s | ~5 s | 596 MB | 419 MB |
  | 32 MiB | 16.8 s | **14.8 s** | 900 MB | 332–494 MB |
  | 128 MiB | >25 s | >15 s | 2.5 GB | **646 MB** |
- **内存大幅下降**（128MiB 2.5GB→646MB，4 倍），大文件不再 OOM/几GB。
- **但首帧耗时几乎没降**：瓶颈从预览转到编辑器。pprof 证实 32MiB 首帧 ~80% CPU 在 `editorAmendLayout → computeMaxContentWidth → XForColumn`（对全部行逐一文本测量算最大行宽，供水平滚动条）。
- 该全宽扫描仅行数变化时全量执行一次（`maxContentCacheLines != total`），是**一次性冷启动成本**；后续行数不变的编辑不重扫。
- 残留风险：超大文件首次打开仍要等一次 O(n) 全宽扫描（32MiB≈15s，128MiB 线性更长）。待决策是否 patch go-edit（采样/降频/异步算最大行宽）。

## P1-6 额外定位并修复：编辑器 maxContentW 每帧全扫 bug
在降级验证中发现并修复了 go-edit 的第二个性能 bug：

- **现象**：降级预览后内存降了，但稳态 CPU 仍持续 80%+，首帧后也不空闲。
- **pprof 定位**：`editorAmendLayout → computeMaxContentWidth → XForColumn` 对全部行逐一做文本测量算最大行宽（供水平滚动条）。
- **根因**：缓存键 `maxContentW/CacheLines/Dirty` 被错误放在**每帧丢弃的 `editorFrameData`** 里（其注释明言 "discarded at end of frame"）。每帧缓存归零 → `CacheLines != total` 恒成立 → **每帧对全篇做 O(n) 文本测量**。
- **修复**（本地 patch `ref/go-edit-v0.11.0/edit/editor_state.go` + `editor_wrap_cache.go`）：把 `maxContentW/MaxContentCacheLines/MaxContentDirty` 移到**跨帧持久**的 `editorState`，`frame` 只作每帧快照。`updateMaxContentWidth` 改用持久缓存，OnEdit observer 置持久 dirty 标志。
- **验证**：32MiB 稳态 CPU 从 81%（computeMaxContentWidth）→ **0%（该函数消失）**；首帧后编辑器不再空转。
- **代价**：首帧仍要全扫一次（首次算最大行宽），32MiB ≈15s、10MiB ≈5s 的一次性冷启动成本。对"不追求所见即所得、重点编辑"的大文件场景可接受。
- **回归**：go-edit edit/highlight/text 包测试通过；唯一失败的 `TestAtomicWritePreservesMode` 是 Windows 平台 file-mode 语义的既有失败，与本修复无关。
