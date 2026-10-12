package main

// P0: 空白窗口空闲资源基线。仅开一个窗口，什么都不做，用于测量：
//   - 启动时间 / 内存 / CPU 基线（M0）
//   - 静止 60s、最小化 60s 的空闲 CPU 与唤醒行为
// 运行:  go run ./cmd/p0_blank

import (
	"github.com/go-gui-org/go-gui/gui"
	"github.com/go-gui-org/go-gui/gui/backend"
)

func main() {
	w := gui.SimpleWindow("P0 Blank Idle", 800, 600, nil, func(w *gui.Window) {
		w.SetView(func(w *gui.Window) gui.View {
			return gui.Column(gui.ContainerCfg{
				Sizing: gui.FillFill,
				Content: []gui.View{
					gui.Label("P0: 空白窗口 — 用于空闲 CPU 基线", gui.TextStyle{}),
				},
			})
		})
	})
	backend.Run(w)
}