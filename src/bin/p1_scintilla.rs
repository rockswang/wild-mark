//! P1：Scintilla 编辑器 + 中文输入 + 大文件。
//! 覆盖 plan.md 4.2：载入耗时/内存、键入延迟、容器自染 vs Lexilla 高亮、撤销/查找/剪贴板、
//! 以及可选的 IME 端到端探针（SendInput 打拼音 → 回读编辑器文本判定是否出字）。
//!
//! p1_scintilla [文件] [--lexer lexilla|container|null] [--wrap] [--no-linenum] [--caretline]
//!              [--bench-sizes 1,10,32,64,128] [--bench-edits 200] [--ime-probe] [--ime-text nihao]
//!              [--keep 0] [--idle 2] [--shot] [--metrics artifacts/p1.jsonl]

use std::time::Instant;
use windows::core::Result;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi as gdi;
use windows::Win32::UI::Input::KeyboardAndMouse as kb;

use windows::Win32::UI::WindowsAndMessaging as wm;

use wild_mark::harness;
use wild_mark::metrics::{self, Recorder, Snap};
use wild_mark::{richedit, scintilla as sci, win as w};

const CLS: &str = "WildP1";
static mut SCI_H: HWND = HWND(std::ptr::null_mut());
static mut HOST_H: HWND = HWND(std::ptr::null_mut());
/// 下标对应 IME_MSGS 表：setcontext/start/end/composition/ime_char/select/notify/langchange/char/keydown
static mut CNT: [u32; 12] = [0; 12];
static mut MODS: usize = 0;

const IME_MSGS: [u32; 8] = [
    wm::WM_IME_SETCONTEXT,
    wm::WM_IME_STARTCOMPOSITION,
    wm::WM_IME_ENDCOMPOSITION,
    wm::WM_IME_COMPOSITION,
    wm::WM_IME_CHAR,
    wm::WM_IME_SELECT,
    wm::WM_IME_NOTIFY,
    wm::WM_INPUTLANGCHANGE,
];

unsafe fn note(msg: u32) {
    if let Some(i) = IME_MSGS.iter().position(|m| *m == msg) {
        CNT[i] += 1;
    }
    if msg == wm::WM_CHAR {
        CNT[8] += 1;
    }
    if msg == wm::WM_KEYDOWN {
        CNT[9] += 1;
    }
}

/// 传统子类化（SetWindowLongPtrW）而不用 SetWindowSubclass：
/// 后者只在 comctl32 v6 里，无 manifest 的 Rust 程序会直接加载失败
static mut OLD_PROC: isize = 0;
unsafe fn old_proc() -> wm::WNDPROC { std::mem::transmute_copy(&OLD_PROC) }

unsafe extern "system" fn sub_proc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    note(m);
    wm::CallWindowProcW(old_proc(), h, m, wp, lp)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        wm::WM_SIZE => {
            let (cw, ch) = w::client_size(hwnd);
            if !SCI_H.0.is_null() {
                let _ = wm::MoveWindow(SCI_H, 0, 0, cw, ch, true);
            }
            harness::def(hwnd, msg, wp, lp)
        }
        // 容器自染路径：Scintilla 索要样式时由 Rust 侧按行染色
        wm::WM_COMMAND if ((wp.0 >> 16) & 0xffff) as u32 == sci::SCN_STYLENEEDED => {
            let pos = lp.0.max(0) as usize;
            if pos < sci::text_len(SCI_H) {
                sci::style_container(SCI_H, pos);
            }
            LRESULT(0)
        }
        wm::WM_NOTIFY => {
            let nh = &*(lp.0 as *const windows::Win32::UI::Controls::NMHDR);
            if nh.code == sci::SCN_MODIFIED {
                MODS += 1;
            }
            harness::def(hwnd, msg, wp, lp)
        }
        wm::WM_CLOSE => {
            wm::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => {
            note(msg);
            harness::def(hwnd, msg, wp, lp)
        }
    }
}

fn cnt_json() -> Vec<(&'static str, String)> {
    let names = ["ime_setcontext", "ime_start_comp", "ime_end_comp", "ime_composition", "ime_char", "ime_select", "ime_notify", "langchange", "wm_char", "wm_keydown"];
    unsafe { names.iter().enumerate().map(|(i, n)| (*n, CNT[i].to_string())).collect() }
}

