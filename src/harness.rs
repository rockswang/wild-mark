//! 原型公共外壳：参数解析、全局记录器、场景时间线、指标汇总。
//! 三个 bin（p0/p1/p2）共用「自驱动场景 + 进程内自采样」，避免外部脚本抖动影响读数。
//! 记录器走全局 OnceLock<RefCell>：wndproc 与主流程都要打点，借用冲突最少。

use std::sync::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging as wm;

use crate::metrics::{self, Recorder, Snap};
use crate::win as w;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Idle, // 前台静止
    Back, // 最小化静止
    Drag, // 连续改尺寸
    Wait, // 隐藏/未交互等待
    Task, // 由 bin 注入的自定义动作（通过回调）
}

#[derive(Clone)]
pub struct Phase {
    pub kind: Kind,
    pub secs: f64,
    pub name: &'static str,
}

pub fn phase(name: &'static str, kind: Kind, secs: f64) -> Phase {
    Phase { kind, secs, name }
}

static REC: OnceLock<Mutex<Recorder>> = OnceLock::new();

pub fn init_rec(path: &str) {
    let _ = REC.set(Mutex::new(Recorder::new(path)));
}

pub fn rec() -> &'static Mutex<Recorder> {
    REC.get().expect("Recorder 未初始化")
}

/// 打一行 JSONL
pub fn sample(tag: &str, extra: &[(&str, String)]) -> Snap {
    rec().lock().unwrap().log(tag, extra)
}

/// 里程碑采样：额外带模块数/线程数（成本高，只在阶段点调用）
pub fn sample_deep(tag: &str, extra: &[(&str, String)]) -> Snap {
    rec().lock().unwrap().log_deep(tag, extra)
}

pub fn flush() {
    rec().lock().unwrap().flush();
}

pub fn ms() -> f64 {
    rec().lock().unwrap().ms()
}

// ---------- 参数 ----------

/// --key value / --flag
pub fn args() -> HashMap<String, String> {
    let mut m = HashMap::new();
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        if let Some(k) = v[i].strip_prefix("--") {
            if i + 1 < v.len() && !v[i + 1].starts_with("--") {
                m.insert(k.to_string(), v[i + 1].clone());
                i += 2;
            } else {
                m.insert(k.to_string(), "1".to_string());
                i += 1;
            }
        } else {
            m.insert(format!("pos{}", m.len()), v[i].clone());
            i += 1;
        }
    }
    m
}

pub fn num(m: &HashMap<String, String>, k: &str, def: f64) -> f64 {
    m.get(k).and_then(|v| v.parse().ok()).unwrap_or(def)
}
pub fn strv(m: &HashMap<String, String>, k: &str, def: &str) -> String {
    m.get(k).cloned().unwrap_or(def.to_string())
}
/// 路径型参数：`--shot` 不带值时（值被置为 "1"）回落到默认路径
pub fn pathv(m: &HashMap<String, String>, k: &str, def: &str) -> String {
    match m.get(k) {
        Some(v) if v != "1" && !v.is_empty() => v.clone(),
        _ => def.to_string(),
    }
}
pub fn flag(m: &HashMap<String, String>, k: &str) -> bool {
    m.get(k).is_some()
}
/// --fast=秒级缩时；--quick=1/5；正式验收用真实时长
pub fn scale(m: &HashMap<String, String>) -> f64 {
    if flag(m, "fast") {
        0.02
    } else if flag(m, "quick") {
        0.2
    } else {
        1.0
    }
}
pub fn pos(m: &HashMap<String, String>, i: usize) -> Option<String> {
    m.get(&format!("pos{i}")).cloned()
}

pub unsafe fn def(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    wm::DefWindowProcW(hwnd, msg, wp, lp)
}

// ---------- 场景 ----------

pub unsafe fn sleep_pump(ms: u32) -> bool {
    let _ = wm::MsgWaitForMultipleObjects(None, false, ms, wm::QS_ALLINPUT);
    let mut m = wm::MSG::default();
    while wm::PeekMessageW(&mut m, None, 0, 0, wm::PM_REMOVE).as_bool() {
        if m.message == wm::WM_QUIT {
            return false;
        }
        let _ = wm::TranslateMessage(&m);
        wm::DispatchMessageW(&m);
    }
    true
}


