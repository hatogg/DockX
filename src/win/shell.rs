//! Shell work: icons, shortcut targets, display names, launching, focusing windows, process scan, file dialogs.

use super::util::*;
use crate::ui::gfx::{self, Fonts, Image};
use std::collections::HashMap;
use std::rc::Rc;
use tiny_skia::Pixmap;
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn exists(p: &str) -> bool {
    std::path::Path::new(p).exists()
}

/// High-resolution shell image (icon or thumbnail) as premultiplied RGBA.
pub fn shell_image(path: &str, size: i32, icon_only: bool) -> Option<Pixmap> {
    unsafe {
        let f: IShellItemImageFactory = SHCreateItemFromParsingName(&h(path), None).ok()?;
        let mut flags = SIIGBF_BIGGERSIZEOK;
        if icon_only {
            flags |= SIIGBF_ICONONLY;
        }
        let hbm = f.GetImage(SIZE { cx: size, cy: size }, flags).ok()?;
        let pm = hbitmap_to_pixmap(hbm);
        let _ = DeleteObject(hbm);
        pm
    }
}

unsafe fn hbitmap_to_pixmap(hbm: HBITMAP) -> Option<Pixmap> {
    let mut ds = DIBSECTION::default();
    let n = GetObjectW(hbm, std::mem::size_of::<DIBSECTION>() as i32, Some(&mut ds as *mut _ as *mut _));
    if n == 0 || ds.dsBm.bmBits.is_null() || ds.dsBm.bmBitsPixel != 32 {
        return None;
    }
    let (w, hgt, stride) = (ds.dsBm.bmWidth as usize, ds.dsBm.bmHeight as usize, ds.dsBm.bmWidthBytes as usize);
    let bottom_up = ds.dsBmih.biHeight > 0;
    let src = std::slice::from_raw_parts(ds.dsBm.bmBits as *const u8, stride * hgt);
    let mut px = vec![0u8; w * hgt * 4];
    let mut any_alpha = false;
    for y in 0..hgt {
        let sy = if bottom_up { hgt - 1 - y } else { y };
        for x in 0..w {
            let s = sy * stride + x * 4;
            let d = (y * w + x) * 4;
            px[d] = src[s + 2];
            px[d + 1] = src[s + 1];
            px[d + 2] = src[s];
            px[d + 3] = src[s + 3];
            if src[s + 3] != 0 {
                any_alpha = true;
            }
        }
    }
    if !any_alpha {
        // an all-black opaque bitmap means the shell had no real icon: use our fallback tile
        if px.chunks_exact(4).all(|p| p[0] < 8 && p[1] < 8 && p[2] < 8) {
            return None;
        }
        for i in (3..px.len()).step_by(4) {
            px[i] = 255;
        }
    }
    // Store-app shortcuts come back «plated»: the logo on a flat opaque square. Cut that plate away.
    gfx::unplate(&mut px, w, hgt);
    // shell bitmaps are premultiplied already (32bpp with alpha)
    let mut pm = Pixmap::new(w as u32, hgt as u32)?;
    pm.data_mut().copy_from_slice(&px);
    Some(trim(pm))
}

/// Apps without a 256 px icon come back as a small icon in a big empty canvas: crop it (like 1.1).
fn trim(src: Pixmap) -> Pixmap {
    let (w, hgt) = (src.width() as usize, src.height() as usize);
    let d = src.data();
    let (mut minx, mut miny, mut maxx, mut maxy) = (w, hgt, 0usize, 0usize);
    let mut found = false;
    for y in 0..hgt {
        for x in 0..w {
            if d[(y * w + x) * 4 + 3] > 10 {
                found = true;
                minx = minx.min(x);
                maxx = maxx.max(x);
                miny = miny.min(y);
                maxy = maxy.max(y);
            }
        }
    }
    if !found {
        return src;
    }
    let (bw, bh) = (maxx - minx + 1, maxy - miny + 1);
    let side = bw.max(bh);
    if side as f32 >= w as f32 * 0.72 {
        return src;
    }
    let out_side = (side as f32 * 1.06).ceil();
    let (cx, cy) = (minx as f32 + bw as f32 / 2.0, miny as f32 + bh as f32 / 2.0);
    let mut dst = Pixmap::new(256, 256).unwrap();
    let k = 256.0 / out_side;
    let t = tiny_skia::Transform::from_row(k, 0.0, 0.0, k, -(cx - out_side / 2.0) * k, -(cy - out_side / 2.0) * k);
    dst.draw_pixmap(0, 0, src.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t, None);
    dst
}