/// 单尺寸基准：打开→首绘耗时、内存、键入延迟、尾部编辑延迟
unsafe fn bench_one(rec: &mut Recorder, mb: usize, opts: &sci::SciOpts, edits: usize, lex: Option<&str>) {
    let path = format!("testdata/large/md_{mb}MiB_norm.md");
    let ap = harness::abs(&path);
    if !std::path::Path::new(&ap).exists() {
        println!("SKIP {ap} 不存在（先跑 gen_testdata --large）");
        return;
    }
    let t0 = Instant::now();
    let bytes = std::fs::read(&ap).unwrap();
    let read_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    sci::allocate(SCI_H, bytes.len() + 4096);
    sci::set_text(SCI_H, &bytes);
    let set_ms = t.elapsed().as_secs_f64() * 1000.0;
    let note = sci::configure(SCI_H, opts, lex).unwrap_or_default();
    let t = Instant::now();
    let _ = gdi::UpdateWindow(SCI_H);
    let paint_ms = t.elapsed().as_secs_f64() * 1000.0;
    let lines = sci::line_count(SCI_H);
    let st = sci::status(SCI_H);
    let s = rec.log_deep(
        "load",
        &[
            ("mb", mb.to_string()),
            ("read_ms", format!("{read_ms:.1}")),
            ("set_ms", format!("{set_ms:.1}")),
            ("paint_ms", format!("{paint_ms:.1}")),
            ("lines", lines.to_string()),
            ("status", st.to_string()),
            ("wrap", opts.wrap.to_string()),
            ("lexer", note.clone()),
        ],
    );
    println!(
        "LOAD {mb}MiB read={read_ms:.0} set={set_ms:.0} paint={paint_ms:.0} lines={lines} WS={:.1} PWS={:.1} PRIV={:.1}MiB lexer={note}",
        s.ws as f64 / 1048576.0,
        s.pws as f64 / 1048576.0,
        s.priv_bytes as f64 / 1048576.0
    );

    if edits > 0 {
        // 键入延迟：顶栏逐字插入 + 强制重绘（等价一次按键的完整代价）
        let mut lat: Vec<f64> = Vec::with_capacity(edits);
        sci::send(SCI_H, sci::SCI_SETSEL, 0, 0);
        for i in 0..edits {
            let t = Instant::now();
            sci::replace_sel(SCI_H, "测");
            sci::send(SCI_H, sci::SCI_GOTOPOS, 0, 0);
            let _ = gdi::UpdateWindow(SCI_H);
            lat.push(t.elapsed().as_secs_f64() * 1000.0);
            if i == edits / 2 {
                sci::goto_line(SCI_H, lines / 2); // 中途跳到文档中部，测尾部代价
            }
        }
        lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg = lat.iter().sum::<f64>() / lat.len() as f64;
        let (p50, p95, mx) = (lat[lat.len() / 2], lat[(lat.len() * 95) / 100], lat[lat.len() - 1]);
        rec.log_deep("type_lat", &[("mb", mb.to_string()), ("n", edits.to_string()), ("avg_ms", format!("{avg:.3}")), ("p50_ms", format!("{p50:.3}")), ("p95_ms", format!("{p95:.3}")), ("max_ms", format!("{mx:.3}"))]);
        println!("TYPE {mb}MiB n={edits} avg={avg:.2}ms p50={p50:.2} p95={p95:.2} max={mx:.2}ms");
        for _ in 0..edits {
            sci::send(SCI_H, sci::SCI_UNDO, 0, 0);
        }
    }
    // 尾部插入：超大文档随机写入代价
    let tail = sci::text_len(SCI_H);
    let t = Instant::now();
    sci::insert_text(SCI_H, tail, "尾部中文\n");
    let _ = gdi::UpdateWindow(SCI_H);
    rec.log_deep("tail_edit", &[("mb", mb.to_string()), ("ms", format!("{:.3}", t.elapsed().as_secs_f64() * 1000.0))]);
    sci::set_text(SCI_H, b"");
    let _ = gdi::UpdateWindow(SCI_H);
}

