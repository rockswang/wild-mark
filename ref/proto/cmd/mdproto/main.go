// Command mdproto: P1/P2 阶段的左右分栏 Markdown 查看器原型。
//   左: go-edit 编辑器（用 FromBytes 绕过 32 MiB 限制加载）
//   右: go-gui 原生 Markdown 预览（w.Markdown）
// 用法:  mdproto [文件] [--wide]   ; 打开指定文件做首屏/性能采样
package main

import (
	"fmt"
	"os"
	"runtime"
	"strings"
	"time"

	"github.com/go-gui-org/go-edit/edit"
	"github.com/go-gui-org/go-edit/edit/buffer"
	"github.com/go-gui-org/go-gui/gui"
	"github.com/go-gui-org/go-gui/gui/backend"
)

// app 持有按窗口类型化状态。
type app struct {
	Buf       *buffer.Buffer
	Path      string
	loadMs    int64 // 加载耗时(ms)，用于启动日志
	firstView bool  // 是否已记录首帧
}

// previewMaxBytes 超过该字节数的文档停用完整预览（避免全量 RTF 构建）。
// 2 MiB 为保守阈值：10 MiB 完整预览首帧 5.2s / 596MB，不可接受（见 P1）。
const previewMaxBytes = 2 << 20

// procStart 记录程序进程起始时间，供首帧耗时计算。
var procStart = time.Now()

func main() {
	path := ""
	if len(os.Args) > 1 && os.Args[1] != "" && os.Args[1][0] != '-' {
		path = os.Args[1]
	}
	fname := "untitled"
	if path != "" {
		fname = path
	}

	w := gui.NewWindow(gui.WindowCfg{
		State:  &app{},
		Width:  1400,
		Height: 900,
		Title:  "MD Viewer Proto - " + fname,
		OnInit: func(w *gui.Window) {
			st := gui.State[app](w)
			t0 := time.Now()
			if path != "" {
				if buf, err := loadLarge(path); err == nil {
					st.Buf = buf
					st.Path = path
				} else {
					fmt.Fprintf(os.Stderr, "load %s: %v\n", path, err)
					st.Buf = buffer.New()
				}
			} else {
				st.Buf = buffer.New()
				st.Buf.Apply(buffer.Edit{NewBytes: []byte("# MD Viewer 原型\n\n左栏编辑，右栏实时预览。用 `go run ./cmd/mdproto <file>` 打开大文件。\n")})
			}
			st.loadMs = time.Since(t0).Milliseconds()
			fmt.Fprintf(os.Stderr, "[proto] 文件加载完成: %d ms, 行数=%d\n",
				st.loadMs, st.Buf.LineCount())
			w.SetView(mainView)
			// 关键：编辑焦点必须落到编辑器，IME 才被激活。
			w.SetFocus("mdproto-editor")
		},
	})
	backend.Run(w)
}

// heapMB 返回当前 Go 堆分配 MB。
func heapMB() int64 {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return int64(m.HeapAlloc >> 20)
}

// loadLarge 用 os.ReadFile + buffer.FromBytes 绕过 32 MiB 上限。
func loadLarge(path string) (*buffer.Buffer, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return buffer.FromBytes(raw), nil
}

func mainView(w *gui.Window) gui.View {
	st := gui.State[app](w)
	theme := gui.CurrentTheme()
	ww, wh := w.WindowSize()
	editorW := float32(int(ww)*45/100 - 20)
	editorH := float32(wh) - 30
	return gui.Splitter(gui.SplitterCfg{
		Sizing:      gui.FillFill,
		Orientation: gui.SplitterHorizontal,
		Ratio:       gui.SomeF(0.45), // 左编辑45% 右预览55%
		First: gui.SplitterPaneCfg{
			MinSize: 200,
			Content: []gui.View{
				// 左：编辑器
				gui.Column(gui.ContainerCfg{
					Sizing:  gui.FillFill,
					Padding: theme.PaddingMedium,
					Content: []gui.View{
						edit.Editor(edit.EditorCfg{
							Buffer:          st.Buf,
							ID:              "mdproto-editor",
							ShowLineNumbers: true,
							Height:          editorH,
							Width:           editorW,
						}),
					},
				}),
			},
		},
		Second: gui.SplitterPaneCfg{
			MinSize: 200,
			Content: []gui.View{
				// 右：Markdown 预览（超过阈值则降级为纯文本摘要）
				gui.Column(gui.ContainerCfg{
					Sizing:     gui.FillFill,
					Padding:    theme.PaddingMedium,
					Focusable:  true,
					ID:         "md-scroll",
					Scrollable: true,
					AmendLayout: func(ctx gui.EventCtx) {
						if !st.firstView {
							st.firstView = true
							fmt.Fprintf(os.Stderr, "[proto] 首次布局渲染: %.0f ms (自程序启动)\n",
								time.Since(procStart).Milliseconds())
						}
					},
					Content: []gui.View{previewPane(w, st)},
				}),
			},
		},
	})
}

// previewPane 返回右侧预览内容。文档超过 previewMaxBytes 时降级为纯文本摘要，
// 避免 go-gui 对超大文档全量构建 Markdown view 的二次方级崩溃（见 P1）。
func previewPane(w *gui.Window, st *app) gui.View {
	sz := st.Buf.Len()
	if sz > previewMaxBytes {
		// 降级：提示 + 前若干行纯文本（用 Text 控件，避免 Markdown 树）。
		const headLines = 60
		n := st.Buf.LineCount()
		if n > headLines {
			n = headLines
		}
		var sb strings.Builder
		fmt.Fprintf(&sb, "⚠ 文件过大（%.1f MiB），完整预览已停用（阈值 2 MiB）。\n\n前 %d 行原文：\n\n",
			float64(sz)/1024/1024, n)
		for i := 0; i < n; i++ {
			sb.Write(st.Buf.Line(i))
			sb.WriteByte('\n')
		}
		return gui.Text(gui.TextCfg{
			Text:      sb.String(),
			TextStyle: gui.CurrentTheme().TextStyleBodySmall,
			Mode:      gui.TextModeMultiline, // 保留换行
		})
	}
	return w.Markdown(gui.MarkdownCfg{
		Source:              st.Buf.String(),
		Style:               gui.DefaultMarkdownStyle(),
		DisableExternalAPIs: true,
		Mode:                gui.Some(gui.TextModeWrap),
	})
}