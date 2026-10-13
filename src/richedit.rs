//! RichEdit 4.1（Msftedit.dll / RICHEDIT50W）封装：RTF 流式载入、滚动、选区、复制。
//! 预览管线末端：只接收 RTF 字节，不做 Markdown 语义。

use windows::core::{PCWSTR, Result};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
use windows::Win32::System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Controls::{EM_LINESCROLL, EM_SETSEL};
use windows::Win32::UI::Controls::RichEdit::{
    EDITSTREAM, EM_EXGETSEL, EM_EXSETSEL, EM_GETSCROLLPOS, EM_HIDESELECTION, EM_SETEVENTMASK, EM_SETSCROLLPOS, EM_SETTARGETDEVICE,
    EM_STREAMIN, ENM_LINK, ENM_SELCHANGE, EN_REQUESTRESIZE, SF_RTF, SF_TEXT,
};
use windows::Win32::UI::WindowsAndMessaging as wm;

use crate::win::{hinstance, pcw, wide};

/// EM_STREAMIN 回调上下文：只读游标，回调内不分配内存
pub struct Cursor {
    pub data: *const u8,
    pub len: usize,
    pub pos: usize,
}

unsafe extern "system" fn stream_cb(cookie: usize, buf: *mut u8, cb: i32, pcb: *mut i32) -> u32 {
    let c = &mut *(cookie as *mut Cursor);
    let n = (c.len - c.pos).min(cb.max(0) as usize);
    std::ptr::copy_nonoverlapping(c.data.add(c.pos), buf, n);
    c.pos += n;
    *pcb = n as i32;
    0 // 0=继续/无错误；返回 0 且 *pcb=0 表示流结束
}

#[inline]
pub unsafe fn send(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    wm::SendMessageW(hwnd, msg, WPARAM(wp), LPARAM(lp)).0
}