pub fn icon_mips(path: &str, name: &str, fonts: &Fonts) -> (Vec<Image>, bool) {
    let missing = !exists(path);
    let pm = if missing { None } else { shell_image(path, 256, true) };
    let ok = pm.is_some();
    (gfx::mips(pm.unwrap_or_else(|| gfx::fallback_icon(name, fonts, 256))), ok && !missing)
}

/// Shelf tiles show real thumbnails for images/videos, icons for the rest.
pub fn file_thumb(path: &str) -> Option<Image> {
    shell_image(path, 128, false).or_else(|| shell_image(path, 128, true)).map(|p| Rc::new(gfx::resize(&p, 128)))
}

pub fn resolve_target(path: &str) -> Option<String> {
    let low = path.to_ascii_lowercase();
    if low.ends_with(".exe") {
        return Some(path.to_string());
    }
    if !low.ends_with(".lnk") {
        return None;
    }
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let pf: IPersistFile = link.cast().ok()?;
        pf.Load(&h(path), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let t = from_wide(&buf);
        if t.to_ascii_lowercase().ends_with(".exe") {
            Some(t)
        } else {
            None
        }
    }
}

/// FileDescription from the exe version info, or the file name.
pub fn display_name(path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_dir() {
        return p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
    }
    if path.to_ascii_lowercase().ends_with(".exe") {
        if let Some(d) = file_description(path) {
            if d.chars().count() > 1 && d.chars().count() <= 40 {
                return d;
            }
        }
    }
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string())
}

