//! P2：Markdown → RTF → RichEdit 预览管线。
//! 覆盖 plan.md 4.3：转义/Unicode/中文排版、大文件耗时与内存、整篇刷新的滚动与选区保持、代码块底色与表格。
//!
//! p2_preview [文件] [--cp uni|gbk] [--dark] [--bench-sizes 1,10,32,64,128] [--no-render]
//!            [--max-preview 12] [--stress N] [--cases] [--shot] [--idle 2] [--metrics artifacts/p2.jsonl]

use std::time::Instant;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi as gdi;
use windows::Win32::UI::WindowsAndMessaging as wm;

use wild_mark::harness;
use wild_mark::md2rtf::{self, Cp, Opt};
use wild_mark::metrics::Recorder;
use wild_mark::{richedit, win as w};

const CLS: &str = "WildP2";
static mut RICH: HWND = HWND(std::ptr::null_mut());
static mut HOST: HWND = HWND(std::ptr::null_mut());

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        wm::WM_SIZE => {
            let (cw, ch) = w::client_size(hwnd);
            if !RICH.0.is_null() {
                let _ = wm::MoveWindow(RICH, 0, 0, cw, ch, true);
            }
            harness::def(hwnd, msg, wp, lp)
        }
        wm::WM_NOTIFY => {
            // EN_LINK(超链接点击) / EN_SELCHANGED 事件在此消费，正式版用于跳转与同步滚动
            harness::def(hwnd, msg, wp, lp)
        }
        wm::WM_CLOSE => {
            wm::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => harness::def(hwnd, msg, wp, lp),
    }
}

/// 单文档：解析+RTF 生成 → （可选）RichEdit 灌入 → 指标
unsafe fn one(rec: &mut Recorder, path: &str, opt: &Opt, render: bool, tag: &str) -> Option<(usize, usize)> {
    let ap = harness::abs(path);
    let Ok(md) = std::fs::read_to_string(&ap) else {
        println!("SKIP {ap}");
        return None;
    };
    let t = Instant::now();
    let out = md2rtf::to_rtf(&md, opt);
    let gen_ms = t.elapsed().as_secs_f64() * 1000.0;
    let ratio = out.rtf.len() as f64 / md.len() as f64;
    let mut stream_ms = 0f64;
    let mut tlen = 0usize;
    let mut scroll = (0i32, 0i32);
    if render && !RICH.0.is_null() {
        let t = Instant::now();
        let _ = richedit::set_rtf(RICH, &out.rtf);
        stream_ms = t.elapsed().as_secs_f64() * 1000.0;
        tlen = richedit::text_len(RICH);
        let _ = gdi::UpdateWindow(RICH);
        scroll = richedit::scroll_pos(RICH);
    }
    let s = rec.log_deep(
        tag,
        &[
            ("file", path.to_string()),
            ("md_mb", format!("{:.2}", md.len() as f64 / 1048576.0)),
            ("rtf_kb", (out.rtf.len() / 1024).to_string()),
            ("ratio", format!("{ratio:.2}")),
            ("nodes", out.nodes.to_string()),
            ("gen_ms", format!("{gen_ms:.1}")),
            ("gen_mb_s", format!("{:.1}", md.len() as f64 / 1048576.0 / (gen_ms / 1000.0))),
            ("stream_ms", format!("{stream_ms:.1}")),
            ("render", render.to_string()),
            ("text_len", tlen.to_string()),
            ("scroll", format!("{}x{}", scroll.0, scroll.1)),
            ("cp", format!("{:?}", opt.cp)),
        ],
    );
    println!(
        "{tag} {} md={:.2}MiB rtf={:.1}MiB(x{ratio:.2}) gen={gen_ms:.0}ms stream={stream_ms:.0}ms textlen={tlen} WS={:.1}MiB PWS={:.1}MiB PRIV={:.1}MiB",
        path,
        md.len() as f64 / 1048576.0,
        out.rtf.len() as f64 / 1048576.0,
        s.ws as f64 / 1048576.0,
        s.pws as f64 / 1048576.0,
        s.priv_bytes as f64 / 1048576.0
    );
    let dump = std::env::args().any(|a| a == "--dump");
    if dump {
        let p = harness::abs(&format!("artifacts/{}.rtf", path.replace(['/', '.', ' '], "_")));
        std::fs::write(&p, &out.rtf).ok();
    }
    Some((md.len(), out.rtf.len()))
}

