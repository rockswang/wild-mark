// Command cpumon: 采样目标进程的 CPU%(相对单核) 和内存，输出每间隔一行的数值，结束时输出统计。
// 启动命令行: cpumon -pid <pid> -dur 60s -int 500ms
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"syscall"
	"time"
	"unsafe"
)

var (
	kernel32     = syscall.NewLazyDLL("kernel32.dll")
	procOpenPs   = kernel32.NewProc("OpenProcess")
	procGetTimes = kernel32.NewProc("GetProcessTimes")
	procSysTimes = kernel32.NewProc("GetSystemTimes")
	procClose    = kernel32.NewProc("CloseHandle")
)

const procQueryInfo = 0x0400

func filetimeSec(ft syscall.Filetime) float64 {
	return float64(uint64(ft.HighDateTime)<<32|uint64(ft.LowDateTime)) / 1e7
}

func procTime(pid int) float64 {
	h, _, _ := procOpenPs.Call(uintptr(procQueryInfo), 0, uintptr(pid))
	if h == 0 {
		return -1
	}
	defer procClose.Call(h)
	var creation, exit, kernel, user syscall.Filetime
	r, _, _ := procGetTimes.Call(h,
		uintptr(unsafe.Pointer(&creation)), uintptr(unsafe.Pointer(&exit)),
		uintptr(unsafe.Pointer(&kernel)), uintptr(unsafe.Pointer(&user)))
	if r == 0 {
		return -1
	}
	return filetimeSec(kernel) + filetimeSec(user)
}

func systemTime() (idle, total float64) {
	var idleT, kernelT, userT syscall.Filetime
	r, _, _ := procSysTimes.Call(uintptr(unsafe.Pointer(&idleT)), uintptr(unsafe.Pointer(&kernelT)), uintptr(unsafe.Pointer(&userT)))
	if r == 0 {
		return 0, 0
	}
	idle = filetimeSec(idleT)
	total = filetimeSec(kernelT) + filetimeSec(userT)
	return idle, total
}

func memKB(pid int) float64 {
	cmd := exec.Command("tasklist", "/fo", "csv", "/nh", "/fi", "PID eq "+strconv.Itoa(pid))
	out, err := cmd.Output()
	if err != nil {
		return 0
	}
	line := strings.TrimSpace(string(out))
	if !strings.HasPrefix(line, `"`) {
		return 0
	}
	parts := strings.Split(strings.Trim(line, `"`), `","`)
	if len(parts) >= 5 {
		// parts[4] 形如 "  4,732 K "
		wss := parts[4]
		wss = strings.ReplaceAll(wss, ",", "")
		fields := strings.Fields(wss)
		if len(fields) >= 1 {
			v, err := strconv.ParseFloat(fields[0], 64)
			if err == nil {
				return v
			}
		}
	}
	return 0
}

func main() {
	pid := flag.Int("pid", 0, "target pid")
	dur := flag.Duration("dur", 60*time.Second, "sample duration")
	interval := flag.Duration("int", 500*time.Millisecond, "sample interval")
	ncpu := flag.Int("ncpu", 1, "number of logical CPUs for normalization (relative to 1 core)")
	flag.Parse()
	if *pid == 0 {
		fmt.Fprintln(os.Stderr, "need -pid")
		os.Exit(2)
	}

	t0 := time.Now()
	sysIdle, sysTotal := systemTime()
	prevProc := procTime(*pid)
	var sumPct float64 // sum of per-interval percent (relative to 1 core) across intervals that had IPC>0
	var n int
	var maxPct float64
	var maxMem, sumMem float64
	var nMem int

	measurePeriod := func(wall time.Duration, curSysIdle, curSysTotal, curProc float64) (pct float64) {
		if wall <= 0 {
			return 0
		}
		// 进程 CPU 在 wall 窗口内的核心时间增量（秒）除以墙钟（秒）=> 占用单个逻辑核心的比例。
		dProc := curProc - prevProc // 进程累计 core-seconds
		if dProc < 0 {
			dProc = 0
		}
		// 单核百分比：进程在墙钟窗口内占用的 core-seconds 除以墙钟秒数。
		pct = dProc / wall.Seconds() * 100.0
		return pct
	}

	for time.Since(t0) < *dur {
		time.Sleep(*interval)
		curSysIdle, curSysTotal := systemTime()
		curProc := procTime(*pid)
		if curProc < 0 {
			break // process died
		}
		periodPct := measurePeriod(*interval, curSysIdle, curSysTotal, curProc)
		m := memKB(*pid)
		_ = sysIdle
		_ = sysTotal
		fmt.Printf("%s  per-core=%.3f%%  total=%.3f%%  mem=%.1fMB\n", time.Now().Format("15:04:05.000"), periodPct, periodPct/float64(*ncpu), m/1024)
		sumPct += periodPct
		n++
		if periodPct > maxPct {
			maxPct = periodPct
		}
		if m > maxMem {
			maxMem = m
		}
		sumMem += m
		nMem++

		prevProc = curProc
	}

	if n > 0 {
		fmt.Printf("=== SUMMARY over %d samples ===\n", n)
		fmt.Printf("avg per-core=%+.3f%%  avg total=%.3f%%  max per-core=%.3f%%  max total=%.3f%% (ncpu=%d)\n",
			sumPct/float64(n), sumPct/float64(n)/float64(*ncpu), maxPct, maxPct/float64(*ncpu), *ncpu)
		if nMem > 0 {
			fmt.Printf("avg mem=%.1fMB max mem=%.1fMB\n", sumMem/float64(nMem)/1024, maxMem/1024)
		}
	}
}