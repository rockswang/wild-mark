//! Win32 通用封装：DPI、窗口类、消息循环、菜单、截图。
//! 原型只验证资源占用与原生控件可用性，因此不引入任何 GUI 框架。

use std::ffi::c_void;
use windows::core::{PCWSTR, Result};
use windows::Win32::Foundation::{COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging as wm;

pub type WndProc = unsafe extern "system" fn(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;

/// &str -> 0 结尾 UTF-16，供 PCWSTR 借用
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn pcw(v: &[u16]) -> PCWSTR {
    PCWSTR(v.as_ptr())
}

/// PM-v2 DPI 感知；旧系统失败则退回系统级感知
pub unsafe fn enable_dpi_awareness() -> &'static str {
    if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() {
        "per-monitor-v2"
    } else {
        let _ = wm::SetProcessDPIAware();
        "system-aware"
    }
}

pub struct ClassDesc<'a> {
    pub name: &'a str,
    pub proc: WndProc,
    pub bg: COLORREF,
}

/// 注册窗口类（CS_HREDRAW|CS_VREDRAW|CS_DBLCLKS）
pub unsafe fn register_class(desc: &ClassDesc, hinst: HINSTANCE) -> Result<u16> {
    let cls = wide(desc.name);
    let wc = wm::WNDCLASSEXW {
        cbSize: std::mem::size_of::<wm::WNDCLASSEXW>() as u32,
        style: wm::CS_HREDRAW | wm::CS_VREDRAW | wm::CS_DBLCLKS,
        lpfnWndProc: Some(desc.proc),
        hInstance: hinst,
        hCursor: wm::LoadCursorW(None, wm::IDC_ARROW)?,
        hbrBackground: CreateSolidBrush(desc.bg),
        lpszClassName: pcw(&cls),
        ..Default::default()
    };
    let atom = wm::RegisterClassExW(&wc);
    if atom == 0 {
        return Err(windows::core::Error::from_win32().into());
    }
    Ok(atom)
}

/// 创建顶层窗口；param 透传到 WM_NCCREATE 的 CREATESTRUCT.lpCreateParams
pub unsafe fn create_window(
    class: &str,
    title: &str,
    style: wm::WINDOW_STYLE,
    w: i32,
    h: i32,
    menu: Option<wm::HMENU>,
    hinst: HINSTANCE,
    param: *mut c_void,
) -> Result<HWND> {
    let (c, t) = (wide(class), wide(title));
    wm::CreateWindowExW(
        wm::WINDOW_EX_STYLE(0),
        pcw(&c),
        pcw(&t),
        style,
        wm::CW_USEDEFAULT,
        wm::CW_USEDEFAULT,
        w,
        h,
        HWND::default(),
        menu.unwrap_or_default(),
        hinst,
        Some(param),
    )
}

/// 标准阻塞消息循环：空闲时 CPU≈0（不跑定时器、不自绘循环）
pub unsafe fn pump_until_quit() {
    let mut m = wm::MSG::default();
    while wm::GetMessageW(&mut m, None, 0, 0).as_bool() {
        let _ = wm::TranslateMessage(&m);
        wm::DispatchMessageW(&m);
    }
}

pub unsafe fn def(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    wm::DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 构建多级菜单条：[("文件", [("打开", 101)])]；MF_STRING=0x0000, MF_POPUP=0x0010
pub unsafe fn build_menu(items: &[(&str, &[(&str, usize)])]) -> Result<wm::HMENU> {
    let bar = wm::CreateMenu()?;
    for (title, subs) in items {
        let top = wide(title);
        let sub = wm::CreatePopupMenu()?;
        for (label, id) in *subs {
            let lb = wide(label);
            wm::AppendMenuW(sub, wm::MENU_ITEM_FLAGS(0x0000), *id, pcw(&lb))?;
        }
        wm::AppendMenuW(bar, wm::MF_POPUP, sub.0 as usize, pcw(&top))?;
    }
    Ok(bar)
}

pub unsafe fn client_rect(hwnd: HWND) -> RECT {
    let mut r = RECT::default();
    let _ = wm::GetClientRect(hwnd, &mut r);
    r
}

pub unsafe fn client_size(hwnd: HWND) -> (i32, i32) {
    let r = client_rect(hwnd);
    (r.right - r.left, r.bottom - r.top)
}

pub unsafe fn set_userdata(hwnd: HWND, ptr: *mut c_void) {
    let _ = wm::SetWindowLongPtrW(hwnd, wm::GWLP_USERDATA, ptr as isize);
}

/// 取回 GWLP_USERDATA 中的 &'static mut T；每个窗口的状态独占，故 unsafe 合理
pub unsafe fn userdata<T>(hwnd: HWND) -> Option<&'static mut T> {
    let p = wm::GetWindowLongPtrW(hwnd, wm::GWLP_USERDATA) as *mut T;
    (!p.is_null()).then_some(&mut *p)
}

