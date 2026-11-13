//! Owner-drawn dark context menus (same colours as DockX 1.1's menus).

use super::util::*;
use std::cell::RefCell;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub enum M {
    Item { id: u32, text: String, checked: bool, enabled: bool },
    Sep,
    Sub { text: String, items: Vec<M> },
}

pub fn item(id: u32, text: &str) -> M {
    M::Item { id, text: text.into(), checked: false, enabled: true }
}
pub fn check(id: u32, text: &str, on: bool) -> M {
    M::Item { id, text: text.into(), checked: on, enabled: true }
}
pub fn sub(text: &str, items: Vec<M>) -> M {
    M::Sub { text: text.into(), items }
}

struct Data {
    text: Vec<u16>,
    checked: bool,
    enabled: bool,
    sep: bool,
    sub: bool,
}

thread_local! {
    static DATA: RefCell<Vec<Box<Data>>> = RefCell::new(Vec::new());
    static FONT: RefCell<(HFONT, f32)> = RefCell::new((HFONT::default(), 0.0));
    static SCALE: RefCell<f32> = RefCell::new(1.0);
}

/// Win10 1903+: dark menu frames (undocumented uxtheme ordinals, safe to skip if missing).
pub fn enable_dark_mode() {
    unsafe {
        if let Ok(ux) = LoadLibraryW(&h("uxtheme.dll")) {
            if let Some(f) = GetProcAddress(ux, PCSTR(135 as *const u8)) {
                let set: extern "system" fn(i32) -> i32 = std::mem::transmute(f);
                set(2); // ForceDark
            }
            if let Some(f) = GetProcAddress(ux, PCSTR(136 as *const u8)) {
                let flush: extern "system" fn() = std::mem::transmute(f);
                flush();
            }
        }
    }
}

fn font(s: f32) -> HFONT {
    FONT.with(|f| {
        let mut f = f.borrow_mut();
        if f.1 != s || f.0.is_invalid() {
            unsafe {
                if !f.0.is_invalid() {
                    let _ = DeleteObject(f.0);
                }
                f.0 = CreateFontW((-13.0 * s) as i32, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, &h("Segoe UI"));
                f.1 = s;
            }
        }
        f.0
    })
}

unsafe fn build(items: &[M], brush: HBRUSH) -> HMENU {
    let m = CreatePopupMenu().unwrap_or_default();
    let mi = MENUINFO { cbSize: std::mem::size_of::<MENUINFO>() as u32, fMask: MIM_BACKGROUND, hbrBack: brush, ..Default::default() };
    let _ = SetMenuInfo(m, &mi);
    for it in items {
        let (flags, id, data) = match it {
            M::Item { id, text, checked, enabled } => (
                MF_OWNERDRAW | if *enabled { MF_ENABLED } else { MF_GRAYED },
                *id as usize,
                Data { text: wide(text), checked: *checked, enabled: *enabled, sep: false, sub: false },
            ),
            M::Sep => (MF_OWNERDRAW | MF_DISABLED, 0usize, Data { text: vec![0], checked: false, enabled: false, sep: true, sub: false }),
            M::Sub { text, items } => {
                let sm = build(items, brush);
                (MF_OWNERDRAW | MF_POPUP, sm.0 as usize, Data { text: wide(text), checked: false, enabled: true, sep: false, sub: true })
            }
        };
        let b = Box::new(data);
        let ptr = &*b as *const Data as *const u16;
        let _ = AppendMenuW(m, flags, id, PCWSTR(ptr));
        DATA.with(|d| d.borrow_mut().push(b));
    }
    m
}

/// Shows the menu at (x, y) and returns the chosen id (0 = nothing). Modal: call outside app borrows.
pub fn show(hwnd: HWND, x: i32, y: i32, items: &[M], s: f32) -> u32 {
    unsafe {
        SCALE.with(|c| *c.borrow_mut() = s);
        let brush = CreateSolidBrush(COLORREF(0x00191616)); // RGB(22,22,25)
        let m = build(items, brush);
        let _ = SetForegroundWindow(hwnd);
        let fg = GetForegroundWindow() == hwnd;
        let r = TrackPopupMenuEx(m, (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY).0, x, y, hwnd, None);
        if std::env::var_os("DOCKX_DEBUG").is_some() {
            log_error("menu", &format!("track r={} err={:?} fg={} count={}", r.0, GetLastError(), fg, GetMenuItemCount(m)));
        }
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(m);
        let _ = DeleteObject(brush);
        DATA.with(|d| d.borrow_mut().clear());
        r.0 as u32
    }
}

