//! 分阶段内存拆解：Rust 空窗口 → Scintilla → RichEdit → 载入 1/10MiB 文档。
//! 目的：回答「scintilla + richedit + rust gui 的基本内存占用」，并区分共享/私有。
//!
//! mem_probe [--files testdata/large/md_1MiB_norm.md,...] [--preview] [--idle 3] [--metrics artifacts/mem.jsonl]

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi as gdi;
use windows::Win32::UI::WindowsAndMessaging as wm;

use wild_mark::harness;
use wild_mark::md2rtf::{self, Cp, Opt};
use wild_mark::metrics::{Recorder, Snap};
use wild_mark::{richedit, scintilla, win as w};

const CLS: &str = "WildMemProbe";

static mut SCI: HWND = HWND(std::ptr::null_mut());
static mut RICH: HWND = HWND(std::ptr::null_mut());

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        wm::WM_SIZE => {
            // 左右分栏：编辑区 55%，预览区 45%
            let (cw, ch) = w::client_size(hwnd);
            let lw = (cw * 55) / 100;
            if !SCI.0.is_null() {
                let _ = wm::MoveWindow(SCI, 0, 0, lw, ch, true);
            }
            if !RICH.0.is_null() {
                let _ = wm::MoveWindow(RICH, lw + 3, 0, (cw - lw - 3).max(10), ch, true);
            }
            harness::def(hwnd, msg, wp, lp)
        }
        wm::WM_CLOSE => {
            wm::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => harness::def(hwnd, msg, wp, lp),
    }
}

/// 阶段打点：相对上一阶段的增量
struct Stages {
    prev: Snap,
    rows: Vec<String>,
}

impl Stages {
    fn mark(&mut self, rec: &mut Recorder, name: &str, extra: &[(&str, String)]) {
        let s = rec.log_deep(name, extra);
        let d = |a: u64, b: u64| a.saturating_sub(b) as f64 / 1024.0;
        self.rows.push(format!(
            "{:<26} WS={:>8.2}MiB(+{:>6.0}KiB)  PWS={:>7.2}MiB(+{:>6.0}KiB)  PRIV={:>7.2}MiB  mods={:>3}({:>3}MB)  gdi={:>4}  thr={}  hnd={}",
            name,
            s.ws as f64 / 1048576.0,
            d(s.ws, self.prev.ws),
            s.pws as f64 / 1048576.0,
            d(s.pws, self.prev.pws),
            s.priv_bytes as f64 / 1048576.0,
            s.mods,
            s.mod_mb,
            s.gdi,
            s.threads,
            s.handles
        ));
        self.prev = s;
    }
}

