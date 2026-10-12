// 正式版：Go-Gui Markdown 查看器。
// 版本锁定：go-edit v0.11.0 依赖 go-gui v0.78.0（更高版本会因 Border API 变化而编译失败）。
// 本地补丁的 go-gui / go-edit 源码位于 ref/，由仓库根的 go.work 统一 replace。
module github.com/rockswang/mdviewer

go 1.26.0

require (
	github.com/go-gui-org/go-edit v0.11.0
	github.com/go-gui-org/go-gui v0.78.0
)

require (
	github.com/alecthomas/chroma/v2 v2.27.0 // indirect
	github.com/dlclark/regexp2/v2 v2.2.2 // indirect
	github.com/ebitengine/purego v0.10.2 // indirect
	github.com/go-gui-org/go-glyph v1.25.2 // indirect
	github.com/go-pdf/fpdf v0.9.0 // indirect
	github.com/go-text/typesetting v0.3.5 // indirect
	github.com/godbus/dbus/v5 v5.2.2 // indirect
	github.com/jezek/xgb v1.3.1 // indirect
	github.com/rivo/uniseg v0.4.7 // indirect
	github.com/tdewolff/parse/v2 v2.8.16 // indirect
	github.com/yuin/goldmark v1.8.5 // indirect
	github.com/yuin/goldmark-emoji v1.0.6 // indirect
	golang.org/x/image v0.46.0 // indirect
	golang.org/x/sys v0.48.0 // indirect
	golang.org/x/text v0.42.0 // indirect
)
