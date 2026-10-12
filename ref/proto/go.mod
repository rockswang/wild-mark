// 原型验证模块（P0/P1/P2 探针与基准，已冻结归档）。
// 独立于根模块，便于正式版开发时互不干扰；仍可通过仓库根的 go.work 一起构建。
module github.com/rockswang/mdviewer/ref/proto

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

// 本地已手工打补丁的参考源码（IME 修复 + 编辑器最大行宽缓存修复）。
replace github.com/go-gui-org/go-gui => ../go-gui-v0.78.0

replace github.com/go-gui-org/go-edit => ../go-edit-v0.11.0