fn file_description(path: &str) -> Option<String> {
    unsafe {
        let hp = h(path);
        let size = GetFileVersionInfoSizeW(&hp, None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(&hp, 0, size, buf.as_mut_ptr() as *mut _).ok()?;
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let mut langs = vec![(0x0409u16, 0x04B0u16)];
        if VerQueryValueW(buf.as_ptr() as *const _, &h("\\VarFileInfo\\Translation"), &mut ptr, &mut len).as_bool() && len >= 4 {
            let t = ptr as *const u16;
            langs.insert(0, (*t, *t.add(1)));
        }
        for (l, cp) in langs {
            let q = format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", l, cp);
            if VerQueryValueW(buf.as_ptr() as *const _, &h(&q), &mut ptr, &mut len).as_bool() && len > 0 {
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr as *const u16, len as usize));
                let s = s.trim_end_matches('\0').trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

/// ShellExecuteEx; Err(code) except when the user cancels UAC (1223 → Ok(false)).
pub fn launch(path: &str, args: &str, admin: bool) -> Result<bool, String> {
    unsafe {
        let file = wide(path);
        let params = wide(args);
        let verb = wide("runas");
        let dir = if path.to_ascii_lowercase().ends_with(".exe") {
            std::path::Path::new(path).parent().map(|d| wide(&d.display().to_string()))
        } else {
            None
        };
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC,
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: if args.is_empty() { PCWSTR::null() } else { PCWSTR(params.as_ptr()) },
            lpVerb: if admin { PCWSTR(verb.as_ptr()) } else { PCWSTR::null() },
            lpDirectory: dir.as_ref().map(|d| PCWSTR(d.as_ptr())).unwrap_or(PCWSTR::null()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        match ShellExecuteExW(&mut info) {
            Ok(()) => Ok(true),
            Err(e) => {
                if e.code().0 as u32 == 0x800704C7 || GetLastError().0 == 1223 {
                    Ok(false)
                } else {
                    Err(e.message().to_string())
                }
            }
        }
    }
}

pub fn reveal(path: &str) {
    let _ = launch("explorer.exe", &format!("/select,\"{}\"", path), false);
}

pub fn open_folder(path: &str) {
    let _ = std::fs::create_dir_all(path);
    let _ = launch("explorer.exe", &format!("\"{}\"", path), false);
}

/// Brings the first normal window of these processes to the front.
pub fn focus_pids(pids: &[u32]) -> bool {
    struct Ctx<'a> {
        pids: &'a [u32],
        found: HWND,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> BOOL {
        let ctx = &mut *(lp.0 as *mut Ctx);
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        if GetWindow(hwnd, GW_OWNER).map(|w| !w.is_invalid()).unwrap_or(false) {
            return TRUE;
        }
        if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0 {
            return TRUE;
        }
        if GetWindowTextLengthW(hwnd) == 0 {
            return TRUE;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if ctx.pids.contains(&pid) {
            ctx.found = hwnd;
            return FALSE;
        }
        TRUE
    }
    unsafe {
        let mut ctx = Ctx { pids, found: HWND::default() };
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize));
        if ctx.found.is_invalid() {
            return false;
        }
        if IsIconic(ctx.found).as_bool() {
            let _ = ShowWindow(ctx.found, SW_RESTORE);
        }
        // Alt down/up lets SetForegroundWindow through
        keybd_event(0x12, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(0x12, 0, KEYEVENTF_KEYUP, 0);
        let _ = SetForegroundWindow(ctx.found);
        true
    }
}

/// exe path (lower-case) → pids of running processes.
pub fn scan_processes() -> HashMap<String, Vec<u32>> {
    let mut map: HashMap<String, Vec<u32>> = HashMap::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return map };
        let mut pe = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                if let Ok(hp) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pe.th32ProcessID) {
                    let mut buf = [0u16; 1024];
                    let mut size = buf.len() as u32;
                    if QueryFullProcessImageNameW(hp, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok() {
                        let p = String::from_utf16_lossy(&buf[..size as usize]).to_lowercase();
                        map.entry(p).or_default().push(pe.th32ProcessID);
                    }
                    let _ = CloseHandle(hp);
                }
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    map
}

/// Open dialog. Folder mode picks one folder; file mode allows several (shortcuts stay shortcuts).
pub fn pick(hwnd: HWND, folder: bool) -> Vec<String> {
    unsafe {
        let Ok(d) = CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) else { return vec![] };
        let mut opts = d.GetOptions().unwrap_or_default();
        if folder {
            opts |= FOS_PICKFOLDERS;
            let _ = d.SetTitle(&h("Оберіть папку для Dock"));
        } else {
            opts |= FOS_ALLOWMULTISELECT | FOS_NODEREFERENCELINKS | FOS_FILEMUSTEXIST;
            let _ = d.SetTitle(&h("Додати в Dock"));
            let n1 = wide("Програми та ярлики");
            let p1 = wide("*.exe;*.lnk;*.bat;*.cmd");
            let n2 = wide("Усі файли");
            let p2 = wide("*.*");
            let specs = [
                COMDLG_FILTERSPEC { pszName: PCWSTR(n1.as_ptr()), pszSpec: PCWSTR(p1.as_ptr()) },
                COMDLG_FILTERSPEC { pszName: PCWSTR(n2.as_ptr()), pszSpec: PCWSTR(p2.as_ptr()) },
            ];
            let _ = d.SetFileTypes(&specs);
        }
        let _ = d.SetOptions(opts);
        if d.Show(hwnd).is_err() {
            return vec![];
        }
        let mut out = Vec::new();
        if let Ok(arr) = d.GetResults() {
            let n = arr.GetCount().unwrap_or(0);
            for i in 0..n {
                if let Ok(item) = arr.GetItemAt(i) {
                    if let Ok(p) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                        out.push(p.to_string().unwrap_or_default());
                        CoTaskMemFree(Some(p.0 as *const _));
                    }
                }
            }
        }
        out
    }
}

pub fn message(hwnd: HWND, text: &str) {
    unsafe {
        MessageBoxW(hwnd, &h(text), &h("DockX"), MB_OK | MB_ICONWARNING);
    }
}