pub unsafe fn on_measure(lp: LPARAM) -> bool {
    let mis = &mut *(lp.0 as *mut MEASUREITEMSTRUCT);
    if mis.CtlType != ODT_MENU || mis.itemData == 0 {
        return false;
    }
    let d = &*(mis.itemData as *const Data);
    let s = SCALE.with(|c| *c.borrow());
    if d.sep {
        mis.itemHeight = (9.0 * s) as u32;
        mis.itemWidth = (60.0 * s) as u32;
        return true;
    }
    let dc = GetDC(None);
    let old = SelectObject(dc, font(s));
    let mut sz = SIZE::default();
    let n = d.text.len().saturating_sub(1);
    let _ = GetTextExtentPoint32W(dc, &d.text[..n], &mut sz);
    SelectObject(dc, old);
    ReleaseDC(None, dc);
    mis.itemWidth = (sz.cx as f32 + 64.0 * s) as u32;
    mis.itemHeight = (30.0 * s) as u32;
    true
}

pub unsafe fn on_draw(lp: LPARAM) -> bool {
    let dis = &*(lp.0 as *const DRAWITEMSTRUCT);
    if dis.CtlType != ODT_MENU || dis.itemData == 0 {
        return false;
    }
    let d = &*(dis.itemData as *const Data);
    let s = SCALE.with(|c| *c.borrow());
    let dc = dis.hDC;
    let r = dis.rcItem;
    let bg = CreateSolidBrush(COLORREF(0x00191616));
    FillRect(dc, &r, bg);
    let _ = DeleteObject(bg);
    if d.sep {
        let pen = CreatePen(PS_SOLID, 1, COLORREF(0x003a3434));
        let op = SelectObject(dc, pen);
        let y = (r.top + r.bottom) / 2;
        let _ = MoveToEx(dc, r.left + (10.0 * s) as i32, y, None);
        let _ = LineTo(dc, r.right - (10.0 * s) as i32, y);
        SelectObject(dc, op);
        let _ = DeleteObject(pen);
        return true;
    }
    let selected = dis.itemState.0 & ODS_SELECTED.0 != 0 && d.enabled;
    if selected {
        let b = CreateSolidBrush(COLORREF(0x00221A96)); // RGB(150,26,34)
        let ob = SelectObject(dc, b);
        let op = SelectObject(dc, GetStockObject(NULL_PEN));
        let rad = (10.0 * s) as i32;
        let _ = RoundRect(dc, r.left + (4.0 * s) as i32, r.top + 1, r.right - (4.0 * s) as i32, r.bottom - 1, rad, rad);
        SelectObject(dc, ob);
        SelectObject(dc, op);
        let _ = DeleteObject(b);
    }
    let fg = if d.enabled { COLORREF(0x00ececec) } else { COLORREF(0x00736e6e) };
    if d.checked {
        let pen = CreatePen(PS_SOLID, (2.0 * s).max(1.0) as i32, COLORREF(0x00f0f0f0));
        let op = SelectObject(dc, pen);
        let cx = r.left as f32 + 16.0 * s;
        let cy = (r.top + r.bottom) as f32 / 2.0;
        let _ = MoveToEx(dc, (cx - 5.0 * s) as i32, (cy + 0.5 * s) as i32, None);
        let _ = LineTo(dc, (cx - 1.5 * s) as i32, (cy + 4.0 * s) as i32);
        let _ = LineTo(dc, (cx + 5.5 * s) as i32, (cy - 4.5 * s) as i32);
        SelectObject(dc, op);
        let _ = DeleteObject(pen);
    }
    let of = SelectObject(dc, font(s));
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, fg);
    let mut tr = RECT { left: r.left + (30.0 * s) as i32, top: r.top, right: r.right - (24.0 * s) as i32, bottom: r.bottom };
    let n = d.text.len().saturating_sub(1);
    let mut txt = d.text[..n].to_vec();
    DrawTextW(dc, &mut txt, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_LEFT | DT_NOPREFIX);
    SelectObject(dc, of);
    if d.sub {
        let pen = CreatePen(PS_SOLID, (1.6 * s).max(1.0) as i32, COLORREF(0x00c3bebe));
        let op = SelectObject(dc, pen);
        let x = r.right - (16.0 * s) as i32;
        let y = (r.top + r.bottom) / 2;
        let k = (4.0 * s) as i32;
        let _ = MoveToEx(dc, x - k / 2, y - k, None);
        let _ = LineTo(dc, x + k / 2, y);
        let _ = LineTo(dc, x - k / 2, y + k + 1);
        SelectObject(dc, op);
        let _ = DeleteObject(pen);
        // keep Windows from painting its own (dark-on-dark) arrow over ours
        ExcludeClipRect(dc, r.left, r.top, r.right, r.bottom);
    }
    true
}