/// RichEdit 4.1 必须先载入 Msftedit.dll 才有 RICHEDIT50W 窗口类
pub unsafe fn load_msftedit() -> Result<HMODULE> {
    let p = wide("Msftedit.dll");
    LoadLibraryExW(PCWSTR(p.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
}

/// 创建 RichEdit 子窗口；readonly=true 用于预览
pub unsafe fn create(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: usize, readonly: bool) -> Result<HWND> {
    let cls = wide("RICHEDIT50W");
    let mut st = wm::WS_CHILD | wm::WS_VISIBLE | wm::WS_CLIPSIBLINGS | wm::WS_VSCROLL | wm::WS_HSCROLL;
    st |= wm::WINDOW_STYLE((wm::ES_MULTILINE | wm::ES_AUTOVSCROLL | wm::ES_NOHIDESEL) as u32);
    if readonly {
        st |= wm::WINDOW_STYLE(wm::ES_READONLY as u32);
    }
    wm::CreateWindowExW(
        wm::WINDOW_EX_STYLE(0),
        pcw(&cls),
        PCWSTR::null(),
        st,
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

/// 把 RTF 字节灌入控件（必须与控件同线程调用）。返回已消费字节数。
pub unsafe fn set_rtf(hwnd: HWND, rtf: &[u8]) -> Result<usize> {
    let mut cur = Cursor { data: rtf.as_ptr(), len: rtf.len(), pos: 0 };
    let es = EDITSTREAM { dwCookie: &mut cur as *mut Cursor as usize, dwError: 0, pfnCallback: Some(stream_cb) };
    let consumed = send(hwnd, EM_STREAMIN, SF_RTF as usize, &es as *const EDITSTREAM as isize) as usize;
    if es.dwError != 0 {
        return Err(windows::core::Error::from_win32().into());
    }
    Ok(consumed)
}

/// 纯文本流式载入（对照 RTF 用，也用于超大文档降级预览）
pub unsafe fn set_text(hwnd: HWND, text: &[u8]) -> usize {
    let mut cur = Cursor { data: text.as_ptr(), len: text.len(), pos: 0 };
    let es = EDITSTREAM { dwCookie: &mut cur as *mut Cursor as usize, dwError: 0, pfnCallback: Some(stream_cb) };
    send(hwnd, EM_STREAMIN, SF_TEXT as usize, &es as *const EDITSTREAM as isize) as usize
}

pub unsafe fn text_len(hwnd: HWND) -> usize {
    wm::GetWindowTextLengthW(hwnd) as usize
}

pub unsafe fn get_text(hwnd: HWND) -> String {
    let n = text_len(hwnd);
    let mut buf = vec![0u16; n + 2];
    let got = wm::GetWindowTextW(hwnd, &mut buf) as usize;
    String::from_utf16_lossy(&buf[..got])
}

pub unsafe fn scroll_pos(hwnd: HWND) -> (i32, i32) {
    let mut p = POINT::default();
    send(hwnd, EM_GETSCROLLPOS, 0, &mut p as *mut POINT as isize);
    (p.x, p.y)
}

pub unsafe fn set_scroll_pos(hwnd: HWND, x: i32, y: i32) {
    let p = POINT { x, y };
    send(hwnd, EM_SETSCROLLPOS, 0, &p as *const POINT as isize);
}

/// 按行滚动（编辑区→预览区同步滚动的兜底手段）
pub unsafe fn lines_scroll(hwnd: HWND, dy: i32) {
    let _ = send(hwnd, EM_LINESCROLL, 0, dy as isize);
}

/// target device = NULL：按控件宽度自动换行
pub unsafe fn wrap_to_window(hwnd: HWND) {
    let _ = send(hwnd, EM_SETTARGETDEVICE, 0, 0);
}

pub unsafe fn hide_selection(hwnd: HWND, hide: bool) {
    let _ = send(hwnd, EM_HIDESELECTION, hide as usize, 0);
}

pub unsafe fn select_all(hwnd: HWND) {
    let _ = send(hwnd, EM_SETSEL, 0, -1isize);
}

/// 只读 RichEdit 仍响应 WM_COPY：选区文本进剪贴板（验收“预览可复制”）
pub unsafe fn copy_text(hwnd: HWND) {
    let _ = send(hwnd, wm::WM_COPY, 0, 0);
}

pub unsafe fn clipboard_text() -> String {
    if OpenClipboard(HWND::default()).is_err() {
        return String::new();
    }
    let mut s = String::new();
    unsafe {
        if let Ok(h) = GetClipboardData(CF_UNICODETEXT.0 as u32) {
            let p: *const u16 = h.0 as *const u16;
            if !p.is_null() {
                let mut n = 0usize;
                while *p.add(n) != 0 {
                    n += 1;
                }
                s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            }
        }
        let _ = CloseClipboard();
    }
    s
}

/// 事件掩码：EN_SELCHANGED（同步滚动）、EN_LINK（超链接）、EN_REQUESTRESIZE
pub unsafe fn set_event_mask(hwnd: HWND, mask: u32) {
    let _ = send(hwnd, EM_SETEVENTMASK, mask as usize, 0);
}

pub const EV_DEFAULT: u32 = ENM_SELCHANGE | ENM_LINK | EN_REQUESTRESIZE;

/// 光标/选区（CHARRANGE）：验证整篇刷新后位置是否保持
pub unsafe fn get_sel(hwnd: HWND) -> (i32, i32) {
    #[repr(C)]
    struct CharRange {
        cp_min: i32,
        cp_max: i32,
    }
    let mut cr = CharRange { cp_min: 0, cp_max: 0 };
    send(hwnd, EM_EXGETSEL, 0, &mut cr as *mut CharRange as isize);
    (cr.cp_min, cr.cp_max)
}

pub unsafe fn set_sel(hwnd: HWND, min: i32, max: i32) {
    #[repr(C)]
    struct CharRange {
        cp_min: i32,
        cp_max: i32,
    }
    let cr = CharRange { cp_min: min, cp_max: max };
    send(hwnd, EM_EXSETSEL, 0, &cr as *const CharRange as isize);
}
