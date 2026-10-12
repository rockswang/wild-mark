// Command p1_mdparse: 无头测量 go-gui markdown 解析器的规模成本（纯 parse，不含 view/RTF 布局）。
// 用法: p1_mdparse <file...>   打印 parse 耗时、返回 block 数
package main

import (
	"fmt"
	"os"
	"runtime"
	"time"

	"github.com/go-gui-org/go-gui/gui/markdown"
)

func memMB() int64 {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return int64(m.HeapAlloc >> 20)
}

func run(path string) {
	info, _ := os.Stat(path)
	raw, err := os.ReadFile(path)
	if err != nil {
		fmt.Printf("%-40s read err: %v\n", path, err)
		return
	}
	base := memMB()
	t0 := time.Now()
	blocks := markdown.Parse(string(raw), false)
	el := time.Since(t0)
	after := memMB()
	fmt.Printf("%-42s size=%7.1fMiB  parse=%8s  blocks=%8d  heapΔ=%6dMB\n",
		path, float64(info.Size())/1024/1024, el.Round(time.Millisecond), len(blocks), after-base)
}

func main() {
	for _, p := range os.Args[1:] {
		run(p)
	}
}