/// 静止 secs 秒并按 step 采样
/// 静止 secs 秒并按 step 采样；首个采样点延后一个 interval，避免把上一阶段的 CPU 计入本段
pub fn quiet(secs: f64, step: f64) {
    let end = ms() + secs * 1000.0;
    let mut next = ms() + step * 1000.0;
    unsafe {
        while ms() < end {
            if !sleep_pump(50) {
                break;
            }
            if ms() >= next {
                sample("idle", &[]);
                next = ms() + step * 1000.0;
            }
        }
    }
}

pub fn minimize(hwnd: HWND, secs: f64) {
    unsafe {
        let _ = wm::ShowWindow(hwnd, wm::SW_MINIMIZE);
    }
    quiet(secs, 2.0);
    unsafe {
        let _ = wm::ShowWindow(hwnd, wm::SW_RESTORE);
        let _ = wm::SetForegroundWindow(hwnd);
    }
}

/// 连续改尺寸，模拟拖拽边框
pub fn drag(hwnd: HWND, secs: f64, step_ms: u32) {
    let end = ms() + secs * 1000.0;
    let mut dir = 1i32;
    unsafe {
        let mut rw = w::client_rect(hwnd);
        rw.right = rw.right.max(700);
        rw.bottom = rw.bottom.max(480);
        let _ = wm::AdjustWindowRect(&mut rw, wm::WS_OVERLAPPEDWINDOW, true);
        let (mut fw, mut fh) = (rw.right - rw.left, rw.bottom - rw.top);
        let mut next = ms() + step_ms as f64;
        while ms() < end {
            if ms() >= next {
                let j = dir * 23;
                fw = (fw + j).clamp(560, 1560);
                fh = (fh + j / 2).clamp(380, 1000);
                let _ = wm::SetWindowPos(hwnd, HWND::default(), 0, 0, fw, fh, wm::SWP_NOMOVE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE);
                sample("drag", &[("size", format!("{fw}x{fh}"))]);
                if fw >= 1560 || fw <= 560 {
                    dir = -dir;
                }
                next = ms() + step_ms as f64;
            }
            if !sleep_pump(5) {
                break;
            }
        }
    }
}


/// 时间线；Task 阶段调用回调（bin 在里面做载入/键入等动作）
pub fn run_phases(hwnd: HWND, ps: &[Phase], mut on_task: impl FnMut(&str)) {
    for p in ps {
        sample("phase", &[("name", p.name.to_string()), ("kind", format!("{:?}", p.kind)), ("secs", format!("{:.1}", p.secs))]);
        match p.kind {
            Kind::Idle | Kind::Wait => quiet(p.secs, 2.0),
            Kind::Back => minimize(hwnd, p.secs),
            Kind::Drag => drag(hwnd, p.secs, 60),
            Kind::Task => on_task(p.name),
        }
    }
}

// ---------- 汇总 ----------

