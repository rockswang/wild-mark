"""在用户交互桌面(session 1)里运行原型 exe，并收集 stdout/产物。

本 agent 的 bash 处于 session 0（非交互窗口站）：在那里 CreateWindowEx 出来的窗口不会真正绘制，
测到的内存/CPU/GDI 都不是真实值（实测空闲工作集 6MiB vs 交互会话 42MiB）。
因此所有 GUI 测量一律经 schtasks 投递到交互会话执行，跑完由 marker 文件判定结束。

用法: python scripts/run_task.py [--wait 900] <exe 相对/绝对> [程序参数...]
"""
import os, subprocess, sys, time, uuid

PROJ = os.path.dirname(os.path.dirname(os.path.abspath(__file__))).replace("/", "\\")
SCH = r"C:\Windows\System32\schtasks.exe"


def sh(args):
    r = subprocess.run(args, capture_output=True)
    dec = lambda b: (b or b"").decode("gbk", errors="replace")
    return type("R", (), {"returncode": r.returncode, "stdout": dec(r.stdout), "stderr": dec(r.stderr)})


def run(exe, pargs, wait=900.0):
    exe_abs = exe if os.path.isabs(exe) else os.path.join(PROJ, exe.replace("/", "\\"))
    tag = uuid.uuid4().hex[:8]
    task = f"wrp_{tag}"
    bat = os.path.join(PROJ, f"temp_task_{tag}.bat")
    log = os.path.join(PROJ, "artifacts", f"task_{tag}.out")
    marker = os.path.join(PROJ, "artifacts", f"task_{tag}.done")
    cmd = f'"{exe_abs}" ' + " ".join(pargs)
    with open(bat, "wb") as f:  # bat 必须 CRLF+ASCII 路径，避免 cmd 解析坑
        body = f"@echo off\r\ncd /d \"{PROJ}\"\r\n{cmd} > \"{log}\" 2>&1\r\necho done>\"{marker}\"\r\n"
        f.write(body.encode("gbk", errors="replace"))
    r = sh([SCH, "/create", "/tn", task, "/tr", bat, "/sc", "once", "/st", "23:59", "/f"])
    if r.returncode != 0:
        print("schtasks create 失败:", r.stdout, r.stderr, file=sys.stderr)
        return None
    sh([SCH, "/run", "/tn", task])
    t0, ok = time.time(), False
    while time.time() - t0 < wait:
        if os.path.exists(marker):
            ok = True
            time.sleep(0.5)
            break
        time.sleep(0.5)
    sh([SCH, "/delete", "/tn", task, "/f"])
    for p in (bat, marker):
        try:
            os.remove(p)
        except OSError:
            pass
    txt = open(log, encoding="utf-8", errors="replace").read() if os.path.exists(log) else ""
    if not ok:
        print(f"TIMEOUT {wait}s: {task}", file=sys.stderr)
    return {"task": task, "log": log, "elapsed": time.time() - t0, "stdout": txt, "ok": ok}


if __name__ == "__main__":
    a = sys.argv[1:]
    wait = 900.0
    if a[:1] == ["--wait"]:
        wait = float(a[1])
        a = a[2:]
    res = run(a[0], a[1:], wait)
    if res is None:
        sys.exit(3)
    print(res["stdout"])
    print(f"# log={res['log']} elapsed={res['elapsed']:.1f}s ok={res['ok']}", file=sys.stderr)
    sys.exit(0 if res["ok"] else 4)
