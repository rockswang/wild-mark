package main

// P0-framecount: 每 2 秒打印一次 FrameCount，用于判断空闲时是否有连续帧产生。
import (
	"time"

	"github.com/go-gui-org/go-gui/gui"
	"github.com/go-gui-org/go-gui/gui/backend"
)

func main() {
	w := gui.SimpleWindow("P0 FrameCount", 800, 600, nil, func(w *gui.Window) {
		w.SetView(func(w *gui.Window) gui.View {
			return gui.Column(gui.ContainerCfg{
				Sizing: gui.FillFill,
				Content: []gui.View{
					gui.Label("P0 frame-count probe", gui.TextStyle{}),
				},
			})
		})
	})
	// 后台 goroutine 每 2s 打印帧数增量
	go func() {
		prev := uint64(0)
		for {
			time.Sleep(2 * time.Second)
			now := w.FrameCount()
			delta := now - prev
			prev = now
			println("frames in last 2s:", int(delta))
		}
	}()
	backend.Run(w)
}