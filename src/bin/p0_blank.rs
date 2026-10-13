//! P0：Win32 空窗口 + 菜单 + 消息循环（不加载 Scintilla / RichEdit / Markdown 解析器）。
//! 对应 plan.md 4.1 与「原型准入门槛」：空窗口工作集 ≤10MiB、空闲 CPU <0.5%、发布体积 <2MiB。
//!
//! p0_blank [--idle 60] [--min 60] [--drag 30] [--cold 10] [--long 0] [--hidden]
//!          [--fast|--quick] [--metrics artifacts/p0.jsonl] [--shot artifacts/p0.bmp]

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi as gdi;
use windows::Win32::UI::WindowsAndMessaging as wm;

use wild_mark::harness::{self, phase, Kind};
use wild_mark::metrics;
use wild_mark::win as w;

const CLS: &str = "WildP0";

/// 每窗口计数：证明空闲期无重绘、无定时器
#[derive(Default)]
struct Cnt {
    paints: usize,
    sizes: usize,
    timers: usize,
    other: usize,
}

static CNT: std::sync::OnceLock<std::sync::Mutex<Cnt>> = std::sync::OnceLock::new();
fn cnt() -> &'static std::sync::Mutex<Cnt> {
    CNT.get_or_init(|| std::sync::Mutex::new(Cnt::default()))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        wm::WM_CREATE => {
            w::set_userdata(hwnd, w::create_param(lp));
            LRESULT(0)
        }
        wm::WM_SIZE => {
            cnt().lock().unwrap().sizes += 1;
            harness::def(hwnd, msg, wp, lp)
        }
        wm::WM_PAINT => {
            let mut ps = gdi::PAINTSTRUCT::default();
            let hdc = gdi::BeginPaint(hwnd, &mut ps);
            let n = {
                let mut c = cnt().lock().unwrap();
                c.paints += 1;
                c.paints
            };
            let r = &mut w::client_rect(hwnd);
            let mut t = w::wide(&format!("wild-markdown P0 · 空窗口 · 重绘 #{n} · 中文渲染检查"));
            let _ = gdi::DrawTextW(hdc, &mut t, r as *mut _, gdi::DT_CENTER | gdi::DT_VCENTER | gdi::DT_SINGLELINE);
            let _ = gdi::EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        wm::WM_TIMER => {
            cnt().lock().unwrap().timers += 1; // 本原型刻意不建定时器；非 0 说明有隐藏循环
            LRESULT(0)
        }
        wm::WM_COMMAND => {
            harness::sample("command", &[("id", (wp.0 & 0xffff).to_string())]);
            LRESULT(0)
        }
        wm::WM_CLOSE => {
            wm::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => harness::def(hwnd, msg, wp, lp),
    }
}

fn main() {
    let a = harness::args();
    let sc = harness::scale(&a);
    let mpath = harness::strv(&a, "metrics", "artifacts/p0.jsonl");
    harness::init_rec(&mpath);
    unsafe {
        let dpi = w::enable_dpi_awareness();
        let hi = w::hinstance();
        let entry = metrics::process_age_ms();
        harness::sample("entry", &[("pid", std::process::id().to_string()), ("dpi_aware", dpi.to_string()), ("proc_age_ms", format!("{entry:.1}"))]);

        let menu = w::build_menu(&[
            ("文件(&F)", &[("打开(&O)…", 101), ("保存(&S)", 102), ("退出(&X)", 103)]),
            ("编辑(&E)", &[("撤销(&U)", 201), ("查找替换(&F)", 202)]),
            ("视图(&V)", &[("切换预览(&P)", 301), ("放大(&I)", 302)]),
            ("帮助(&H)", &[("关于(&A)", 401)]),
        ])
        .expect("菜单创建失败");
        let created = metrics::process_age_ms();
        harness::sample("menu-built", &[("proc_age_ms", format!("{created:.1}"))]);

        w::register_class(&w::ClassDesc { name: CLS, proc: wndproc, bg: COLORREF(0x00FAFAFA) }, hi).expect("注册类失败");
        let hwnd = w::create_window(CLS, "wild-markdown P0 空窗口", wm::WS_OVERLAPPEDWINDOW, 980, 660, Some(menu), hi, std::ptr::null_mut())
            .expect("创建窗口失败");
        let _ = wm::ShowWindow(hwnd, if harness::flag(&a, "hidden") { wm::SW_HIDE } else { wm::SW_SHOWNORMAL });
        let _ = gdi::UpdateWindow(hwnd);
        let visible = metrics::process_age_ms();
        let s1 = harness::sample_deep("visible", &[("startup_ms", format!("{visible:.1}")), ("hidden", harness::flag(&a, "hidden").to_string())]);
        println!("{}", metrics::line("visible", &s1, visible));

        let mut ph = vec![
            phase("cold", Kind::Wait, harness::num(&a, "cold", 10.0) * sc),
            phase("static", Kind::Idle, harness::num(&a, "idle", 60.0) * sc),
            phase("min", Kind::Back, harness::num(&a, "min", 60.0) * sc),
            phase("drag", Kind::Drag, harness::num(&a, "drag", 30.0) * sc),
            phase("after", Kind::Idle, harness::num(&a, "after", 10.0) * sc),
        ];
        let long = harness::num(&a, "long", 0.0);
        if long > 0.0 {
            ph.push(phase("longrun", Kind::Idle, long * sc));
        }
        harness::run_phases(hwnd, &ph, |_| {});

        if harness::flag(&a, "shot") {
            let _ = w::capture_bmp(hwnd, &harness::abs(&harness::pathv(&a, "shot", "artifacts/p0.bmp")));
        }
        let c = cnt().lock().unwrap();
        let s2 = harness::sample_deep(
            "final",
            &[("paints", c.paints.to_string()), ("sizes", c.sizes.to_string()), ("timers", c.timers.to_string())],
        );
        println!("{}", metrics::line("final", &s2, harness::ms()));
        harness::flush();
        let paints = c.paints;
        let timers = c.timers;
        let sizes = c.sizes;
        drop(c);
        let _ = wm::DestroyWindow(hwnd);
        wm::PostQuitMessage(0);
        w::pump_until_quit();
        let after = metrics::snap();
        let mut sum = harness::summarize(&harness::abs(&mpath), &harness::abs(&harness::strv(&a, "exe", "target/release/p0_blank.exe")));
        sum.insert("startup_ms".into(), format!("{visible:.1}"));
        sum.insert("paints".into(), paints.to_string());
        sum.insert("timers".into(), timers.to_string());
        sum.insert("sizes".into(), sizes.to_string());
        sum.insert("after_close_pws_mb".into(), format!("{:.2}", after.pws as f64 / 1048576.0));
        sum.insert("after_close_ws_mb".into(), format!("{:.2}", after.ws as f64 / 1048576.0));
        harness::write_json(&sum, &harness::abs("artifacts/p0.summary.json"));
        harness::print_map(&sum);
    }
}
