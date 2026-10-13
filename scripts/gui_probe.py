"""检查原型进程是否真的出现在交互桌面上：窗口可见性 + 可前台化 + 截图可行性。

用法: python scripts/gui_probe.py <exe> [args...]
"""
import ctypes, ctypes.wintypes as wt, subprocess, sys, time, os

u = ctypes.windll.user32
k = ctypes.windll.kernel32


class RECT(ctypes.Structure):
    _fields_ = [("l", wt.LONG), ("t", wt.LONG), ("r", wt.LONG), ("b", wt.LONG)]


def enum_windows(pid):
    out = []
    cb = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)

    def _cb(h, _l):
        p = wt.DWORD()
        u.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid and u.IsWindowVisible(h):
            n = u.GetWindowTextLengthW(h)
            buf = ctypes.create_unicode_buffer(n + 1)
            u.GetWindowTextW(h, buf, n + 1)
            r = RECT()
            u.GetWindowRect(h, ctypes.byref(r))
            out.append((h, buf.value, (r.l, r.t, r.r, r.b)))
        return True

    u.EnumWindows(cb(_cb), 0)
    return out


def main():
    exe = sys.argv[1]
    extra = sys.argv[2:]
    t0 = time.perf_counter()
    p = subprocess.Popen([exe] + extra, cwd=os.path.dirname(exe) or ".")
    visible = None
    for _ in range(120):
        time.sleep(0.05)
        ws = enum_windows(p.pid)
        if ws:
            visible = (time.perf_counter() - t0, ws)
            break
    if not visible:
        print(f"NO_WINDOW pid={p.pid} (进程在交互桌面看不到窗口；会话可能非交互)")
        p.kill()
        return 2
    dt, ws = visible
    h, title, rect = ws[0]
    print(f"window_found_after_ms={dt*1000:.0f} hwnd={h} title={title!r} rect={rect}")
    print(f"foreground_ok={bool(u.SetForegroundWindow(h))} cur_fore={u.GetForegroundWindow()}")
    print(f"is_iconic={bool(u.IsIconic(h))} dpi={u.GetDpiForWindow(h)} thread={u.GetWindowThreadProcessId(h, None)}")
    p.kill()
    return 0


if __name__ == "__main__":
    sys.exit(main())
