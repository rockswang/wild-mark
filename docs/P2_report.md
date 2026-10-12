# P2 验证报告：磁盘体积与依赖

环境：Windows 10 64-bit，Go 1.26.6，go-gui v0.78.0 + go-edit v0.11.0，CGO-free。
满功能 build = mdproto（编辑器 + Markdown 预览 + 全部依赖）。

## P2-1 单二进制体积
| 形态 | 体积 | 说明 |
|---|---|---|
| Raw release（-trimpath） | 25.2 MB | 含调试/符号 |
| Stripped（-ldflags "-s -w"） | **18.7 MB** | 交付建议形态 |
| Zip 压缩包（7z -mx9） | **6.4 MB** | 近似安装包 |

- 附录：纯空白窗口 stripped = 17.4 MB；加编辑器+Markdown 后 18.7 MB，增量 ~1.3 MB（Markdown 相关代码与 chroma）。

## P2-2 发布目录
- **单一 EXE，自包含**。无独立 DLL/运行库/资源目录。
- 图标字体 `feathericon.ttf` 通过 `go:embed` **打进 EXE**。
- 正文字体依赖**系统字体**（go-glyph 读 Windows 系统字体）。→ 不在 EXE 内，发布体积不因中文字体增大。
- 发布目录 = 单个 mdviewer.exe（18.7 MB）。220 KB 与 25 MB 的差异即符号表。

## P2-3 DLL/原生依赖
- Windows 后端 Win32+WGL，**全链路 CGO-free**（`CGO_ENABLED=0`）。
- npad/示例用 `gui/backend`，非 README 过时的 SDL2/FreeType。✅
- 运行仅依赖系统 DLL（kernel32/user32/opengl32 等）。

## P2-4 安装后/首启缓存
- 无额外配置、字体、索引缓存目录（首启无大缓存生成）。
- GPU 上下文/字体 run 时在内存，不落盘。安装后磁盘 ≈ EXE 体积。

## 版本兼容（必须对齐，否则编译失败）
- go-edit v0.11.0 go.mod 锁定 go-gui **v0.78.0** + go-glyph **v1.25.2**。
- 用 latest go-gui v0.87.0 会因 `Border` 类型改动编译失败。→ 本地 replace 已对齐 v0.78.0。

## 裁决
- **体积轻量（单 EXE ≤19MB，zip ≤6.5MB），无浏览器运行时，CGO-free。** ✅
- 相比"浏览器运行时"方案（动辄几十上百 MB），本方案磁盘优势明显。
- 风险低。若后续打包中文字体需另测体积增量（当前未打包，靠系统字体）。

## WSS 内存构成（尽力归因，offline GL 下）
- Go 堆仅 ~12MB；非堆 ~156MB 主要来自：
  1. **go-glyph 字形图集**：默认 1024×1024 RGBA=4MB/页，随字形累积可涨到 4096×4096=64MB/页；另有 4096-entry shape/字形缓存。软件 GL 下这些纹理在进程内存。
  2. **字体分析**（go-text/typesetting + FreeType 纯 Go 解析系统字体，多字体族预加载）。
  3. WGL/软件交换链缓冲。
- **优化方向**：真 GPU 下图集转显存（RAM 降）；限制图集最大尺寸/缓存条目；按需懒加载字体而非预载全部；复用字形缓存、控制 DPI 缩放。