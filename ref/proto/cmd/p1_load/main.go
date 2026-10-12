// Command p1_load: 无头验证 go-edit buffer 加载性能与 32 MiB 限制。
// 默认用 buffer.Load（带 32 MiB 上限）；-raw 用 os.ReadFile + buffer.FromBytes 绕过上限。
// 用法: p1_load [-raw] <file...>
package main

import (
	"errors"
	"fmt"
	"os"
	"runtime"
	"time"

	"github.com/go-gui-org/go-edit/edit/buffer"
)

var rawMode bool

func memMB() float64 {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return float64(m.HeapAlloc) / 1024 / 1024
}

func run(path string) {
	info, err := os.Stat(path)
	if err != nil {
		fmt.Printf("%-46s stat err: %v\n", path, err)
		return
	}
	baseHeap := memMB()
	t0 := time.Now()
	var lines int
	var loadErr error
	if rawMode {
		raw, e := os.ReadFile(path)
		if e != nil {
			loadErr = e
		} else {
			buf := buffer.FromBytes(raw)
			lines = buf.LineCount()
		}
	} else {
		f, e := os.Open(path)
		if e != nil {
			loadErr = e
		} else {
			buf, le := buffer.Load(f)
			f.Close()
			if buf != nil {
				lines = buf.LineCount()
			}
			loadErr = le
		}
	}
	el := time.Since(t0)
	afterHeap := memMB()
	tag := "Load      "
	if rawMode {
		tag = "FromBytes "
	}
	const name = 46
	if loadErr != nil {
		fmt.Printf("%-*s size=%8.1fMiB  %s= ERROR: %v\n", name, path, float64(info.Size())/1024/1024, tag, loadErr)
		return
	}
	fmt.Printf("%-*s size=%8.1fMiB  %s=%8s  lines=%8d  heapΔ=%7.1fMB\n",
		name, path, float64(info.Size())/1024/1024, tag, el.Round(time.Millisecond), lines, afterHeap-baseHeap)
	_ = errors.Is
}

func main() {
	args := os.Args[1:]
	if len(args) > 0 && args[0] == "-raw" {
		rawMode = true
		args = args[1:]
	}
	if len(args) == 0 {
		fmt.Println("usage: p1_load [-raw] <file...>")
		return
	}
	for _, p := range args {
		run(p)
	}
}