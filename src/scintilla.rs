//! Scintilla 薄 FFI 层：LoadLibrary + CreateWindow + SendMessageW。
//! 64 位下不用 ScintillaDirect2Call（那是 32 位专用），统一走 SendMessageW。
//! 消息号来自 scripts/gen_sci_const.py 生成的 sci_const.rs（Scintilla.h 5.15 / SciLexer.h 5.30）。

include!("sci_const.rs");

use windows::core::{PCSTR, PCWSTR, Result};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH};
use windows::Win32::UI::WindowsAndMessaging as wm;

use crate::win::{hinstance, pcw, wide};

// SC_FIND_* / SC_IDLESTYLING_* / SC_PERFORMED_* 在 5.15 里是 C++ enum，不在 Scintilla.h 宏中，按头文件值补齐
pub const SC_MOD_AFTERTEXT: usize = 0x00200000;
pub const SC_PERFORMED_USER: usize = 0x10;
pub const SC_PERFORMED_UNDO: usize = 0x20;
pub const SC_PERFORMED_REDO: usize = 0x40;
pub const SC_FIND_NONE: usize = 0x0;
pub const SC_FIND_WHOLEWORD: usize = 0x2;
pub const SC_FIND_MATCHCASE: usize = 0x4;
pub const SC_FIND_WORDSTART: usize = 0x00100000;
pub const SC_FIND_REGEXP: usize = 0x00200000;
pub const SC_IDLESTYLING_NONE: usize = 0;
pub const SC_IDLESTYLING_AFTERVISIBLE: usize = 1;
pub const SC_IDLESTYLING_ALL: usize = 2;

#[allow(dead_code)]
pub unsafe fn send(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    wm::SendMessageW(hwnd, msg, WPARAM(wp), LPARAM(lp)).0
}

/// 载入 Scintilla.dll：其 DllMain 已注册窗口类 "Scintilla"
pub unsafe fn load_dll(path: &str) -> Result<HMODULE> {
    let p = wide(path);
    LoadLibraryExW(PCWSTR(p.as_ptr()), None, LOAD_WITH_ALTERED_SEARCH_PATH)
}

/// 创建 Scintilla 子窗口；id 供父窗口 WM_COMMAND/WM_NOTIFY 路由
pub unsafe fn create(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: usize) -> Result<HWND> {
    let cls = wide("Scintilla");
    wm::CreateWindowExW(
        wm::WINDOW_EX_STYLE(0),
        pcw(&cls),
        PCWSTR::null(),
        wm::WS_CHILD | wm::WS_VISIBLE | wm::WS_CLIPSIBLINGS,
        x,
        y,
        w,
        h,
        parent,
        wm::HMENU(id as *mut core::ffi::c_void),
        hinstance(),
        None,
    )
}

pub unsafe fn status(sci: HWND) -> usize {
    send(sci, SCI_GETSTATUS, 0, 0) as usize
}

/// 整篇替换文本（UTF-8 字节，NUL 结尾）
pub unsafe fn set_text(sci: HWND, bytes: &[u8]) {
    let mut buf = bytes.to_vec();
    buf.push(0);
    send(sci, SCI_SETTEXT, 0, buf.as_ptr() as isize);
}

/// 预分配缓冲区，避免大文件反复扩容（SCI_ALLOCATE 参数是字节数）
pub unsafe fn allocate(sci: HWND, bytes: usize) {
    send(sci, SCI_ALLOCATE, bytes, 0);
}

pub unsafe fn append_text(sci: HWND, s: &str) {
    let mut buf = s.as_bytes().to_vec();
    buf.push(0);
    send(sci, SCI_APPENDTEXT, buf.len() - 1, buf.as_ptr() as isize);
}

pub unsafe fn insert_text(sci: HWND, pos: usize, s: &str) {
    let mut buf = s.as_bytes().to_vec();
    buf.push(0);
    send(sci, SCI_INSERTTEXT, pos, buf.as_ptr() as isize);
}

pub unsafe fn replace_sel(sci: HWND, s: &str) {
    let mut buf = s.as_bytes().to_vec();
    buf.push(0);
    send(sci, SCI_REPLACESEL, 0, buf.as_ptr() as isize);
}

