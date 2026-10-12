package main

// P0-pprof: 保留 pprof 端点，抓取空闲时的 CPU 归属。
import (
	"net/http"
	_ "net/http/pprof"

	"github.com/go-gui-org/go-gui/gui"
	"github.com/go-gui-org/go-gui/gui/backend"
)

func main() {
	go func() { http.ListenAndServe(":6060", nil) }()
	w := gui.SimpleWindow("P0 Pprof", 800, 600, nil, func(w *gui.Window) {
		w.SetView(func(w *gui.Window) gui.View {
			return gui.Column(gui.ContainerCfg{
				Sizing: gui.FillFill,
				Content: []gui.View{
					gui.Label("P0 pprof probe", gui.TextStyle{}),
				},
			})
		})
	})
	backend.Run(w)
}