/// 整篇重刷压力：验证 plan.md 提出的"一次性替换 RTF 导致滚动/选区/重绘问题"
unsafe fn stress(rec: &mut Recorder, path: &str, opt: &Opt, n: usize) {
    let Ok(md) = std::fs::read_to_string(&harness::abs(path)) else {
        return;
    };
    let out = md2rtf::to_rtf(&md, opt);
    let _ = richedit::set_rtf(RICH, &out.rtf);
    // 先滚到中部并把光标放在中间，再看刷新后是否保持
    let tl = richedit::text_len(RICH);
    richedit::set_sel(RICH, (tl / 2) as i32, (tl / 2) as i32);
    richedit::lines_scroll(RICH, 40);
    let (sx0, sy0) = richedit::scroll_pos(RICH);
    let (c0, c1) = richedit::get_sel(RICH);
    let mut times = Vec::with_capacity(n);
    let (mut keep_scroll, mut keep_sel) = (0usize, 0usize);
    for _ in 0..n {
        let t = Instant::now();
        let _ = richedit::set_rtf(RICH, &out.rtf);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        let (sx, sy) = richedit::scroll_pos(RICH);
        let (a, b) = richedit::get_sel(RICH);
        if (sx, sy) == (sx0, sy0) {
            keep_scroll += 1;
        }
        if (a, b) == (c0, c1) {
            keep_sel += 1;
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let s = rec.log_deep(
        "stress_refresh",
        &[
            ("n", n.to_string()),
            ("avg_ms", format!("{:.1}", times.iter().sum::<f64>() / n as f64)),
            ("max_ms", format!("{:.1}", times[n - 1])),
            ("keep_scroll", format!("{keep_scroll}/{n}")),
            ("keep_sel", format!("{keep_sel}/{n}")),
            ("scroll_before", format!("{sx0},{sy0}")),
            ("sel_before", format!("{c0},{c1}")),
            ("sel_after", format!("{}", richedit::get_sel(RICH).0)),
        ],
    );
    println!(
        "STRESS n={n} avg={:.1}ms keep_scroll={keep_scroll}/{n} keep_sel={keep_sel}/{n} WS={:.1}MiB",
        times.iter().sum::<f64>() / n as f64,
        s.ws as f64 / 1048576.0
    );
}

fn main() {
    let a = harness::args();
    let mpath = harness::pathv(&a, "metrics", "artifacts/p2.jsonl");
    harness::init_rec(&mpath);
    let mut rec = Recorder::new(&harness::abs(&mpath));
    let opt = Opt {
        cp: if harness::strv(&a, "cp", "uni") == "gbk" { Cp::Gbk } else { Cp::Uni },
        font_pt: harness::num(&a, "font", 11.0) as usize,
        dark: harness::flag(&a, "dark"),
        page_twips: harness::num(&a, "page", 9026.0) as i32,
        heading_anchor: harness::flag(&a, "anchor"),
    };
    let render = !harness::flag(&a, "no-render");
    let max_prev = harness::num(&a, "max-preview", 12.0) as usize;

    unsafe {
        let _ = w::enable_dpi_awareness();
        let hi = w::hinstance();
        rec.log_deep("entry", &[("cp", format!("{:?}", opt.cp)), ("render", render.to_string())]);
        w::register_class(&w::ClassDesc { name: CLS, proc: wndproc, bg: COLORREF(0x00FFFFFF) }, hi).unwrap();
        HOST = w::create_window(CLS, "wild-markdown P2 RichEdit 预览原型", wm::WS_OVERLAPPEDWINDOW, 1100, 820, None, hi, std::ptr::null_mut()).unwrap();
        let _ = wm::ShowWindow(HOST, if render { wm::SW_SHOWNORMAL } else { wm::SW_HIDE });
        let _ = richedit::load_msftedit().expect("Msftedit.dll 载入失败（RichEdit 4.1 不可用？）");
        let (cw, ch) = w::client_size(HOST);
        RICH = richedit::create(HOST, 0, 0, cw, ch, 2001, true).expect("RichEdit 创建失败");
        richedit::wrap_to_window(RICH);
        richedit::set_event_mask(RICH, richedit::EV_DEFAULT);
        let s = rec.log_deep("richedit_created", &[]);
        println!("{}", wild_mark::metrics::line("richedit_created(empty)", &s, 0.0));

        // 具体用例（功能矩阵/病态）
        for f in [
            "testdata/small/feature_matrix.md",
            "testdata/pathological/rtf_escape.md",
            "testdata/pathological/single_huge_line.md",
        ] {
            one(&mut rec, f, &opt, render, "case");
        }

        if let Some(p) = harness::pos(&a, 0) {
            one(&mut rec, &p, &opt, render, "file");
        }

        // 多尺寸基准
        let sizes: Vec<usize> = harness::strv(&a, "bench-sizes", "").split(',').filter_map(|s| s.trim().parse().ok()).collect();
        for mb in sizes {
            let p = format!("testdata/large/md_{mb}MiB_norm.md");
            if mb > max_prev {
                // 大文件模式：只测解析+RTF，不灌控件（plan.md：达到阈值后默认关闭实时预览）
                one(&mut rec, &p, &opt, false, "oversize_no_render");
            } else {
                one(&mut rec, &p, &opt, render, "render");
            }
        }

        // \u 与 GBK 编码对照
        if harness::flag(&a, "compare-cp") {
            let p = harness::pos(&a, 0).unwrap_or_else(|| "testdata/large/md_10MiB_norm.md".to_string());
            one(&mut rec, &p, &Opt { cp: Cp::Uni, ..opt.clone() }, false, "cp_uni");
            one(&mut rec, &p, &Opt { cp: Cp::Gbk, ..opt.clone() }, false, "cp_gbk");
            one(&mut rec, &p, &Opt { cp: Cp::Gbk, ..opt.clone() }, render, "cp_gbk_render");
        }

        let n = harness::num(&a, "stress", 0.0) as usize;
        if n > 0 {
            let p = harness::pos(&a, 0).unwrap_or_else(|| "testdata/large/md_1MiB_norm.md".to_string());
            stress(&mut rec, &p, &opt, n);
        }

        // 目视验收截图
        if harness::flag(&a, "shot") {
            let _ = gdi::UpdateWindow(RICH);
            let _ = w::capture_bmp(RICH, &harness::abs(&harness::pathv(&a, "shot", "artifacts/p2.bmp")));
        }
        harness::quiet(harness::num(&a, "idle", 2.0), 1.0);
        let s = rec.log_deep("final", &[]);
        println!("{}", wild_mark::metrics::line("final", &s, 0.0));
        rec.flush();
        let mut sum = harness::summarize(&harness::abs(&mpath), &harness::abs("target/release/p2_preview.exe"));
        sum.insert("cp".into(), format!("{:?}", opt.cp));
        sum.insert("render".into(), render.to_string());
        harness::write_json(&sum, &harness::abs("artifacts/p2.summary.json"));
        harness::print_map(&sum);
        let _ = wm::DestroyWindow(HOST);
        wm::PostQuitMessage(0);
    }
}

#[allow(dead_code)]
const _RECT_UNUSED: Option<RECT> = None;
