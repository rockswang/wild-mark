package main

// P0-probe: 每 5s 打印 Go 堆统计(HeapInuse, HeapSys, HeapAlloc) 与进程工作集。
import (
	"os"
	"runtime"
	"time"

	"github.com/go-gui-org/go-gui/gui"
	"github.com/go-gui-org/go-gui/gui/backend"
)

func main() {
	w := gui.SimpleWindow("P0 Probe", 800, 600, nil, func(w *gui.Window) {
		w.SetView(func(w *gui.Window) gui.View {
			return gui.Column(gui.ContainerCfg{
				Sizing: gui.FillFill,
				Content: []gui.View{
					gui.Label("P0 memory probe", gui.TextStyle{}),
				},
			})
		})
	})
	go func() {
		var m runtime.MemStats
		for {
			time.Sleep(5 * time.Second)
			runtime.ReadMemStats(&m)
			// 间隔中强制 GC 一次观察堆回落
			println("heap: inuse=", m.HeapInuse/1024/1024, "MB sys=", m.HeapSys/1024/1024, "MB alloc=", m.HeapAlloc/1024/1024,
				"MB numGC=", m.NumGC, "  sysTotal=", m.Sys/1024/1024, "MB")
			os.Stdout.Sync()
		}
	}()
	backend.Run(w)
}