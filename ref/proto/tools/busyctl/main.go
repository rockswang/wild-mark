package main

// busyctl: 可控忙等，用于校准 cpumon。占一个核指定比例。
import (
	"os"
	"strconv"
	"time"
)

func main() {
	pct := 10.0
	if len(os.Args) > 1 {
		if v, err := strconv.ParseFloat(os.Args[1], 64); err == nil {
			pct = v
		}
	}
	dur := 30 * time.Second
	t0 := time.Now()
	burn := time.Duration(int64(float64(time.Second) * pct / 100.0)) // burn pct of each 1s
	for time.Since(t0) < dur {
		ws := time.Now()
		// spin for 'burn'
		for time.Since(ws) < burn {
		}
		// sleep rest
		time.Sleep(time.Second - burn)
	}
	os.Exit(0)
}