fn main() {
    let a = harness::args();
    let mpath = harness::pathv(&a, "metrics", "artifacts/mem.jsonl");
    harness::init_rec(&mpath);
    let files: Vec<String> = harness::strv(&a, "files", "testdata/large/md_1MiB_norm.md,testdata/large/md_10MiB_norm.md")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let preview = harness::flag(&a, "preview");
    let idle = harness::num(&a, "idle", 3.0);
    let hidden = harness::flag(&a, "hidden");
    let no_menu = harness::flag(&a, "no-menu");
    let dump_mods = harness::flag(&a, "mods");
    // 模块清单：定位窗口创建时被系统注入的 DLL（交互会话 PWS 的主要来源）
    let mods_dump = |tag: &str| {
        if !dump_mods {
            return;
        }
        let mut ms = wild_mark::metrics::module_list();
        ms.sort_by(|x, y| y.1.cmp(&x.1));
        let p = harness::abs(&format!("artifacts/mem.mods.{tag}.txt"));
        let body: String = ms.iter().map(|(n, s)| format!("{:>9.2}MiB  {}\n", *s as f64 / 1048576.0, n)).collect();
        std::fs::write(&p, format!("# {tag}  modules={}\n{body}", ms.len())).ok();
    };
    let mut rec = Recorder::new(&harness::abs(&harness::strv(&a, "extra_metrics", "artifacts/mem_stage.jsonl")));
    let mut st = Stages { prev: Snap::default(), rows: Vec::new() };

    unsafe {
        let _ = w::enable_dpi_awareness();
        let hi = w::hinstance();
        st.mark(&mut rec, "s0_process_entry", &[]);

        let menu: Option<windows::Win32::UI::WindowsAndMessaging::HMENU> = if no_menu {
            None
        } else {
            Some(w::build_menu(&[("文件(&F)", &[("打开(&O)…", 101), ("退出(&X)", 103)])]).unwrap())
        };
        w::register_class(&w::ClassDesc { name: CLS, proc: wndproc, bg: COLORREF(0x00FFFFFF) }, hi).expect("类注册失败");
        let hwnd = w::create_window(CLS, "wild-markdown 内存拆解探针", wm::WS_OVERLAPPEDWINDOW, 1280, 800, menu, hi, std::ptr::null_mut()).unwrap();
        let _ = wm::ShowWindow(hwnd, if hidden { wm::SW_HIDE } else { wm::SW_SHOWNORMAL });
        let _ = gdi::UpdateWindow(hwnd);
        st.mark(&mut rec, "s1_host_window", &[("mode", if hidden { "hidden".into() } else { "visible+menu".into() })]);
        mods_dump("s1");
        harness::quiet(idle, 1.0);
        rec.log("s1_idle", &[]);

        // ---- Scintilla ----
        let sci_dll = harness::find_dll("native/scintilla/bin/Scintilla.dll").expect("找不到 Scintilla.dll");
        let _ = scintilla::load_dll(&sci_dll).expect("Scintilla.dll 载入失败");
        st.mark(&mut rec, "s2_scintilla_dll", &[("kb", (std::fs::metadata(&sci_dll).map(|m| m.len()).unwrap_or(0) / 1024).to_string())]);

        let (cw, ch) = w::client_size(hwnd);
        SCI = scintilla::create(hwnd, 0, 0, cw * 55 / 100, ch, 1001).expect("Scintilla 窗口创建失败");
        let lex_dll = harness::find_dll("native/lexilla/bin/lexilla.dll");
        let note = scintilla::configure(SCI, &scintilla::SciOpts::default(), lex_dll.as_deref()).unwrap_or_default();
        let _ = wm::ShowWindow(hwnd, wm::SW_SHOWNORMAL);
        let _ = gdi::UpdateWindow(hwnd);
        st.mark(&mut rec, "s3_scintilla_empty", &[("lexer", note)]);
        harness::quiet(idle, 1.0);
        rec.log("s3_idle", &[]);

        // ---- RichEdit ----
        let _ = richedit::load_msftedit().expect("Msftedit.dll 载入失败");
        RICH = richedit::create(hwnd, cw * 55 / 100 + 3, 0, cw * 45 / 100 - 3, ch, 1002, true).expect("RichEdit 创建失败");
        richedit::wrap_to_window(RICH);
        let _ = gdi::UpdateWindow(hwnd);
        st.mark(&mut rec, "s4_richedit_empty", &[]);
        harness::quiet(idle, 1.0);
        rec.log("s4_idle", &[]);

        // ---- 文档载入 ----
        for (i, f) in files.iter().enumerate() {
            let path = harness::abs(f);
            let Ok(bytes) = std::fs::read(&path) else {
                println!("跳过不存在的文件 {path}");
                continue;
            };
            let t = std::time::Instant::now();
            scintilla::allocate(SCI, bytes.len() + 1024);
            scintilla::set_text(SCI, &bytes);
            let load_ms = t.elapsed().as_secs_f64() * 1000.0;
            let lc = scintilla::line_count(SCI);
            st.mark(
                &mut rec,
                &format!("s5_edit_{}_{}", i + 1, bytes.len() / 1048576),
                &[("mb", format!("{:.2}", bytes.len() as f64 / 1048576.0)), ("load_ms", format!("{load_ms:.1}")), ("lines", lc.to_string())],
            );
            harness::quiet(idle, 1.0);
            rec.log("s5_idle", &[]);

            if preview {
                let md = String::from_utf8_lossy(&bytes).to_string();
                let t = std::time::Instant::now();
                let out = md2rtf::to_rtf(&md, &Opt { cp: Cp::Uni, ..Default::default() });
                let gen_ms = t.elapsed().as_secs_f64() * 1000.0;
                let t = std::time::Instant::now();
                let _ = richedit::set_rtf(RICH, &out.rtf);
                let in_ms = t.elapsed().as_secs_f64() * 1000.0;
                st.mark(
                    &mut rec,
                    &format!("s6_preview_{}", i + 1),
                    &[("rtf_kb", (out.rtf.len() / 1024).to_string()), ("gen_ms", format!("{gen_ms:.1}")), ("stream_ms", format!("{in_ms:.1}"))],
                );
                harness::quiet(idle, 1.0);
                rec.log("s6_idle", &[]);
            }
        }

        if harness::flag(&a, "shot") {
            let _ = w::capture_bmp(hwnd, &harness::abs("artifacts/mem_split.bmp"));
        }

        // ---- 释放文档后是否回落 ----
        scintilla::set_text(SCI, b"");
        richedit::set_text(RICH, b"");
        let _ = gdi::UpdateWindow(hwnd);
        st.mark(&mut rec, "s7_docs_cleared", &[]);
        harness::quiet(idle, 1.0);
        rec.log("s7_idle", &[]);

        let _ = wm::DestroyWindow(hwnd);
        wm::PostQuitMessage(0);
        w::pump_until_quit();
        st.mark(&mut rec, "s8_window_destroyed", &[]);

        println!("\n=== 分阶段内存（{}）===", harness::root());
        for r in &st.rows {
            println!("{r}");
        }
        let mut sum = harness::summarize(&harness::abs(&mpath), &harness::abs("target/release/mem_probe.exe"));
        sum.insert("stages".into(), st.rows.len().to_string());
        harness::write_json(&sum, &harness::abs("artifacts/mem.summary.json"));
        harness::print_map(&sum);
        rec.flush();
    }
}