/// 端到端 IME：切到 zh-CN 布局 → 聚焦 Scintilla → SendInput 拼音+空格 → 回读文本判定是否出汉字
unsafe fn ime_probe(rec: &mut Recorder, text: &str, do_send: bool) {
    rec.log(
        "ime_begin",
        &[
            ("layout", "00000804 zh-CN".into()),
            ("sent", text.into()),
            ("mode", if do_send { "send-keys".into() } else { "passive".into() }),
            ("note", "被动模式只验证 TSF 附着；发键模式会把按键注入前台窗口，可能落入用户正在使用的应用".into()),
        ],
    );
    let cur = kb::GetKeyboardLayout(0);
    let klid = w::wide("00000804");
    let hk = kb::LoadKeyboardLayoutW(w::pcw(&klid), kb::KLF_ACTIVATE).unwrap_or_default();
    let _ = kb::ActivateKeyboardLayout(hk, kb::KLF_SETFORPROCESS);
    // 抢焦点：普通 SetForegroundWindow 会被前台锁拒绝，先 AttachThreadInput 到前台线程
    let fg = wm::GetForegroundWindow();
    let fg_tid = wm::GetWindowThreadProcessId(fg, None);
    let my_tid = windows::Win32::System::Threading::GetCurrentThreadId();
    let attached = windows::Win32::System::Threading::AttachThreadInput(my_tid, fg_tid, true).as_bool();
    let _ = wm::SetForegroundWindow(HOST_H);
    let _ = kb::SetFocus(SCI_H);
    let _ = windows::Win32::System::Threading::AttachThreadInput(my_tid, fg_tid, false);
    // 子类化 Scintilla 窗口，观察 IME 消息是否真正到达控件
    OLD_PROC = wm::SetWindowLongPtrW(SCI_H, wm::GWLP_WNDPROC, sub_proc as isize);
    harness::sleep_pump(500);
    let fg_after = wm::GetForegroundWindow();
    let focus_is_sci = kb::SetFocus(SCI_H).unwrap_or_default() == SCI_H;
    rec.log(
        "ime_focus",
        &[
            ("fg_before", format!("{:?}", fg.0 as usize)),
            ("fg_after_is_host", (fg_after == HOST_H).to_string()),
            ("attach_ok", attached.to_string()),
            ("focus_sci_ok", focus_is_sci.to_string()),
            ("layout", format!("{:?}", kb::GetKeyboardLayout(0))),
        ],
    );
    let _before = sci::text_len(SCI_H);
    let mut sent_total = 0u32;
    if !do_send {
        // 被动模式：不注入按键，只验证 TSF 消息链路与中文文本显示/复制
        let all = sci::get_text(SCI_H, sci::text_len(SCI_H));
        let _ = wm::SetWindowLongPtrW(SCI_H, wm::GWLP_WNDPROC, OLD_PROC);
        let _ = kb::ActivateKeyboardLayout(cur, kb::KLF_SETFORPROCESS);
        let cjk = String::from_utf8_lossy(&all).chars().filter(|c| (0x4e00..=0x9fff).contains(&(*c as u32))).count();
        let mut v: Vec<(&str, String)> = vec![("passive", "1".into()), ("text_len", all.len().to_string()), ("cjk_chars", cjk.to_string())];
        v.extend(cnt_json().iter().map(|(k, s2)| (*k, s2.clone())));
        rec.log("ime_result", &v);
        println!("IME passive: text_len={} cjk={} counts={:?}", all.len(), cjk, cnt_json().iter().map(|(k, v2)| format!("{k}={v2}")).collect::<Vec<_>>().join(","));
        return;
    }
    for ch in text.chars() {
        let vk = kb::VkKeyScanW(ch as u16);
        if vk < 0 {
            continue;
        }
        let code = kb::VIRTUAL_KEY(vk as u16);
        let mk = |up: bool| kb::INPUT {
            r#type: kb::INPUT_KEYBOARD,
            Anonymous: kb::INPUT_0 {
                ki: kb::KEYBDINPUT {
                    wVk: code,
                    wScan: 0,
                    dwFlags: if up { kb::KEYEVENTF_KEYUP } else { kb::KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let n = kb::SendInput(&[mk(false), mk(true)], std::mem::size_of::<kb::INPUT>() as i32);
        sent_total += n;
        harness::sleep_pump(120);
    }
    rec.log("ime_sent", &[("keydown", CNT[9].to_string()), ("char", CNT[8].to_string()), ("sentinput_events", sent_total.to_string())]);
    // 空格选词
    let sp = |up: bool| kb::INPUT {
        r#type: kb::INPUT_KEYBOARD,
        Anonymous: kb::INPUT_0 {
            ki: kb::KEYBDINPUT {
                wVk: kb::VK_SPACE,
                wScan: 0,
                dwFlags: if up { kb::KEYEVENTF_KEYUP } else { kb::KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    kb::SendInput(&[sp(false), sp(true)], std::mem::size_of::<kb::INPUT>() as i32);
    harness::sleep_pump(800);
    let all = sci::get_text(SCI_H, sci::text_len(SCI_H));
    let caret = sci::send(SCI_H, sci::SCI_GETCURRENTPOS, 0, 0) as usize;
    let _ = wm::SetWindowLongPtrW(SCI_H, wm::GWLP_WNDPROC, OLD_PROC);
    let _ = kb::ActivateKeyboardLayout(cur, kb::KLF_SETFORPROCESS);
    // IME 在光标处插入，读光标前后片段而不是末尾偏移
    let lo = caret.saturating_sub(30);
    let near = String::from_utf8_lossy(&all[lo.min(all.len())..((caret + 10).min(all.len()))]).to_string();
    let cjk = near.chars().filter(|c| (0x4e00..=0x9fff).contains(&(*c as u32))).count();
    let mut v: Vec<(&str, String)> = vec![
        ("sent", text.to_string()),
        ("caret", caret.to_string()),
        ("near_caret", near.trim().chars().take(24).collect::<String>()),
        ("cjk_chars", cjk.to_string()),
        ("pass", (cjk > 0).to_string()),
    ];
    v.extend(cnt_json().iter().map(|(k, s)| (*k, s.clone())));
    rec.log("ime_result", &v);
    println!("IME sent={text} caret={caret} near={near:?} cjk={cjk} counts={:?}", cnt_json().iter().map(|(k, v2)| format!("{k}={v2}")).collect::<Vec<_>>().join(","));
}

fn main() {
    let a = harness::args();
    let mpath = harness::pathv(&a, "metrics", "artifacts/p1.jsonl");
    harness::init_rec(&mpath);
    let mut rec = Recorder::new(&harness::abs(&mpath));
    let lexer = match harness::strv(&a, "lexer", "lexilla").as_str() {
        "container" => sci::LexerMode::Container,
        "null" => sci::LexerMode::None,
        _ => sci::LexerMode::Lexilla,
    };
    let opts = sci::SciOpts {
        wrap: harness::flag(&a, "wrap"),
        line_numbers: !harness::flag(&a, "no-linenum"),
        font_pt: harness::num(&a, "font", 11.0) as usize,
        lexer: lexer.clone(),
        caret_line: harness::flag(&a, "caretline"),
        mod_mask: sci::SC_MODEVENTMASKALL,
    };
    unsafe {
        let _ = w::enable_dpi_awareness();
        let hi = w::hinstance();
        let dll = harness::find_dll("native/scintilla/bin/Scintilla.dll").expect("Scintilla.dll 未构建：先跑 build_native.bat");
        let lex = harness::find_dll("native/lexilla/bin/lexilla.dll");
        rec.log_deep("entry", &[("scintilla_dll", dll.clone()), ("lexilla_dll", lex.clone().unwrap_or_default()), ("lexer_mode", format!("{lexer:?}"))]);
        sci::load_dll(&harness::abs(&dll)).expect("载入 Scintilla.dll 失败");
        w::register_class(&w::ClassDesc { name: CLS, proc: wndproc, bg: COLORREF(0x00FFFFFF) }, hi).unwrap();
        HOST_H = w::create_window(CLS, "wild-markdown P1 Scintilla", wm::WS_OVERLAPPEDWINDOW, 1200, 820, None, hi, std::ptr::null_mut()).unwrap();
        let _ = wm::ShowWindow(HOST_H, wm::SW_SHOWNORMAL);
        let (cw, ch) = w::client_size(HOST_H);
        SCI_H = sci::create(HOST_H, 0, 0, cw, ch, 1001).expect("创建 Scintilla 失败");
        let note = sci::configure(SCI_H, &opts, lex.as_deref()).unwrap_or_default();
        let _ = gdi::UpdateWindow(HOST_H);
        let s = rec.log_deep("editor_created", &[("lexer", note.clone()), ("wrap", opts.wrap.to_string()), ("linenum", opts.line_numbers.to_string())]);
        println!("{}", metrics::line("editor_created(empty)", &s, rec.ms()));
        harness::quiet(harness::num(&a, "idle", 2.0), 1.0);

        // 指定文件
        if let Some(f) = harness::pos(&a, 0) {
            let ap = harness::abs(&f);
            if std::path::Path::new(&ap).exists() {
                let bytes = std::fs::read(&ap).unwrap();
                let t = Instant::now();
                sci::allocate(SCI_H, bytes.len() + 4096);
                sci::set_text(SCI_H, &bytes);
                let _ = gdi::UpdateWindow(SCI_H);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let s = rec.log_deep("open_file", &[("file", f.clone()), ("mb", format!("{:.2}", bytes.len() as f64 / 1048576.0)), ("ms", format!("{ms:.1}")), ("lines", sci::line_count(SCI_H).to_string())]);
                println!("{}", metrics::line(&format!("open {f} {ms:.0}ms"), &s, rec.ms()));
            } else {
                println!("文件不存在：{ap}");
            }
        }

        // 多尺寸
        let sizes: Vec<usize> = harness::strv(&a, "bench-sizes", "").split(',').filter_map(|s| s.trim().parse().ok()).collect();
        let edits = harness::num(&a, "bench-edits", 200.0) as usize;
        for mb in sizes {
            bench_one(&mut rec, mb, &opts, edits, lex.as_deref());
        }

        // 编辑功能回归：查找/复制粘贴/撤销
        sci::set_text(SCI_H, "第一行 中文 Hello\n第二行 世界 World\n第三行 中文 again\n".as_bytes());
        sci::send(SCI_H, sci::SCI_SETSEARCHFLAGS, sci::SC_FIND_MATCHCASE, 0);
        let hit = sci::find_text(SCI_H, "中文", 0, sci::text_len(SCI_H), sci::SC_FIND_MATCHCASE);
        rec.log("search", &[("hit", format!("{hit:?}"))]);
        sci::send(SCI_H, sci::SCI_SELECTALL, 0, 0);
        sci::send(SCI_H, sci::SCI_COPY, 0, 0);
        let clip = richedit::clipboard_text();
        let want = "第一行 中文 Hello\n第二行 世界 World\n第三行 中文 again\n";
        rec.log("copy", &[("clip_len", clip.len().to_string()), ("eq_lf", (clip.replace("\r\n", "\n") == want).to_string()), ("eq_crlf", (clip.replace("\n", "\r\n") == want.replace("\n", "\r\n")).to_string())]);
        let before = sci::text_len(SCI_H);
        sci::send(SCI_H, sci::SCI_SETSEL, 0, 0);
        sci::replace_sel(SCI_H, "插入");
        let mid = sci::text_len(SCI_H);
        sci::send(SCI_H, sci::SCI_UNDO, 0, 0);
        let after = sci::text_len(SCI_H);
        rec.log("undo", &[("before", before.to_string()), ("mid", mid.to_string()), ("after", after.to_string()), ("ok", (after == before).to_string())]);
        sci::send(SCI_H, sci::SCI_REDO, 0, 0);
        rec.log("redo", &[("len", sci::text_len(SCI_H).to_string()), ("ok", (sci::text_len(SCI_H) == mid).to_string())]);
        // 粘贴：把系统剪贴板内容插入
        sci::send(SCI_H, sci::SCI_SETSEL, 0, 0);
        sci::send(SCI_H, sci::SCI_PASTE, 0, 0);
        rec.log("paste", &[("len", sci::text_len(SCI_H).to_string())]);

        if harness::flag(&a, "ime-probe") {
            ime_probe(&mut rec, &harness::strv(&a, "ime-text", "nihao"), harness::flag(&a, "ime-send"));
        }
        if harness::flag(&a, "shot") {
            let _ = w::capture_bmp(HOST_H, &harness::abs(&harness::pathv(&a, "shot", "artifacts/p1.bmp")));
        }
        let keep = harness::num(&a, "keep", 0.0);
        if keep > 0.0 {
            harness::quiet(keep, 2.0);
        }
        let mut v = vec![("mods", MODS.to_string())];
        v.extend(cnt_json().iter().map(|(k, s2)| (*k, s2.clone())));
        let s: Snap = rec.log_deep("final", &v);
        println!("{}", metrics::line("final", &s, rec.ms()));
        rec.flush();
        let mut sum = harness::summarize(&harness::abs(&mpath), &harness::abs("target/release/p1_scintilla.exe"));
        sum.insert("lexer".into(), note);
        sum.insert("wrap".into(), opts.wrap.to_string());
        harness::write_json(&sum, &harness::abs("artifacts/p1.summary.json"));
        harness::print_map(&sum);
        let _ = wm::DestroyWindow(HOST_H);
        wm::PostQuitMessage(0);
    }
}

#[allow(dead_code)]
fn _unused(r: Result<()>) -> Option<()> {
    r.ok()
}
