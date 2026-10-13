//! 进程内自采样：工作集 / 私有工作集 / 私有提交 / CPU 时间 / 模块数 / GDI 对象数，输出 JSONL。
//! 区分 WorkingSet（含共享系统 DLL 页）与 PrivateWorkingSet，对应 plan.md 的内存口径要求。

use std::io::Write;
use windows::Win32::Foundation::{FILETIME, HMODULE};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModulesEx, GetModuleInformation, K32GetProcessMemoryInfo, MODULEINFO, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    PROCESS_MEMORY_COUNTERS_EX2, LIST_MODULES_ALL,
};
use windows::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;
use windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GetProcessHandleCount, GetProcessTimes, GR_GDIOBJECTS, GR_USEROBJECTS};

/// QPC 时基；进程内所有耗时打点用它
pub struct Clock {
    base: i64,
    freq: i64,
}

impl Clock {
    pub fn new() -> Self {
        let (mut f, mut b) = (0i64, 0i64);
        unsafe {
            let _ = QueryPerformanceFrequency(&mut f as *mut _);
            let _ = QueryPerformanceCounter(&mut b as *mut _);
        }
        Self { base: b, freq: f.max(1) }
    }
    pub fn ms(&self) -> f64 {
        let mut v = 0i64;
        unsafe {
            let _ = QueryPerformanceCounter(&mut v as *mut _);
        }
        (v - self.base) as f64 * 1000.0 / self.freq as f64
    }
}

/// 进程创建 → 当前时刻 的墙钟毫秒；用于「启动至可交互」
pub fn process_age_ms() -> f64 {
    unsafe {
        let (mut ct, mut et, mut kt, mut ut) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        if GetProcessTimes(GetCurrentProcess(), &mut ct, &mut et, &mut kt, &mut ut).is_err() {
            return 0.0;
        }
        let now = GetSystemTimePreciseAsFileTime();
        let c = ((ct.dwHighDateTime as u64) << 32) | ct.dwLowDateTime as u64;
        let n = ((now.dwHighDateTime as u64) << 32) | now.dwLowDateTime as u64;
        n.saturating_sub(c) as f64 / 1e4
    }
}

#[derive(Default, Clone, Copy)]
pub struct Snap {
    pub ws: u64,         // WorkingSetSize：含共享系统 DLL 映射页
    pub pws: u64,        // PrivateWorkingSetSize：真正私有工作集（内存验收主口径）
    pub priv_bytes: u64, // PrivateUsage：私有提交内存
    pub peak_ws: u64,
    pub faults: u64,
    pub cpu_100ns: u64, // user+kernel 累计 CPU 时间
    pub gdi: u64,
    pub usr: u64,
    pub handles: u64,
    pub threads: u64,
    pub mods: u64,
    pub mod_mb: u64, // 已加载模块映像大小合计（MB），用于区分"系统 DLL 占工作集"
}