pub unsafe fn text_len(sci: HWND) -> usize {
    send(sci, SCI_GETTEXTLENGTH, 0, 0) as usize
}

pub unsafe fn get_text(sci: HWND, n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n + 1];
    send(sci, SCI_GETTEXT, n, buf.as_mut_ptr() as isize);
    buf.truncate(n);
    buf
}

pub unsafe fn line_count(sci: HWND) -> usize {
    send(sci, SCI_GETLINECOUNT, 0, 0) as usize
}

pub unsafe fn sel_text(sci: HWND) -> String {
    let n = send(sci, SCI_GETSELTEXT, 0, 0) as usize;
    if n == 0 {
        return String::new();
    }
    let mut buf = vec![0u8; n + 1];
    send(sci, SCI_GETSELTEXT, 0, buf.as_mut_ptr() as isize);
    buf.truncate(n);
    String::from_utf8_lossy(&buf).into_owned()
}

/// COLORREF 为 0x00BBGGRR
pub fn rgb(r: u8, g: u8, b: u8) -> isize {
    (r as isize) | (g as isize) << 8 | (b as isize) << 16
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LexerMode {
    None,     // SCI_SETILEXER(0,nullptr)：全部默认样式
    Container, // 容器自染：SCN_STYLENEEDED + SCI_SETSTYLING
    Lexilla,  // lexilla.dll 的 markdown lexer
}

#[derive(Clone)]
pub struct SciOpts {
    pub wrap: bool,
    pub line_numbers: bool,
    pub font_pt: usize,
    pub lexer: LexerMode,
    pub caret_line: bool,
    pub mod_mask: usize,
}

impl Default for SciOpts {
    fn default() -> Self {
        Self { wrap: false, line_numbers: true, font_pt: 11, lexer: LexerMode::Lexilla, caret_line: false, mod_mask: SC_MODEVENTMASKALL }
    }
}

/// lexilla.dll 的 CreateLexer(name) -> ILexer5*，交给 SCI_SETILEXER
/// 注意：Scintilla 5.x 里 Lexilla 是唯一 lexer 来源（核心不再内置 Catalogue）
pub unsafe fn create_lexlexer(dll: &str, name: &str) -> Result<*mut core::ffi::c_void> {
    let h = load_dll(dll)?;
    let fname = b"CreateLexer\0";
    let proc = GetProcAddress(h, PCSTR(fname.as_ptr()));
    if proc.is_none() {
        return Err(windows::core::Error::from_win32().into());
    }
    // FARPROC = Option<unsafe extern "system" fn() -> isize>，宽度与目标函数指针一致
    type CreateLexer = unsafe extern "system" fn(*const u8) -> *mut core::ffi::c_void;
    let f: CreateLexer = unsafe { std::mem::transmute_copy(&proc) };
    let mut cname: Vec<u8> = name.as_bytes().to_vec();
    cname.push(0);
    let p = f(cname.as_ptr());
    if p.is_null() {
        return Err(windows::core::Error::from_win32().into());
    }
    Ok(p)
}

/// 一次性完成外观/行为配置；face 名称需 NUL 结尾且在 SendMessage 期间保持存活
pub unsafe fn configure(sci: HWND, o: &SciOpts, lexilla_dll: Option<&str>) -> Result<String> {
    send(sci, SCI_SETCODEPAGE, SC_CP_UTF8 as usize, 0);
    send(sci, SCI_SETEOLMODE, 2, 0); // SC_EOL_LF：与 Markdown 源保持一致
    if o.line_numbers {
        send(sci, SCI_SETMARGINTYPEN, 1, SC_MARGIN_NUMBER as isize);
        send(sci, SCI_SETMARGINWIDTHN, 1, 52);
        send(sci, SCI_SETMARGINSENSITIVEN, 1, 1);
    } else {
        send(sci, SCI_SETMARGINWIDTHN, 1, 0);
    }
    send(sci, SCI_SETWRAPMODE, if o.wrap { SC_WRAP_WORD } else { SC_WRAP_NONE } as usize, 0);
    send(sci, SCI_SETCARETLINEVISIBLE, o.caret_line as usize, 0);
    send(sci, SCI_SETTABWIDTH, 4, 0);
    send(sci, SCI_SETINDENT, 4, 0);
    send(sci, SCI_SETIDLESTYLING, SC_IDLESTYLING_AFTERVISIBLE, 0); // 大文件只给可见区染色
    let face = cname("Microsoft YaHei Mono,微软雅黑,Consolas");
    let mono = cname("Cascadia Mono,Consolas,Courier New");
    let (fptr, mptr) = (face.0, mono.0);
    for st in 0..=SCE_MARKDOWN_CODEBK as usize {
        send(sci, SCI_STYLESETFONT, st, if st == SCE_MARKDOWN_CODE as usize || st == SCE_MARKDOWN_CODE2 as usize || st == SCE_MARKDOWN_CODEBK as usize { mptr } else { fptr });
        send(sci, SCI_STYLESETSIZE, st, (o.font_pt * 2) as isize);
        send(sci, SCI_STYLESETCHARACTERSET, st, 134); // GB2312_CHARSET：中文与全角标点回退
    }
    std::mem::forget(face);
    std::mem::forget(mono);
    send(sci, SCI_STYLESETFORE, 0, rgb(0x20, 0x20, 0x20));
    send(sci, SCI_STYLESETBACK, 0, rgb(0xff, 0xff, 0xff));
    let styles: &[(usize, (u8, u8, u8), bool, bool)] = &[
        (SCE_MARKDOWN_STRONG1 as usize, (0x00, 0x00, 0x00), true, false),
        (SCE_MARKDOWN_STRONG2 as usize, (0x00, 0x00, 0x00), true, false),
        (SCE_MARKDOWN_EM1 as usize, (0x30, 0x30, 0x30), false, true),
        (SCE_MARKDOWN_EM2 as usize, (0x30, 0x30, 0x30), false, true),
        (SCE_MARKDOWN_HEADER1 as usize, (0x0b, 0x5c, 0xa8), true, false),
        (SCE_MARKDOWN_HEADER2 as usize, (0x0b, 0x5c, 0xa8), true, false),
        (SCE_MARKDOWN_HEADER3 as usize, (0x0b, 0x5c, 0xa8), true, false),
        (SCE_MARKDOWN_PRECHAR as usize, (0x9a, 0x00, 0x9a), false, false),
        (SCE_MARKDOWN_ULIST_ITEM as usize, (0x00, 0x80, 0x00), false, false),
        (SCE_MARKDOWN_OLIST_ITEM as usize, (0x00, 0x80, 0x00), false, false),
        (SCE_MARKDOWN_BLOCKQUOTE as usize, (0x60, 0x60, 0x60), false, true),
        (SCE_MARKDOWN_STRIKEOUT as usize, (0x80, 0x80, 0x80), false, false),
        (SCE_MARKDOWN_HRULE as usize, (0x99, 0x99, 0x99), false, false),
        (SCE_MARKDOWN_LINK as usize, (0x00, 0x60, 0xc0), false, false),
        (SCE_MARKDOWN_CODE as usize, (0x8b, 0x00, 0x8b), false, false),
        (SCE_MARKDOWN_CODE2 as usize, (0x8b, 0x00, 0x8b), false, false),
        (SCE_MARKDOWN_CODEBK as usize, (0x50, 0x50, 0x50), false, false),
    ];
    for (st, (r, g, b), bold, italic) in styles {
        send(sci, SCI_STYLESETFORE, *st, rgb(*r, *g, *b));
        send(sci, SCI_STYLESETBOLD, *st, *bold as isize);
        send(sci, SCI_STYLESETITALIC, *st, *italic as isize);
    }
    let note = match o.lexer {
        LexerMode::None => {
            send(sci, SCI_SETILEXER, 0, 0);
            "none".to_string()
        }
        LexerMode::Container => {
            send(sci, SCI_SETILEXER, 0, 0);
            send(sci, SCI_SETMODEVENTMASK, SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT, 0);
            "container".to_string()
        }
        LexerMode::Lexilla => match lexilla_dll {
            Some(dll) => match create_lexlexer(dll, "markdown") {
                Ok(p) => {
                    send(sci, SCI_SETILEXER, 0, p as isize);
                    format!("lexilla/markdown 模块数={}", send(sci, SCI_GETLEXER, 0, 0))
                }
                Err(e) => {
                    send(sci, SCI_SETILEXER, 0, 0);
                    format!("lexilla 失败({e})→container")
                }
            },
            None => {
                send(sci, SCI_SETILEXER, 0, 0);
                "no-lexilla-dll→container".to_string()
            }
        },
    };
    if o.mod_mask != 0 {
        send(sci, SCI_SETMODEVENTMASK, o.mod_mask, 0);
    }
    send(sci, SCI_EMPTYUNDOBUFFER, 0, 0);
    Ok(note)
}

/// 返回 (指针, 保活缓冲)；调用方用 forget 延长生命周期到进程结束
fn cname(s: &str) -> (isize, Vec<u8>) {
    let mut v: Vec<u8> = s.as_bytes().to_vec();
    v.push(0);
    (v.as_ptr() as isize, v)
}

/// 容器自染：整篇按行扫描做极简 Markdown 高亮（无 Lexilla 时的兜底路径）
pub unsafe fn style_container(sci: HWND, start: usize) {
    let end = text_len(sci);
    send(sci, SCI_STARTSTYLING, start, 0);
    let mut pos = start;
    while pos < end {
        let line = send(sci, SCI_LINEFROMPOSITION, pos, 0) as usize;
        let lstart = send(sci, SCI_POSITIONFROMLINE, line, 0) as usize;
        let lend = (lstart + send(sci, SCI_LINELENGTH, line, 0) as usize).min(end);
        let mut buf = vec![0u8; lend - lstart + 1];
        send(sci, SCI_GETTEXT, lend - lstart, buf.as_mut_ptr() as isize);
        let head = String::from_utf8_lossy(&buf[..lend - lstart]);
        let st = if head.starts_with('#') {
            SCE_MARKDOWN_HEADER1 as usize
        } else if head.starts_with("```") {
            SCE_MARKDOWN_CODEBK as usize
        } else if head.starts_with("> ") {
            SCE_MARKDOWN_BLOCKQUOTE as usize
        } else if head.starts_with("- ") || head.starts_with("* ") {
            SCE_MARKDOWN_ULIST_ITEM as usize
        } else {
            SCE_MARKDOWN_DEFAULT as usize
        };
        send(sci, SCI_SETSTYLING, lend - pos, st as isize);
        pos = (lend + 1).min(end);
    }
}

pub unsafe fn goto_line(sci: HWND, line: usize) {
    send(sci, SCI_GOTOLINE, line, 0);
    send(sci, SCI_SCROLLCARET, 0, 0);
}

/// 查找：返回 (起始, 结束)，未命中返回 None
pub unsafe fn find_text(sci: HWND, needle: &str, from: usize, to: usize, flags: usize) -> Option<(usize, usize)> {
    #[repr(C)]
    struct ChRange {
        cp_min: i32,
        cp_max: i32,
    }
    #[repr(C)]
    struct TextToFind<'a> {
        chr: ChRange,
        lpstrtext: *const u8,
        flags: usize,
        _phdr: std::marker::PhantomData<&'a ()>,
    }
    let mut n: Vec<u8> = needle.as_bytes().to_vec();
    n.push(0);
    let ttf = TextToFind { chr: ChRange { cp_min: from as i32, cp_max: to as i32 }, lpstrtext: n.as_ptr(), flags, _phdr: std::marker::PhantomData };
    let r = send(sci, SCI_FINDTEXT, flags, &ttf as *const TextToFind as isize);
    if r >= 0 {
        Some((r as usize, ttf.chr.cp_max as usize))
    } else {
        None
    }
}

pub unsafe fn colourise_all(sci: HWND) -> isize {
    send(sci, SCI_COLOURISE, 0, -1)
}
