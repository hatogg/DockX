//! Small Windows helpers: wide strings, paths, logging, registry, clipboard, clock.

use std::path::PathBuf;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::SystemInformation::*;
use windows::Win32::System::Time::*;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(w: &[u16]) -> String {
    let n = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..n])
}

pub fn h(s: &str) -> HSTRING {
    HSTRING::from(s)
}

pub fn appdata_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("DockX")
}

pub fn log_error(wherex: &str, msg: &str) {
    let dir = appdata_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("error.log")) {
        use std::io::Write;
        let t = crate::ui::time::local(unix_now(), tz_offset_min());
        let _ = writeln!(f, "{} {} [{}] {}", t.iso(), t.hm(), wherex, msg);
    }
}

/// Writes atomically (tmp + rename) so a crash never leaves a half-written file.
pub fn write_atomic(path: &std::path::Path, text: &str) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, text.as_bytes()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub fn unix_now() -> f64 {
    unsafe {
        let ft = GetSystemTimeAsFileTime();
        let t = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
        (t as f64 - 116444736000000000.0) / 1e7
    }
}

/// Local UTC offset in minutes (DST included).
pub fn tz_offset_min() -> i32 {
    unsafe {
        let mut tz = TIME_ZONE_INFORMATION::default();
        let r = GetTimeZoneInformation(&mut tz);
        let mut bias = tz.Bias;
        if r == 2 {
            bias += tz.DaylightBias; // TIME_ZONE_ID_DAYLIGHT
        } else if r == 1 {
            bias += tz.StandardBias;
        }
        -bias
    }
}

pub fn tick_ms() -> u64 {
    unsafe { GetTickCount64() }
}

// ───────── registry (autostart) ─────────

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn autostart_get(name: &str) -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &h(RUN_KEY), 0, KEY_READ, &mut key).is_err() {
            return false;
        }
        let ok = RegQueryValueExW(key, &h(name), None, None, None, None).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

pub fn autostart_set(name: &str, on: bool) -> bool {
    unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(HKEY_CURRENT_USER, &h(RUN_KEY), 0, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut key, None).is_err() {
            return false;
        }
        let ok = if on {
            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
            let v = wide(&format!("\"{}\"", exe));
            let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2);
            RegSetValueExW(key, &h(name), 0, REG_SZ, Some(bytes)).is_ok()
        } else {
            let _ = RegDeleteValueW(key, &h(name));
            true
        };
        let _ = RegCloseKey(key);
        ok
    }
}

// ───────── clipboard ─────────

pub fn clipboard_text(hwnd: HWND) -> Option<String> {
    unsafe {
        if IsClipboardFormatAvailable(13).is_err() {
            return None; // CF_UNICODETEXT
        }
        let mut tries = 0;
        while OpenClipboard(hwnd).is_err() {
            tries += 1;
            if tries > 5 {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let mut out = None;
        if let Ok(hd) = GetClipboardData(13) {
            let g = HGLOBAL(hd.0);
            let p = GlobalLock(g) as *const u16;
            if !p.is_null() {
                let mut n = 0;
                while *p.add(n) != 0 && n < 4_000_000 {
                    n += 1;
                }
                out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)));
                let _ = GlobalUnlock(g);
            }
        }
        let _ = CloseClipboard();
        out
    }
}

pub fn hglobal_text(text: &str) -> Option<HGLOBAL> {
    unsafe {
        let w = wide(text);
        let g = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2).ok()?;
        let p = GlobalLock(g) as *mut u16;
        if p.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
        let _ = GlobalUnlock(g);
        Some(g)
    }
}

pub fn set_clipboard_text(hwnd: HWND, text: &str) -> bool {
    unsafe {
        if OpenClipboard(hwnd).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let ok = match hglobal_text(text) {
            Some(g) => SetClipboardData(13, HANDLE(g.0)).is_ok(),
            None => false,
        };
        let _ = CloseClipboard();
        ok
    }
}

/// Windows' taskbar «Automatically hide» setting.
pub fn taskbar_autohide_get() -> bool {
    use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA};
    unsafe {
        let mut d = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, ..Default::default() };
        SHAppBarMessage(ABM_GETSTATE, &mut d) as u32 & ABS_AUTOHIDE != 0
    }
}

pub fn taskbar_autohide_set(on: bool) {
    use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_SETSTATE, ABS_ALWAYSONTOP, ABS_AUTOHIDE, APPBARDATA};
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe {
        let tray = FindWindowW(&HSTRING::from("Shell_TrayWnd"), PCWSTR::null()).unwrap_or_default();
        let state = if on { ABS_AUTOHIDE } else { ABS_ALWAYSONTOP };
        let mut d = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, hWnd: tray, lParam: LPARAM(state as isize), ..Default::default() };
        SHAppBarMessage(ABM_SETSTATE, &mut d);
    }
}