fn thread_count(pid: u32) -> u64 {
    unsafe {
        let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(h) => h,
            Err(_) => return 0,
        };
        let mut te = THREADENTRY32 { dwSize: std::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
        let mut n = 0u64;
        if Thread32First(snap, &mut te).is_ok() {
            loop {
                if te.th32OwnerProcessID == pid {
                    n += 1;
                }
                if Thread32Next(snap, &mut te).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        n
    }
}

/// 轻量快照：只调廉价 API，供高频周期采样。
/// 模块枚举与线程快照很贵，放进去会把空闲 CPU 和缺页数据本身打爆（已踩到）。
pub fn snap() -> Snap {
    let h = unsafe { GetCurrentProcess() };
    let mut s = Snap::default();
    unsafe {
        let mut c = PROCESS_MEMORY_COUNTERS_EX2 { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32, ..Default::default() };
        if K32GetProcessMemoryInfo(h, &mut c as *mut PROCESS_MEMORY_COUNTERS_EX2 as *mut PROCESS_MEMORY_COUNTERS, c.cb).as_bool() {
            s.ws = c.WorkingSetSize as u64;
            s.pws = c.PrivateWorkingSetSize as u64;
            s.priv_bytes = c.PrivateUsage as u64;
            s.peak_ws = c.PeakWorkingSetSize as u64;
            s.faults = c.PageFaultCount as u64;
        } else {
            // 老系统退化为 EX（无私有工作集），仍可拿到 priv_bytes
            let mut e = PROCESS_MEMORY_COUNTERS_EX { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
            if K32GetProcessMemoryInfo(h, &mut e as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS, e.cb).as_bool() {
                s.ws = e.WorkingSetSize as u64;
                s.priv_bytes = e.PrivateUsage as u64;
                s.peak_ws = e.PeakWorkingSetSize as u64;
                s.faults = e.PageFaultCount as u64;
            }
        }
        let (mut ct, mut et, mut kt, mut ut) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        if GetProcessTimes(h, &mut ct, &mut et, &mut kt, &mut ut).is_ok() {
            s.cpu_100ns = ((kt.dwHighDateTime as u64) << 32 | kt.dwLowDateTime as u64) + ((ut.dwHighDateTime as u64) << 32 | ut.dwLowDateTime as u64);
        }
        let mut hc = 0u32;
        if GetProcessHandleCount(h, &mut hc).is_ok() {
            s.handles = hc as u64;
        }
        s.gdi = GetGuiResources(h, GR_GDIOBJECTS) as u64;
        s.usr = GetGuiResources(h, GR_USEROBJECTS) as u64;
    }
    s
}

/// 重量快照：额外枚举模块与线程，只在里程碑点调用（启动/控件创建/退出前）
pub fn snap_deep() -> Snap {
    let mut s = snap();
    let h = unsafe { GetCurrentProcess() };
    unsafe {
        let mut mods = [HMODULE::default(); 1024];
        let mut need = 0u32;
        if EnumProcessModulesEx(h, mods.as_mut_ptr(), (mods.len() * std::mem::size_of::<HMODULE>()) as u32, &mut need, LIST_MODULES_ALL).is_ok() {
            let n = (need as usize / std::mem::size_of::<HMODULE>()).min(mods.len());
            s.mods = n as u64;
            s.mod_mb = (0..n)
                .map(|i| {
                    let mut mi = MODULEINFO::default();
                    match GetModuleInformation(h, mods[i], &mut mi, std::mem::size_of::<MODULEINFO>() as u32) {
                        Ok(_) => mi.SizeOfImage as u64,
                        Err(_) => 0,
                    }
                })
                .sum::<u64>()
                / (1024 * 1024);
        }
        s.threads = thread_count(std::process::id());
    }
    s
}

/// JSONL 记录器：每行一个阶段/时刻指标，附瞬时 CPU（单核等效 %）
pub struct Recorder {
    f: std::io::BufWriter<std::fs::File>,
    clock: Clock,
    pub base: Snap,
    last: Option<(f64, u64)>,
}

impl Recorder {
    pub fn new(path: &str) -> Self {
        if let Some(p) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(p).ok();
        }
        // 来能来路：计划任务/不同工作目录下相对路径会写失败，统一按工程根解析
        let p = crate::harness::abs(path);
        if let Some(d) = std::path::Path::new(&p).parent() {
            std::fs::create_dir_all(d).ok();
        }
        let f = std::io::BufWriter::new(std::fs::File::create(&p).unwrap_or_else(|e| panic!("创建指标文件失败 {p}: {e}")));
        let clock = Clock::new();
        let base = snap_deep();
        Self { f, clock, base, last: None }
    }

    pub fn ms(&self) -> f64 {
        self.clock.ms()
    }

    /// tag=阶段名；extra=附加 k-v（文件大小、行数等）
    pub fn log(&mut self, tag: &str, extra: &[(&str, String)]) -> Snap {
        self.log_at(tag, extra, snap())
    }

    /// 里程碑采样：额外带模块数与线程数
    pub fn log_deep(&mut self, tag: &str, extra: &[(&str, String)]) -> Snap {
        self.log_at(tag, extra, snap_deep())
    }

    fn log_at(&mut self, tag: &str, extra: &[(&str, String)], s: Snap) -> Snap {
        let ms = self.clock.ms();
        let inst = match self.last {
            // Δt < 100ms 的瞬时 CPU 没有统计意义（会把偶发工作算成几十%），下限钳到 100ms
            Some((pms, pcpu)) => (s.cpu_100ns - pcpu) as f64 / 1e4 / (ms - pms).max(100.0) * 100.0,
            None => 0.0,
        };
        self.last = Some((ms, s.cpu_100ns));
        let cpu_total = (s.cpu_100ns - self.base.cpu_100ns) as f64 / 1e4;
        let ex = extra.iter().map(|(k, v)| format!("\"{k}\":\"{v}\"")).collect::<Vec<_>>().join(",");
        let _ = writeln!(
            self.f,
            "{{\"tag\":\"{tag}\",\"t_ms\":{ms:.1},\"cpu_ms\":{cpu_total:.1},\"cpu_pct\":{inst:.3},\"ws_kb\":{},\"pws_kb\":{},\"priv_kb\":{},\"peak_ws_kb\":{},\"mods\":{},\"mod_mb\":{},\"gdi\":{},\"usr\":{},\"threads\":{},\"handles\":{},\"faults\":{}{}}}",
            s.ws / 1024,
            s.pws / 1024,
            s.priv_bytes / 1024,
            s.peak_ws / 1024,
            s.mods,
            s.mod_mb,
            s.gdi,
            s.usr,
            s.threads,
            s.handles,
            s.faults,
            if ex.is_empty() { String::new() } else { format!(",{ex}") }
        );
        s
    }

    pub fn flush(&mut self) {
        let _ = self.f.flush();
    }
}

/// 枚举已加载模块的 (文件名, 映像大小)；定位窗口创建时注入的系统 DLL
pub fn module_list() -> Vec<(String, u64)> {
    unsafe {
        let h = GetCurrentProcess();
        let mut mods = [HMODULE::default(); 1024];
        let mut need = 0u32;
        if EnumProcessModulesEx(h, mods.as_mut_ptr(), (mods.len() * std::mem::size_of::<HMODULE>()) as u32, &mut need, LIST_MODULES_ALL).is_err() {
            return vec![];
        }
        let n = (need as usize / std::mem::size_of::<HMODULE>()).min(mods.len());
        (0..n)
            .map(|i| {
                let mut name = [0u16; 600];
                let len = windows::Win32::System::LibraryLoader::GetModuleFileNameW(mods[i], &mut name);
                let mut mi = MODULEINFO::default();
                GetModuleInformation(h, mods[i], &mut mi, std::mem::size_of::<MODULEINFO>() as u32).ok();
                (String::from_utf16_lossy(&name[..len as usize]), mi.SizeOfImage as u64)
            })
            .collect()
    }
}

/// 一行人类可读摘要
pub fn line(tag: &str, s: &Snap, ms: f64) -> String {
    format!(
        "{tag}: t={ms:.0}ms WS={:.1}MiB PWS={:.1}MiB PRIV={:.1}MiB mods={}({}MB) gdi={} thr={} hnd={}",
        s.ws as f64 / 1048576.0,
        s.pws as f64 / 1048576.0,
        s.priv_bytes as f64 / 1048576.0,
        s.mods,
        s.mod_mb,
        s.gdi,
        s.threads,
        s.handles
    )
}

pub const _CB_EX: u32 = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