/// 扁平 JSON 行解析（原型只用 string/number 字段，不引第三方库）
pub fn json_line(line: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let body = line.trim().trim_start_matches('{').trim_end_matches('}');
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in body.chars() {
        match c {
            '"' => {
                depth += if cur.is_empty() { 1 } else { -1 };
                cur.push(c)
            }
            ',' if depth <= 0 => {
                push_kv(&mut m, &cur);
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        push_kv(&mut m, &cur);
    }
    m
}

fn push_kv(m: &mut HashMap<String, String>, s: &str) {
    if let Some((k, v)) = s.split_once(':') {
        m.insert(k.trim().trim_matches('"').to_string(), v.trim().trim_matches('"').to_string());
    }
}

pub fn f(m: &HashMap<String, String>, k: &str) -> f64 {
    m.get(k).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}
pub fn u(m: &HashMap<String, String>, k: &str) -> u64 {
    m.get(k).and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// 按 tag 聚合：空闲 CPU 均值/峰值、内存峰值、模块数、GDI、线程、句柄
pub fn summarize(jsonl: &str, exe: &str) -> HashMap<String, String> {
    let txt = std::fs::read_to_string(jsonl).unwrap_or_default();
    let mut res = HashMap::new();
    let (mut idle_n, mut idle_cpu, mut cpu_max) = (0usize, 0f64, 0f64);
    let (mut ws, mut pws, mut prv, mut mods, mut gdi, mut thr, mut hnd, mut t_ms, mut cpu_ms) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for line in txt.lines() {
        let m = json_line(line);
        if m.get("tag").map(|s| s.as_str()) == Some("idle") {
            idle_n += 1;
            idle_cpu += f(&m, "cpu_pct");
        }
        cpu_max = cpu_max.max(f(&m, "cpu_pct"));
        t_ms = t_ms.max(f(&m, "t_ms") as u64);
        cpu_ms = cpu_ms.max(f(&m, "cpu_ms") as u64);
        ws = ws.max(u(&m, "ws_kb") * 1024);
        pws = pws.max(u(&m, "pws_kb") * 1024);
        prv = prv.max(u(&m, "priv_kb") * 1024);
        mods = mods.max(u(&m, "mods"));
        gdi = gdi.max(u(&m, "gdi"));
        thr = thr.max(u(&m, "threads"));
        hnd = hnd.max(u(&m, "handles"));
    }
    let mb = |b: u64| format!("{:.2}", b as f64 / 1048576.0);
    res.insert("idle_samples".into(), idle_n.to_string());
    res.insert("idle_cpu_pct".into(), format!("{:.3}", if idle_n > 0 { idle_cpu / idle_n as f64 } else { 0.0 }));
    res.insert("cpu_pct_max".into(), format!("{:.3}", cpu_max));
    res.insert("span_ms".into(), t_ms.to_string());
    res.insert("cpu_total_ms".into(), cpu_ms.to_string());
    res.insert("ws_peak_mb".into(), mb(ws));
    res.insert("pws_peak_mb".into(), mb(pws));
    res.insert("priv_peak_mb".into(), mb(prv));
    res.insert("mods".into(), mods.to_string());
    res.insert("gdi".into(), gdi.to_string());
    res.insert("threads".into(), thr.to_string());
    res.insert("handles".into(), hnd.to_string());
    if let Ok(md) = std::fs::metadata(exe) {
        res.insert("exe_mb".into(), mb(md.len()));
    }
    res
}

pub fn write_json(m: &HashMap<String, String>, path: &str) {
    let mut kv: Vec<_> = m.iter().collect();
    kv.sort();
    let body = kv.iter().map(|(k, v)| format!("\"{k}\":\"{v}\"")).collect::<Vec<_>>().join(",\n  ");
    std::fs::write(path, format!("{{\n  {body}\n}}\n")).ok();
}

pub fn print_map(m: &HashMap<String, String>) {
    let mut kv: Vec<_> = m.iter().collect();
    kv.sort();
    for (k, v) in kv {
        println!("{k}={v}");
    }
}

pub fn say(tag: &str, s: &Snap) {
    println!("{}", metrics::line(tag, s, ms()));
}

/// 阶段标签，用于 bin 注入
pub const WM_APP_TASK: u32 = wm::WM_APP + 11;

/// 供 bin 复用：把 exe 同目录拼成绝对路径
pub fn exe_rel(rel: &str) -> String {
    let dir = std::path::PathBuf::from(std::env::current_exe().unwrap_or_default()).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    dir.join(rel).to_string_lossy().replace('\\', "/")
}

pub fn cwd_rel(rel: &str) -> String {
    std::path::Path::new(rel).to_string_lossy().replace('\\', "/")
}

/// 原型里所有 DLL 都放在 native/*/bin 下；相对 exe 或 cwd 两种路径都试一次
pub fn find_dll(rel: &str) -> Option<String> {
    for base in [String::from("."), root(), exe_rel("")] {
        let p = format!("{base}/{rel}").replace("//", "/");
        if std::path::Path::new(&p).exists() {
            return Some(std::path::Path::new(&p).canonicalize().map(|c| c.to_string_lossy().replace('\\', "/")).unwrap_or(p));
        }
    }
    None
}

/// 工程根：从 current_exe 往上找含 Cargo.toml 与 src/ 的目录；
/// 保证计划任务等异工作目录下产物落盘位置一致
pub fn root() -> String {
    static ROOT: OnceLock<String> = OnceLock::new();
    ROOT.get_or_init(|| {
        let mut d = std::path::PathBuf::from(std::env::current_exe().unwrap_or_default());
        while d.pop() {
            if d.join("Cargo.toml").exists() && d.join("src").is_dir() {
                return d.to_string_lossy().replace('\\', "/");
            }
        }
        std::env::current_dir().map(|c| c.to_string_lossy().replace('\\', "/")).unwrap_or_default()
    })
    .clone()
}

/// 相对路径→工程根下的绝对路径（已是绝对路返原值）
pub fn abs(p: &str) -> String {
    let ph = std::path::Path::new(p);
    if ph.is_absolute() || p.is_empty() {
        return p.to_string();
    }
    format!("{}/{p}", root()).replace("//", "/")
}