/// 从 CREATESTRUCT 取 lpCreateParams
pub unsafe fn create_param(lparam: LPARAM) -> *mut c_void {
    let cs = &*(lparam.0 as *const wm::CREATESTRUCTW);
    cs.lpCreateParams
}

/// 抓客户区为 24bpp BMP，供中文排版/表格渲染的目视验收
pub unsafe fn capture_bmp(hwnd: HWND, path: &str) -> Result<()> {
    use std::io::Write;
    let (w, h) = client_size(hwnd);
    if w <= 0 || h <= 0 {
        return Err(windows::core::Error::new(windows::Win32::Foundation::E_INVALIDARG, "窗口尺寸非法").into());
    }
    let hdc = GetDC(hwnd);
    let mem = CreateCompatibleDC(hdc);
    let bmp = CreateCompatibleBitmap(hdc, w, h);
    let _old = SelectObject(mem, bmp);
    let _ = BitBlt(mem, 0, 0, w, h, hdc, 0, 0, SRCCOPY); // BitBlt 客户区，控件已合成
    let row = ((w * 3 + 3) / 4 * 4) as u32; // 24bpp 扫描行需 4 字节对齐
    let mut px = vec![0u8; (row as usize * h as usize) + 4];
    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // 负值=自顶向下扫描
            biPlanes: 1,
            biBitCount: 24,
            biCompression: 0,
            biSizeImage: row * h as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    let ok = GetDIBits(hdc, bmp, 0, h as u32, Some(px.as_mut_ptr() as *mut c_void), &mut bi, DIB_RGB_COLORS);
    let _ = DeleteDC(mem);
    let _ = DeleteObject(bmp);
    let _ = ReleaseDC(hwnd, hdc);
    if ok == 0 {
        return Err(windows::core::Error::from_win32().into());
    }
    let size = 54 + row as usize * h as usize;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut hdr = [0u8; 54];
    hdr[0..2].copy_from_slice(b"BM");
    hdr[2..6].copy_from_slice(&(size as u32).to_le_bytes());
    hdr[10..14].copy_from_slice(&54u32.to_le_bytes());
    hdr[14..18].copy_from_slice(&40u32.to_le_bytes());
    hdr[18..22].copy_from_slice(&w.to_le_bytes());
    hdr[22..26].copy_from_slice(&h.to_le_bytes());
    hdr[26..28].copy_from_slice(&1u16.to_le_bytes());
    hdr[28..30].copy_from_slice(&24u16.to_le_bytes());
    hdr[32..36].copy_from_slice(&row.to_le_bytes());
    f.write_all(&hdr)?;
    f.write_all(&px[..row as usize * h as usize])?;
    Ok(())
}

/// 枚举本进程可见顶层窗口标题（外部脚本据此定位窗口）
pub fn top_titles(pid: u32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let state = (pid, &mut out as *mut Vec<String>);
    unsafe {
        let _ = wm::EnumWindows(Some(enum_cb), LPARAM(&state as *const _ as isize));
    }
    out
}

unsafe extern "system" fn enum_cb(h: HWND, l: LPARAM) -> windows::Win32::Foundation::BOOL {
    let st = &*(l.0 as *const (u32, *mut Vec<String>));
    let mut pid = 0u32;
    let _ = wm::GetWindowThreadProcessId(h, Some(&mut pid));
    if pid == st.0 && wm::IsWindowVisible(h).as_bool() {
        let mut buf = vec![0u16; 256];
        let n = wm::GetWindowTextW(h, &mut buf) as usize;
        (*st.1).push(String::from_utf16_lossy(&buf[..n]));
    }
    true.into()
}

pub fn hinstance() -> HINSTANCE {
    // GetModuleHandleW 返 HMODULE，两者都是 *mut c_void 包装，可直接换型
    HINSTANCE(unsafe { GetModuleHandleW(PCWSTR::null()) }.unwrap_or_default().0)
}

pub fn cur_process() -> HANDLE {
    unsafe { GetCurrentProcess() }
}
