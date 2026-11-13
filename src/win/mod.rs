//! Windows host: layered windows for the dock, the island and the settings, a vsync-paced frame
//! clock, input, OLE drag & drop, menus, and the workers (media, lyrics, system monitor, process scan).

mod controls;
mod dnd;
mod media;
mod menu;
mod shell;
mod sysmon;
mod util;

use crate::ui::dock::*;
use crate::ui::gfx::{self, Canvas, Font, Fonts};
use crate::ui::island::*;
use crate::ui::settings::{self, AppSettings, SettingsAction, SettingsPanel};
use crate::ui::text_field::Key;
use crate::ui::{lrc, time, Btn, Mods};
use dnd::{DropKind, DropSink};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
use util::*;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmFlush, DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::DataExchange::AddClipboardFormatListener;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::*;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_APP_FRAME: u32 = WM_APP + 1;
const WM_APP_WAKE: u32 = WM_APP + 2;
const KIND_DOCK: isize = 1;
const KIND_ISLAND: isize = 2;
const KIND_SETTINGS: isize = 3;

static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static ANIM: AtomicBool = AtomicBool::new(false);
static FRAME_PENDING: AtomicBool = AtomicBool::new(false);
static FAST_SCAN: AtomicBool = AtomicBool::new(false);
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

fn post_wake() {
    let h = MAIN_HWND.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(HWND(h as *mut _), WM_APP_WAKE, WPARAM(0), LPARAM(0));
        }
    }
}

// ───────────────────────── layered surface ─────────────────────────

struct Surface {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    w: u32,
    h: u32,
    shown: bool,
    last_rev: u64,
    last_pos: (i32, i32, u8),
}

impl Surface {
    fn new() -> Surface {
        Surface { dc: HDC::default(), bmp: HBITMAP::default(), old: HGDIOBJ::default(), bits: std::ptr::null_mut(), w: 0, h: 0, shown: false, last_rev: u64::MAX, last_pos: (i32::MIN, 0, 0) }
    }

    unsafe fn ensure(&mut self, w: u32, h: u32) {
        if !self.bits.is_null() && w <= self.w && h <= self.h {
            return;
        }
        self.free();
        let (nw, nh) = (w.max(self.w), h.max(self.h));
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: nw as i32,
                biHeight: -(nh as i32),
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let sdc = GetDC(None);
        self.bmp = CreateDIBSection(sdc, &bi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        ReleaseDC(None, sdc);
        self.dc = CreateCompatibleDC(None);
        self.old = SelectObject(self.dc, self.bmp);
        self.bits = bits as *mut u8;
        self.w = nw;
        self.h = nh;
    }

    unsafe fn free(&mut self) {
        if !self.dc.is_invalid() {
            SelectObject(self.dc, self.old);
            let _ = DeleteDC(self.dc);
        }
        if !self.bmp.is_invalid() {
            let _ = DeleteObject(self.bmp);
        }
        self.bits = std::ptr::null_mut();
        self.dc = HDC::default();
        self.bmp = HBITMAP::default();
    }

    /// Copies the canvas (RGBA premultiplied) into the DIB (BGRA) and updates the layered window.
    /// `pos` None keeps the window where it is (the settings window can be dragged).
    unsafe fn present(&mut self, hwnd: HWND, c: &Canvas, pos: Option<(i32, i32)>, alpha: u8) {
        let (w, h) = (c.w(), c.h());
        self.ensure(w, h);
        if self.bits.is_null() {
            return;
        }
        let src = c.pm.data();
        let stride = self.w as usize * 4;
        for y in 0..h as usize {
            let s = &src[y * w as usize * 4..(y + 1) * w as usize * 4];
            let d = std::slice::from_raw_parts_mut(self.bits.add(y * stride), w as usize * 4);
            for (sp, dp) in s.chunks_exact(4).zip(d.chunks_exact_mut(4)) {
                dp[0] = sp[2];
                dp[1] = sp[1];
                dp[2] = sp[0];
                dp[3] = sp[3];
            }
        }
        let sdc = GetDC(None);
        let size = SIZE { cx: w as i32, cy: h as i32 };
        let src_pt = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: alpha, AlphaFormat: 1 };
        let dst = pos.map(|(x, y)| POINT { x, y });
        let _ = UpdateLayeredWindow(
            hwnd,
            sdc,
            dst.as_ref().map(|p| p as *const POINT),
            Some(&size),
            self.dc,
            Some(&src_pt),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        ReleaseDC(None, sdc);
    }

    unsafe fn set_visible(&mut self, hwnd: HWND, v: bool) {
        if v == self.shown {
            return;
        }
        self.shown = v;
        let _ = ShowWindow(hwnd, if v { SW_SHOWNOACTIVATE } else { SW_HIDE });
        if v {
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

// ───────────────────────── app state ─────────────────────────

enum Modal {
    DockMenu(Option<usize>, i32, i32),
    IslandMenu(i32, i32),
    AudioMenu(i32, i32),
    Pick(bool),
    DragFiles(Vec<String>),
    DragText(String),
    Warn(String),
    MoveSettings,
}

struct SettingsWin {
    hwnd: HWND,
    panel: SettingsPanel,
    surf: Surface,
}

struct App {
    s: f32,
    fonts: Rc<Fonts>,
    cfg: AppSettings,
    dock: Dock,
    island: Island,
    settings: Option<SettingsWin>,
    hwnd_dock: HWND,
    hwnd_island: HWND,
    surf_dock: Surface,
    surf_island: Surface,
    t0: Instant,
    screen: gfx::RectF,
    work: gfx::RectF,

    media_rx: Receiver<media::Snapshot>,
    media_cmd: Sender<media::MediaCmd>,
    lyrics_req: Sender<media::LyricsReq>,
    lyrics_rx: Receiver<(String, media::LyricsRes)>,
    sys_rx: Receiver<sysmon::SysEvent>,
    scan_rx: Receiver<HashMap<String, Vec<u32>>>,
    running: HashMap<String, Vec<u32>>,
    art: Option<gfx::Image>,
    art_id: u64,
    lyrics_key: String,

    volume: sysmon::Volume,
    last_vol: Option<(f32, bool)>,
    vol_set_at: f64,
    last_caps: Option<bool>,
    last_layout: Option<u32>,
    last_poll: f64,
    last_battery: f64,
    last_fs: f64,
    fullscreen: bool,
    idle: f64,

    menu_open: bool,
    internal_drag: bool,
    kb: isize,
    ignore_clip_until: f64,
    save_cfg_at: f64,
    save_data_at: f64,
    modal: Vec<Modal>,
    capture: bool,
    surrogate: u16,

    ctl_tx: Sender<controls::Cmd>,
    ctl_rx: Receiver<controls::Snapshot>,
    ctl_snap: controls::Snapshot,
    ctl_awake: bool,
    gamma: controls::Gamma,
    ctl_was_open: bool,
    ctl_local_at: f64,
    fs_since: f64,
    taskbar_hidden: bool,
    last_taskbar: f64,
    dock_covered: bool,
    last_cover: f64,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| match a.try_borrow_mut() {
        Ok(mut g) => g.as_mut().map(f),
        Err(_) => None,
    })
}

fn load_fonts() -> Fonts {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\Fonts\\";
    let load = |names: &[&str]| -> Font {
        for n in names {
            if let Ok(b) = std::fs::read(format!("{dir}{n}")) {
                if let Some(f) = Font::from_bytes(&b) {
                    return f;
                }
            }
        }
        Font::from_bytes(&std::fs::read(format!("{dir}arial.ttf")).unwrap_or_default()).expect("no usable font")
    };
    Fonts { regular: load(&["segoeui.ttf", "arial.ttf"]), semibold: load(&["seguisb.ttf", "segoeuib.ttf", "arialbd.ttf"]), bold: load(&["segoeuib.ttf", "arialbd.ttf"]) }
}

fn primary_rects() -> (gfx::RectF, gfx::RectF) {
    unsafe {
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        let r = |r: RECT| gfx::rect(r.left as f32, r.top as f32, (r.right - r.left) as f32, (r.bottom - r.top) as f32);
        (r(mi.rcMonitor), r(mi.rcWork))
    }
}

fn default_items() -> Vec<(String, String)> {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let win = env("WINDIR");
    let pf = env("ProgramFiles");
    let pf86 = env("ProgramFiles(x86)");
    let local = env("LOCALAPPDATA");
    let roaming = env("APPDATA");
    let mut v = Vec::new();
    let mut add = |p: String, n: &str| {
        if std::path::Path::new(&p).exists() {
            v.push((p, n.to_string()));
        }
    };
    add(format!("{win}\\explorer.exe"), "Провідник");
    for b in [
        format!("{pf}\\Google\\Chrome\\Application\\chrome.exe"),
        format!("{pf86}\\Google\\Chrome\\Application\\chrome.exe"),
        format!("{local}\\Google\\Chrome\\Application\\chrome.exe"),
        format!("{pf86}\\Microsoft\\Edge\\Application\\msedge.exe"),
        format!("{pf}\\Mozilla Firefox\\firefox.exe"),
    ] {
        if std::path::Path::new(&b).exists() {
            add(b, "");
            break;
        }
    }
    add(format!("{roaming}\\Telegram Desktop\\Telegram.exe"), "Telegram");
    add(format!("{local}\\Programs\\Microsoft VS Code\\Code.exe"), "VS Code");
    add(format!("{pf86}\\Steam\\steam.exe"), "Steam");
    add(format!("{win}\\System32\\notepad.exe"), "Блокнот");
    add(format!("{win}\\System32\\Taskmgr.exe"), "Диспетчер завдань");
    v
}

fn make_item(path: &str, name: &str, fonts: &Fonts) -> DockItem {
    let name = if name.trim().is_empty() { shell::display_name(path) } else { name.to_string() };
    let (mips, _) = shell::icon_mips(path, &name, fonts);
    let mut it = DockItem::new(path, &name, mips);
    it.missing = !shell::exists(path);
    it.target = if it.missing { None } else { shell::resolve_target(path) };
    it
}

impl App {
    fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn cursor(&self) -> (f32, f32) {
        unsafe {
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            (p.x as f32, p.y as f32)
        }
    }

    fn apply_cfg(&mut self) {
        if !self.cfg.show_dock && !self.cfg.island.enabled {
            self.cfg.show_dock = true; // never hide everything: settings live in the dock menu
        }
        self.dock.cfg = self.cfg.dock.clone();
        self.island.cfg = self.cfg.island.clone();
        self.dock.kick();
        self.island.kick();
        if autostart_get("DockX") != self.cfg.autostart {
            autostart_set("DockX", self.cfg.autostart);
        }
        self.apply_taskbar();
        self.save_cfg_at = self.now() + 0.4;
    }

    /// «Сховати панель задач»: auto-hide (so the work area spans the whole screen and the dock sits
    /// at the very bottom) plus the taskbar windows hidden; the user's own auto-hide choice comes back after.
    fn apply_taskbar(&mut self) {
        if self.cfg.hide_taskbar && !self.taskbar_hidden {
            taskbar_autohide_set(true);
            taskbars_show(false);
            self.taskbar_hidden = true;
        } else if !self.cfg.hide_taskbar && self.taskbar_hidden {
            self.restore_taskbar();
        }
    }

    fn restore_taskbar(&mut self) {
        if self.taskbar_hidden {
            taskbars_show(true);
            taskbar_autohide_set(self.cfg.taskbar_prev_autohide);
            self.taskbar_hidden = false;
        }
    }

    /// Windows shows the taskbar again now and then (Start, Explorer restart): hide it again,
    /// except while Start, search or the tray overflow is open.
    fn keep_taskbar_hidden(&mut self, now: f64) {
        if !self.taskbar_hidden || now - self.last_taskbar < 1.0 {
            return;
        }
        self.last_taskbar = now;
        unsafe {
            let fg = GetForegroundWindow();
            let mut cls = [0u16; 64];
            let n = GetClassNameW(fg, &mut cls);
            let cls = String::from_utf16_lossy(&cls[..n as usize]);
            if matches!(cls.as_str(), "Windows.UI.Core.CoreWindow" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "NotifyIconOverflowWindow" | "TopLevelWindowForOverflowXamlIsland") {
                return;
            }
        }
        if taskbars_visible() {
            taskbar_autohide_set(true);
            taskbars_show(false);
        }
    }

    fn save_now(&mut self) {
        let dir = appdata_dir();
        if self.save_cfg_at > 0.0 {
            self.save_cfg_at = 0.0;
            write_atomic(&dir.join("dock.ini"), &settings::to_ini(&self.cfg, &self.dock.items));
        }
        if self.save_data_at > 0.0 {
            self.save_data_at = 0.0;
            write_atomic(&dir.join("island.dat"), &self.island.data_text());
        }
    }

    /// One step of everything; returns true while frames are needed.
    fn tick(&mut self) -> bool {
        let now = self.now();
        let cursor = self.cursor();
        self.poll_workers(now);
        self.poll_system(now);
        self.poll_controls(now);

        let mut anim = false;
        if self.cfg.show_dock {
            if now - self.last_cover > 0.25 {
                self.last_cover = now;
                self.dock_covered = self.cfg.dock.smart_hide && self.window_over_dock();
            }
            let env = DockEnv { now, cursor, work: self.work, screen: self.screen, s: self.s, fullscreen: self.fullscreen, menu_open: self.menu_open, covered: self.dock_covered };
            anim |= self.dock.step(&env);
        } else {
            self.dock.win_visible = false;
        }
        FAST_SCAN.store(self.dock.bouncing(), Ordering::Relaxed);
        let ienv = IslandEnv {
            now,
            cursor,
            screen: self.screen,
            s: self.s,
            fullscreen: self.fullscreen,
            menu_open: self.menu_open,
            unix: unix_now(),
            tz_min: tz_offset_min(),
            idle_secs: self.idle,
        };
        anim |= self.island.step(&ienv);
        if let Some(sw) = &mut self.settings {
            anim |= sw.panel.step(now);
        }
        self.handle_dock_actions();
        self.handle_island_actions(now);
        self.handle_settings_actions();
        self.present();
        let dir = appdata_dir();
        if self.save_cfg_at > 0.0 && now >= self.save_cfg_at {
            self.save_cfg_at = 0.0;
            write_atomic(&dir.join("dock.ini"), &settings::to_ini(&self.cfg, &self.dock.items));
        }
        if self.save_data_at > 0.0 && now >= self.save_data_at {
            self.save_data_at = 0.0;
            write_atomic(&dir.join("island.dat"), &self.island.data_text());
        }
        anim
    }

    fn present(&mut self) {
        unsafe {
            let d = &self.dock;
            if d.win_visible && self.cfg.show_dock {
                let key = (d.win_x, d.win_y, d.win_alpha);
                if d.rendered != self.surf_dock.last_rev || key != self.surf_dock.last_pos || !self.surf_dock.shown {
                    self.surf_dock.last_rev = d.rendered;
                    self.surf_dock.last_pos = key;
                    self.surf_dock.present(self.hwnd_dock, &d.canvas, Some((d.win_x, d.win_y)), d.win_alpha);
                }
                self.surf_dock.set_visible(self.hwnd_dock, true);
            } else {
                self.surf_dock.set_visible(self.hwnd_dock, false);
            }
            let i = &self.island;
            if i.win_visible {
                let key = (i.win_x, i.win_y, 255);
                if i.rendered != self.surf_island.last_rev || key != self.surf_island.last_pos || !self.surf_island.shown {
                    self.surf_island.last_rev = i.rendered;
                    self.surf_island.last_pos = key;
                    self.surf_island.present(self.hwnd_island, &i.canvas, Some((i.win_x, i.win_y)), 255);
                }
                self.surf_island.set_visible(self.hwnd_island, true);
            } else {
                self.surf_island.set_visible(self.hwnd_island, false);
            }
            if let Some(sw) = &mut self.settings {
                if sw.panel.rendered != sw.surf.last_rev {
                    sw.surf.last_rev = sw.panel.rendered;
                    sw.surf.present(sw.hwnd, &sw.panel.canvas, None, 255);
                }
            }
        }
    }

    fn poll_workers(&mut self, now: f64) {
        while let Ok(snap) = self.media_rx.try_recv() {
            if snap.art_id != self.art_id {
                self.art_id = snap.art_id;
                self.art = snap.art.as_ref().and_then(|(w, h, px)| {
                    let mut pm = tiny_skia::Pixmap::new(*w, *h)?;
                    pm.data_mut().copy_from_slice(&px[..(*w * *h * 4) as usize]);
                    Some(Rc::new(pm))
                });
            } else if let Some((w, h, px)) = &snap.art {
                if let Some(mut pm) = tiny_skia::Pixmap::new(*w, *h) {
                    pm.data_mut().copy_from_slice(&px[..(*w * *h * 4) as usize]);
                    self.art = Some(Rc::new(pm));
                }
            }
            let age = (unix_now() - snap.sampled_unix).max(0.0);
            let m = MediaInfo {
                has: snap.has,
                title: snap.title.clone(),
                artist: snap.artist.clone(),
                album: snap.album.clone(),
                app: snap.app.clone(),
                art: if snap.has { self.art.clone() } else { None },
                playing: snap.playing,
                position: snap.position,
                duration: snap.duration,
                sampled_at: now - age,
                can_prev: snap.can_prev,
                can_next: snap.can_next,
                players: snap.players.clone(),
                player_idx: snap.player_idx,
            };
            let key = m.key();
            self.island.set_media(m);
            if snap.has && key != self.lyrics_key {
                self.lyrics_key = key.clone();
                if self.cfg.island.lyrics_online {
                    self.island.set_lyrics(&key, Lyrics::Loading);
                    let _ = self.lyrics_req.send(media::LyricsReq { key, title: snap.title, artist: snap.artist, album: snap.album, duration: snap.duration });
                } else {
                    self.island.set_lyrics(&key, Lyrics::NotFound);
                }
            }
        }
        while let Ok((key, res)) = self.lyrics_rx.try_recv() {
            let l = match res {
                media::LyricsRes::Synced(s) => {
                    let lines = lrc::parse(&s);
                    if lines.is_empty() {
                        Lyrics::NotFound
                    } else {
                        Lyrics::Synced(Rc::new(lines))
                    }
                }
                media::LyricsRes::Plain(s) => Lyrics::Plain(Rc::new(s.lines().map(|l| l.to_string()).filter(|l| !l.trim().is_empty()).collect())),
                media::LyricsRes::NotFound => Lyrics::NotFound,
            };
            self.island.set_lyrics(&key, l);
        }
        while let Ok(ev) = self.sys_rx.try_recv() {
            match ev {
                sysmon::SysEvent::Wifi(s) => self.island.wifi_changed(s),
                sysmon::SysEvent::Brightness(b) => self.island.show_hud(Hud::Brightness(b)),
                sysmon::SysEvent::AudioDevice(n) => {
                    self.volume.refresh();
                    self.last_vol = self.volume.get();
                    self.island.audio_device(&n);
                }
            }
        }
        while let Ok(snap) = self.ctl_rx.try_recv() {
            self.ctl_snap = snap;
            self.push_controls(now);
        }
        while let Ok(map) = self.scan_rx.try_recv() {
            self.running = map;
            let mut changed = false;
            for it in &mut self.dock.items {
                let r = pids_for(&self.running, it).is_some();
                if r != it.running {
                    it.running = r;
                    changed = true;
                }
            }
            if changed {
                self.dock.kick();
            }
        }
    }

    fn poll_system(&mut self, now: f64) {
        if now - self.last_poll < 0.12 {
            return;
        }
        self.last_poll = now;
        // volume (HUD when it changes outside the island)
        if let Some(v) = self.volume.get() {
            let ext = now - self.vol_set_at > 0.8;
            if self.last_vol.map(|l| (l.0 - v.0).abs() > 0.004 || l.1 != v.1).unwrap_or(false) {
                self.island.set_volume(v.0, v.1, ext);
            } else if self.last_vol.is_none() {
                self.island.set_volume(v.0, v.1, false);
            }
            self.last_vol = Some(v);
        }
        let caps = sysmon::caps_on();
        if self.last_caps.map(|c| c != caps).unwrap_or(false) {
            self.island.show_hud(Hud::Caps(caps));
        }
        self.last_caps = Some(caps);
        let (lid, lname) = sysmon::layout();
        if self.last_layout.map(|l| l != lid).unwrap_or(false) {
            self.island.show_hud(Hud::Layout(lname));
        }
        self.last_layout = Some(lid);
        self.idle = sysmon::idle_secs();
        if now - self.last_battery > 2.0 {
            self.last_battery = now;
            self.island.set_battery(sysmon::battery());
        }
        self.keep_taskbar_hidden(now);
        if now - self.last_fs > 0.5 {
            self.last_fs = now;
            // hide only when a fullscreen window stays in front (~1 s): task view, overlays or a
            // window flashing through fullscreen while switching must not make everything blink
            if self.fullscreen_active() {
                if self.fs_since < 0.0 {
                    self.fs_since = now;
                }
                self.fullscreen = now - self.fs_since >= 0.9;
            } else {
                self.fs_since = -1.0;
                self.fullscreen = false;
            }
        }
    }

    /// The active window reaches into the dock's place (smart hide).
    fn window_over_dock(&self) -> bool {
        unsafe {
            let h = GetForegroundWindow();
            if h.is_invalid() || h == GetShellWindow() || h == GetDesktopWindow() || IsIconic(h).as_bool() || !IsWindowVisible(h).as_bool() {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == std::process::id() {
                return false;
            }
            let mut cls = [0u16; 64];
            let n = GetClassNameW(h, &mut cls);
            let cls = String::from_utf16_lossy(&cls[..n as usize]);
            if matches!(cls.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Windows.UI.Core.CoreWindow" | "MultitaskingViewFrame" | "#32768") {
                return false;
            }
            let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
            if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0) != 0 {
                return false;
            }
            let mut cloaked = 0u32;
            if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4).is_ok() && cloaked != 0 {
                return false;
            }
            // the visible frame, without the invisible resize borders
            let mut r = RECT::default();
            if DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32).is_err()
                && GetWindowRect(h, &mut r).is_err()
            {
                return false;
            }
            let d = self.dock.rest_rect();
            (r.left as f32) < d.r() && (r.right as f32) > d.x && (r.top as f32) < d.b() && (r.bottom as f32) > d.y
        }
    }

    fn fullscreen_active(&self) -> bool {
        unsafe {
            if let Ok(st) = SHQueryUserNotificationState() {
                if st == QUNS_RUNNING_D3D_FULL_SCREEN || st == QUNS_PRESENTATION_MODE {
                    return true;
                }
            }
            let h = GetForegroundWindow();
            if h.is_invalid() || h == self.hwnd_dock || h == self.hwnd_island || h == GetShellWindow() || h == GetDesktopWindow() {
                return false;
            }
            let mut cls = [0u16; 64];
            let n = GetClassNameW(h, &mut cls);
            let cls = String::from_utf16_lossy(&cls[..n as usize]);
            if matches!(
                cls.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "MultitaskingViewFrame" | "XamlExplorerHostIslandWindow" | "ForegroundStaging" | "Windows.UI.Core.CoreWindow"
            ) {
                return false;
            }
            // our own windows (settings, menus)
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == std::process::id() {
                return false;
            }
            // click-through or non-activating overlays (recording frames, screen dimmers…) are not games
            let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
            if ex & (WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0) != 0 || !IsWindowVisible(h).as_bool() {
                return false;
            }
            let mut cloaked = 0u32;
            if DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4).is_ok() && cloaked != 0 {
                return false;
            }
            if (GetWindowLongW(h, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0 {
                return false;
            }
            let mut r = RECT::default();
            if GetWindowRect(h, &mut r).is_err() {
                return false;
            }
            let mon = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if !GetMonitorInfoW(mon, &mut mi).as_bool() {
                return false;
            }
            let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            mon == primary
                && r.left <= mi.rcMonitor.left
                && r.top <= mi.rcMonitor.top
                && r.right >= mi.rcMonitor.right
                && r.bottom >= mi.rcMonitor.bottom
        }
    }

    fn handle_dock_actions(&mut self) {
        let acts = std::mem::take(&mut self.dock.actions);
        for a in acts {
            match a {
                DockAction::Activate { idx, new_instance, admin } => self.activate(idx, new_instance, admin),
                DockAction::OpenWith { idx, files } => {
                    if let Some(it) = self.dock.items.get(idx) {
                        let args = files.iter().map(|f| format!("\"{}\"", f)).collect::<Vec<_>>().join(" ");
                        match shell::launch(&it.path.clone(), &args, false) {
                            Ok(true) => self.dock.bounce_once(idx),
                            Ok(false) => {}
                            Err(e) => self.modal.push(Modal::Warn(format!("Не вдалося відкрити файли в {}:\n{}", it.name, e))),
                        }
                    }
                }
                DockAction::AddPaths { paths, index, into_gap } => {
                    let fonts = self.fonts.clone();
                    let items: Vec<DockItem> = paths.iter().filter(|p| shell::exists(p) && !self.dock.has_path(p)).map(|p| make_item(p, "", &fonts)).collect();
                    self.dock.insert_items(items, index, into_gap);
                    self.refresh_running();
                }
                DockAction::Menu { idx, x, y } => self.modal.push(Modal::DockMenu(idx, x as i32, y as i32)),
                DockAction::Save => self.save_cfg_at = self.now() + 0.4,
                DockAction::WantKeyboard(on) => self.want_keyboard(KIND_DOCK, on),
            }
        }
    }

    fn refresh_running(&mut self) {
        for it in &mut self.dock.items {
            it.running = pids_for(&self.running, it).is_some();
        }
    }

    fn activate(&mut self, idx: usize, new_instance: bool, admin: bool) {
        let Some(it) = self.dock.items.get(idx) else { return };
        if !new_instance && !admin && it.running && !it.is_explorer() {
            if let Some(pids) = pids_for(&self.running, it) {
                if shell::focus_pids(pids) {
                    return;
                }
            }
        }
        let (path, name) = (it.path.clone(), it.name.clone());
        if !shell::exists(&path) {
            self.modal.push(Modal::Warn(format!("Файл не знайдено:\n{}", path)));
            return;
        }
        match shell::launch(&path, "", admin) {
            Ok(true) => self.dock.start_bounce(idx, new_instance),
            Ok(false) => {}
            Err(e) => self.modal.push(Modal::Warn(format!("Не вдалося запустити {}:\n{}", name, e))),
        }
    }

    fn want_keyboard(&mut self, kind: isize, on: bool) {
        unsafe {
            let hwnd = if kind == KIND_DOCK { self.hwnd_dock } else { self.hwnd_island };
            if on {
                self.kb = kind;
                let _ = SetForegroundWindow(hwnd);
                let _ = SetFocus(hwnd);
            } else if self.kb == kind {
                self.kb = 0;
            }
        }
    }

    fn handle_island_actions(&mut self, now: f64) {
        let acts = std::mem::take(&mut self.island.actions);
        for a in acts {
            match a {
                IslandAction::MediaToggle => {
                    let _ = self.media_cmd.send(media::MediaCmd::Toggle);
                }
                IslandAction::MediaPause => {
                    let _ = self.media_cmd.send(media::MediaCmd::Pause);
                }
                IslandAction::MediaNext => {
                    let _ = self.media_cmd.send(media::MediaCmd::Next);
                }
                IslandAction::MediaPrev => {
                    let _ = self.media_cmd.send(media::MediaCmd::Prev);
                }
                IslandAction::MediaSeek(t) => {
                    let _ = self.media_cmd.send(media::MediaCmd::Seek(t));
                }
                IslandAction::NextPlayer => {
                    let _ = self.media_cmd.send(media::MediaCmd::NextPlayer);
                }
                IslandAction::SetVolume(v) => {
                    self.vol_set_at = now;
                    self.volume.set(v);
                    self.last_vol = self.volume.get();
                }
                IslandAction::ToggleMute => {
                    self.vol_set_at = now;
                    self.volume.toggle_mute();
                    self.last_vol = self.volume.get();
                }
                IslandAction::OpenFile(p) => {
                    let _ = shell::launch(&p, "", false);
                }
                IslandAction::DragOut(paths) => self.modal.push(Modal::DragFiles(paths)),
                IslandAction::DragText(t) => self.modal.push(Modal::DragText(t)),
                IslandAction::CopyText(t) => {
                    self.ignore_clip_until = now + 0.6;
                    set_clipboard_text(self.hwnd_island, &t);
                }
                IslandAction::ShelfAdd(paths) => {
                    let items = paths.iter().filter(|p| shell::exists(p)).map(|p| ShelfItem::new(p, shell::file_thumb(p))).collect();
                    self.island.add_shelf_items(items);
                }
                IslandAction::SaveData => self.save_data_at = now + 0.6,
                IslandAction::SaveSettings => {
                    self.cfg.island = self.island.cfg.clone();
                    self.save_cfg_at = now + 0.4;
                    if let Some(sw) = &mut self.settings {
                        sw.panel.app = self.cfg.clone();
                        sw.panel.kick();
                    }
                }
                IslandAction::WantKeyboard(on) => self.want_keyboard(KIND_ISLAND, on),
                IslandAction::Menu { x, y } => self.modal.push(Modal::IslandMenu(x as i32, y as i32)),
                IslandAction::Sound(_) => unsafe {
                    let _ = PlaySoundW(&h("SystemNotification"), None, SND_ALIAS | SND_ASYNC | SND_NODEFAULT);
                },
                IslandAction::Control(c) => self.control(c, now),
            }
        }
    }

    // ───────── «Керування» ─────────

    /// Page open → the worker polls radios; local switches (theme, mic) are re-read every second.
    fn poll_controls(&mut self, now: f64) {
        let open = self.island.control_open();
        controls::ACTIVE.store(open, Ordering::Relaxed);
        if open && !self.ctl_was_open {
            let _ = self.ctl_tx.send(controls::Cmd::Refresh);
            self.push_controls(now);
        } else if open && now - self.ctl_local_at > 1.0 {
            self.push_controls(now);
        }
        self.ctl_was_open = open;
    }

    fn push_controls(&mut self, now: f64) {
        self.ctl_local_at = now;
        let out = controls::audio_outputs().into_iter().find(|o| o.2).map(|o| o.1).unwrap_or_default();
        let c = controls::merge(&self.ctl_snap, self.ctl_awake, self.gamma.on(), &out);
        self.island.set_controls(c);
    }

    fn control(&mut self, c: Ctl, now: f64) {
        match c {
            Ctl::Wifi(v) => {
                let _ = self.ctl_tx.send(controls::Cmd::Wifi(v));
            }
            Ctl::Bluetooth(v) => {
                let _ = self.ctl_tx.send(controls::Cmd::Bluetooth(v));
            }
            Ctl::Airplane(v) => {
                let _ = self.ctl_tx.send(controls::Cmd::Airplane(v));
            }
            Ctl::Hotspot(v) => {
                let _ = self.ctl_tx.send(controls::Cmd::Hotspot(v));
            }
            Ctl::Brightness(v) => {
                let _ = self.ctl_tx.send(controls::Cmd::Brightness(v));
            }
            Ctl::WifiMore => controls::open_uri("ms-availablenetworks:"),
            Ctl::BluetoothMore => controls::open_uri("ms-settings:bluetooth"),
            Ctl::HotspotMore => controls::open_uri("ms-settings:network-mobilehotspot"),
            Ctl::Settings => controls::open_uri("ms-settings:"),
            Ctl::Awake(v) => {
                controls::keep_awake(v);
                self.ctl_awake = v;
            }
            Ctl::Dark(v) => controls::set_dark_mode(v),
            Ctl::MicMute(m) => controls::set_mic_muted(m),
            Ctl::Night(v) => {
                if v {
                    if !self.gamma.enable() {
                        // the driver refuses gamma ramps: Windows' own night light instead
                        controls::open_uri("ms-settings:nightlight");
                    }
                } else {
                    self.gamma.disable();
                }
            }
            Ctl::Snip => {
                // let the island fold away first so it isn't in the screenshot
                std::thread::spawn(|| unsafe {
                    let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
                    std::thread::sleep(Duration::from_millis(380));
                    controls::open_uri("ms-screenclip:");
                });
            }
            Ctl::Lock => controls::lock(),
            Ctl::AudioOut { x, y } => self.modal.push(Modal::AudioMenu(x as i32, y as i32)),
        }
        self.push_controls(now);
    }

    fn handle_settings_actions(&mut self) {
        let Some(sw) = &mut self.settings else { return };
        let acts = std::mem::take(&mut sw.panel.actions);
        let hwnd = sw.hwnd;
        for a in acts {
            match a {
                SettingsAction::Changed | SettingsAction::ResetAll => {
                    if let Some(sw) = &mut self.settings {
                        // switched on just now: remember the user's own auto-hide choice to put it back later
                        if sw.panel.app.hide_taskbar && !self.cfg.hide_taskbar {
                            sw.panel.app.taskbar_prev_autohide = taskbar_autohide_get();
                        }
                        self.cfg = sw.panel.app.clone();
                    }
                    self.apply_cfg();
                }
                SettingsAction::Close => unsafe {
                    if let Some(mut sw) = self.settings.take() {
                        sw.surf.free();
                    }
                    let _ = DestroyWindow(hwnd);
                    return;
                },
                SettingsAction::StartMove => self.modal.push(Modal::MoveSettings),
                SettingsAction::OpenFolder => shell::open_folder(&appdata_dir().display().to_string()),
                SettingsAction::WantKeyboard(_) => unsafe {
                    let _ = SetFocus(hwnd);
                },
            }
        }
    }

    fn open_settings(&mut self, hinst: HINSTANCE) {
        if let Some(sw) = &self.settings {
            unsafe {
                let _ = ShowWindow(sw.hwnd, SW_SHOWNORMAL);
                let _ = SetForegroundWindow(sw.hwnd);
            }
            return;
        }
        let mut cfg = self.cfg.clone();
        cfg.autostart = autostart_get("DockX");
        cfg.island = self.island.cfg.clone();
        let mut panel = SettingsPanel::new(cfg, self.fonts.clone(), self.s);
        panel.version = env!("CARGO_PKG_VERSION").into();
        let (w, hh) = panel.size();
        let x = (self.work.cx() - w as f32 / 2.0) as i32;
        let y = (self.work.cy() - hh as f32 / 2.0) as i32;
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_APPWINDOW,
                &h("DockXWindow"),
                &h("Налаштування DockX"),
                WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX,
                x,
                y,
                w as i32,
                hh as i32,
                None,
                None,
                hinst,
                None,
            )
            .unwrap_or_default();
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, KIND_SETTINGS);
            let mut sw = SettingsWin { hwnd, panel, surf: Surface::new() };
            sw.panel.step(self.now());
            sw.surf.present(hwnd, &sw.panel.canvas, Some((x, y)), 255);
            sw.surf.last_rev = sw.panel.rendered;
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(hwnd);
            self.settings = Some(sw);
        }
    }

    fn island_menu_items(&self) -> Vec<menu::M> {
        use menu::*;
        let c = &self.island.cfg;
        let pages = self.island.pages();
        let mut p = Vec::new();
        for (i, pg) in pages.iter().enumerate() {
            p.push(item(70 + i as u32, pg.title()));
        }
        vec![
            sub("Відкрити сторінку", p),
            M::Sep,
            check(60, "Плаваюча пігулка", c.floating),
            check(61, "Мінімальний виріз", c.minimal),
            check(62, "Ефект скла", c.glass),
            check(63, "Маскот", c.mascot != 0),
            check(65, "Рядок пісні під островом", c.lyrics_pill),
            M::Sep,
            item(50, "Налаштування…"),
            item(64, "Сховати острів"),
            item(99, "Вийти з DockX"),
        ]
    }

    fn dock_menu_items(&self, idx: Option<usize>) -> Vec<menu::M> {
        use menu::*;
        let mut v = Vec::new();
        if let Some(it) = idx.and_then(|i| self.dock.items.get(i)) {
            let prog = is_program(&it.path);
            v.push(item(1, if it.running && prog { "Перейти до вікна" } else { "Відкрити" }));
            if prog {
                v.push(item(2, "Нове вікно"));
                v.push(item(3, "Запустити від імені адміністратора"));
            }
            v.push(item(4, "Показати у Провіднику"));
            v.push(item(5, "Перейменувати…"));
            v.push(M::Sep);
            v.push(item(6, "Прибрати з Dock"));
            v.push(M::Sep);
            v.push(sub("Налаштування Dock", self.dock_general()));
        } else {
            v = self.dock_general();
        }
        v
    }

    fn dock_general(&self) -> Vec<menu::M> {
        use menu::*;
        let c = &self.cfg.dock;
        let sizes = [40, 48, 56, 64, 72, 84];
        let names = ["Дуже малі", "Малі", "Середні", "Великі", "Дуже великі", "Величезні"];
        let size_items = sizes.iter().enumerate().map(|(i, v)| check(20 + i as u32, &format!("{}  ({} px)", names[i], v), (c.icon_size - v).abs() < 4)).collect();
        let mags = [1.0f32, 1.35, 1.7, 2.05];
        let mnames = ["Вимкнено", "Слабке", "Середнє", "Сильне"];
        let mag_items = mags.iter().enumerate().map(|(i, v)| check(30 + i as u32, mnames[i], (c.magnification - v).abs() < 0.1)).collect();
        vec![
            item(10, "Додати програму…"),
            item(11, "Додати папку…"),
            M::Sep,
            sub("Розмір іконок", size_items),
            sub("Збільшення при наведенні", mag_items),
            check(40, "Автоприховування", c.auto_hide),
            check(41, "Ховати в повноекранних іграх і відео", c.hide_in_fullscreen),
            check(42, "Запускати разом із Windows", autostart_get("DockX")),
            M::Sep,
            check(43, "Показувати Dynamic Island", self.island.cfg.enabled),
            item(50, "Налаштування…"),
            item(51, "Відкрити папку налаштувань"),
            item(99, "Вийти"),
        ]
    }

    /// Applies a menu choice; returns true to quit.
    fn menu_command(&mut self, id: u32, idx: Option<usize>, hinst: HINSTANCE) -> bool {
        self.cfg.island = self.island.cfg.clone();
        match id {
            1 => {
                if let Some(i) = idx {
                    self.activate(i, false, false)
                }
            }
            2 => {
                if let Some(i) = idx {
                    self.activate(i, true, false)
                }
            }
            3 => {
                if let Some(i) = idx {
                    self.activate(i, true, true)
                }
            }
            4 => {
                if let Some(it) = idx.and_then(|i| self.dock.items.get(i)) {
                    shell::reveal(&it.path.clone());
                }
            }
            5 => {
                if let Some(i) = idx {
                    self.dock.begin_rename(i);
                }
            }
            6 => {
                if let Some(i) = idx {
                    self.dock.remove_item(i);
                }
            }
            10 => self.modal.push(Modal::Pick(false)),
            11 => self.modal.push(Modal::Pick(true)),
            20..=25 => self.cfg.dock.icon_size = [40, 48, 56, 64, 72, 84][(id - 20) as usize],
            30..=33 => self.cfg.dock.magnification = [1.0, 1.35, 1.7, 2.05][(id - 30) as usize],
            40 => self.cfg.dock.auto_hide = !self.cfg.dock.auto_hide,
            41 => {
                let on = !self.cfg.dock.hide_in_fullscreen;
                self.cfg.dock.hide_in_fullscreen = on;
                self.cfg.island.hide_in_fullscreen = on;
            }
            42 => self.cfg.autostart = !autostart_get("DockX"),
            43 | 64 => self.cfg.island.enabled = !self.island.cfg.enabled,
            50 => {
                self.open_settings(hinst);
                return false;
            }
            51 => {
                shell::open_folder(&appdata_dir().display().to_string());
                return false;
            }
            60 => self.cfg.island.floating = !self.cfg.island.floating,
            61 => self.cfg.island.minimal = !self.cfg.island.minimal,
            62 => self.cfg.island.glass = !self.cfg.island.glass,
            63 => self.cfg.island.mascot = if self.island.cfg.mascot != 0 { 0 } else { 1 },
            65 => self.cfg.island.lyrics_pill = !self.cfg.island.lyrics_pill,
            70..=79 => {
                let pages = self.island.pages();
                if let Some(p) = pages.get((id - 70) as usize) {
                    self.island.open_page(*p);
                }
                return false;
            }
            99 => return true,
            _ => return false,
        }
        if (20..=65).contains(&id) {
            self.apply_cfg();
            if let Some(sw) = &mut self.settings {
                sw.panel.app = self.cfg.clone();
                sw.panel.kick();
            }
        }
        false
    }
}

// ───────────────────────── modal operations (run outside the app borrow) ─────────────────────────

fn run_modals(hinst: HINSTANCE) {
    loop {
        let Some((op, hwnd_dock, hwnd_island, s, settings_hwnd)) = with(|a| {
            if a.modal.is_empty() {
                None
            } else {
                let op = a.modal.remove(0);
                Some((op, a.hwnd_dock, a.hwnd_island, a.s, a.settings.as_ref().map(|w| w.hwnd)))
            }
        })
        .flatten() else {
            return;
        };
        match op {
            Modal::DockMenu(idx, x, y) => {
                let items = with(|a| {
                    a.menu_open = true;
                    a.dock_menu_items(idx)
                })
                .unwrap_or_default();
                let id = menu::show(hwnd_dock, x, y, &items, s);
                if std::env::var_os("DOCKX_DEBUG").is_some() {
                    log_error("menu", &format!("dock menu at {x},{y} items={} -> {id}", items.len()));
                }
                let quit = with(|a| {
                    a.menu_open = false;
                    let now = a.now();
                    a.dock.menu_closed(now);
                    a.menu_command(id, idx, hinst)
                })
                .unwrap_or(false);
                if quit {
                    quit_app();
                    return;
                }
            }
            Modal::IslandMenu(x, y) => {
                let items = with(|a| {
                    a.menu_open = true;
                    a.island_menu_items()
                })
                .unwrap_or_default();
                let id = menu::show(hwnd_island, x, y, &items, s);
                let quit = with(|a| {
                    a.menu_open = false;
                    a.menu_command(id, None, hinst)
                })
                .unwrap_or(false);
                if quit {
                    quit_app();
                    return;
                }
            }
            Modal::AudioMenu(x, y) => {
                let outs = controls::audio_outputs();
                let mut items: Vec<menu::M> = outs.iter().enumerate().map(|(i, o)| menu::check(200 + i as u32, &o.1, o.2)).collect();
                if !items.is_empty() {
                    items.push(menu::M::Sep);
                }
                items.push(menu::item(299, "Параметри звуку…"));
                with(|a| a.menu_open = true);
                let id = menu::show(hwnd_island, x, y, &items, s);
                with(|a| {
                    a.menu_open = false;
                    if id == 299 {
                        controls::open_uri("ms-settings:sound");
                    } else if id >= 200 {
                        if let Some(o) = outs.get((id - 200) as usize) {
                            controls::set_default_output(&o.0);
                            a.volume.refresh();
                            a.last_vol = a.volume.get();
                        }
                    }
                    let now = a.now();
                    a.push_controls(now);
                });
            }
            Modal::Pick(folder) => {
                let paths = shell::pick(hwnd_dock, folder);
                with(|a| {
                    let n = a.dock.items.len();
                    a.dock.actions.push(DockAction::AddPaths { paths, index: n, into_gap: false });
                });
            }
            Modal::DragFiles(paths) => {
                with(|a| a.internal_drag = true);
                let eff = dnd::drag_files_out(hwnd_island, &paths);
                with(|a| {
                    a.internal_drag = false;
                    let left = eff != DROPEFFECT_NONE || paths.iter().all(|p| !shell::exists(p));
                    a.island.drag_out_finished(&paths, left);
                });
            }
            Modal::DragText(t) => {
                with(|a| a.internal_drag = true);
                let _ = dnd::drag_text_out(hwnd_island, &t);
                with(|a| {
                    a.internal_drag = false;
                    a.island.drag_text_finished();
                });
            }
            Modal::Warn(text) => shell::message(hwnd_dock, &text),
            Modal::MoveSettings => {
                if let Some(hw) = settings_hwnd {
                    unsafe {
                        let _ = ReleaseCapture();
                        SendMessageW(hw, WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
                    }
                }
            }
        }
    }
}

/// Processes of a dock item. Shortcuts to Store apps have no .exe target, so they match by name:
/// «Claude.lnk» ↔ …\WindowsApps\…\claude.exe.
fn pids_for<'a>(running: &'a HashMap<String, Vec<u32>>, it: &DockItem) -> Option<&'a Vec<u32>> {
    if let Some(t) = &it.target {
        return running.get(&t.to_lowercase());
    }
    let low = it.path.to_lowercase();
    if !low.ends_with(".lnk") {
        return None;
    }
    let stem = std::path::Path::new(&low).file_stem()?.to_string_lossy().to_string();
    running
        .iter()
        .find(|(p, _)| std::path::Path::new(p.as_str()).file_stem().map(|f| f.to_string_lossy() == stem).unwrap_or(false))
        .map(|(_, v)| v)
}

fn quit_app() {
    with(|a| {
        a.restore_taskbar();
        a.gamma.disable();
        if a.ctl_awake {
            controls::keep_awake(false);
        }
        a.save_cfg_at = a.save_cfg_at.max(0.001);
        a.save_data_at = a.save_data_at.max(0.001);
        a.save_now();
        unsafe {
            if let Some(sw) = &a.settings {
                let _ = DestroyWindow(sw.hwnd);
            }
            let _ = DestroyWindow(a.hwnd_island);
            let _ = DestroyWindow(a.hwnd_dock);
        }
    });
    unsafe { PostQuitMessage(0) };
}

/// Runs a tick, then any modal work it queued, and keeps the frame clock running while needed.
fn pump(hinst: HINSTANCE) {
    let anim = with(|a| a.tick()).unwrap_or(false);
    run_modals(hinst);
    if anim && !ANIM.swap(true, Ordering::SeqCst) {
        let (m, cv) = &WAKE;
        *m.lock().unwrap() = true;
        cv.notify_one();
    } else if !anim {
        ANIM.store(false, Ordering::SeqCst);
    }
}

// ───────────────────────── drop sink ─────────────────────────

struct Sink;

impl DropSink for Sink {
    fn drag_over(&self, kind: DropKind, x: i32, y: i32, files: bool, allowed: DROPEFFECT) -> DROPEFFECT {
        let r = with(|a| {
            if a.internal_drag || !files {
                return DROPEFFECT_NONE;
            }
            match kind {
                DropKind::Dock => match a.dock.drag_over(x as f32, y as f32, allowed.0 & DROPEFFECT_COPY.0 != 0, allowed.0 & DROPEFFECT_LINK.0 != 0) {
                    DropEffect::None => DROPEFFECT_NONE,
                    DropEffect::Copy => DROPEFFECT_COPY,
                    DropEffect::Link => DROPEFFECT_LINK,
                },
                DropKind::Island => {
                    a.island.drag_over(x as f32, y as f32);
                    if allowed.0 & DROPEFFECT_LINK.0 != 0 {
                        DROPEFFECT_LINK
                    } else {
                        DROPEFFECT_COPY
                    }
                }
            }
        })
        .unwrap_or(DROPEFFECT_NONE);
        poke();
        r
    }
    fn drag_leave(&self, kind: DropKind) {
        with(|a| match kind {
            DropKind::Dock => a.dock.drag_leave(),
            DropKind::Island => a.island.drag_leave(),
        });
        poke();
    }
    fn drop_files(&self, kind: DropKind, files: Vec<String>) {
        with(|a| match kind {
            DropKind::Dock => a.dock.drop_files(files),
            DropKind::Island => a.island.drop_files(files),
        });
        poke();
    }
}

fn poke() {
    if !ANIM.swap(true, Ordering::SeqCst) {
        let (m, cv) = &WAKE;
        *m.lock().unwrap() = true;
        cv.notify_one();
    }
}

// ───────────────────────── window procedure ─────────────────────────

fn mods(wp: WPARAM) -> Mods {
    Mods { ctrl: wp.0 & 0x0008 != 0, shift: wp.0 & 0x0004 != 0 }
}

fn lp_xy(lp: LPARAM) -> (f32, f32) {
    ((lp.0 & 0xffff) as i16 as f32, ((lp.0 >> 16) & 0xffff) as i16 as f32)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let kind = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
    match msg {
        WM_TIMER | WM_APP_FRAME | WM_APP_WAKE => {
            if msg == WM_APP_FRAME {
                FRAME_PENDING.store(false, Ordering::SeqCst);
            }
            pump(hinst);
            return LRESULT(0);
        }
        WM_MEASUREITEM => {
            if menu::on_measure(lp) {
                return LRESULT(1);
            }
        }
        WM_DRAWITEM => {
            if menu::on_draw(lp) {
                return LRESULT(1);
            }
        }
        WM_MOUSEACTIVATE => {
            if kind == KIND_DOCK || kind == KIND_ISLAND {
                let kb = with(|a| a.kb).unwrap_or(0);
                return LRESULT(if kb == kind { MA_ACTIVATE as isize } else { MA_NOACTIVATE as isize });
            }
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let btn = match msg {
                WM_RBUTTONDOWN => Btn::Right,
                WM_MBUTTONDOWN => Btn::Middle,
                _ => Btn::Left,
            };
            if kind == KIND_SETTINGS {
                let (x, y) = lp_xy(lp);
                with(|a| {
                    if let Some(sw) = &mut a.settings {
                        sw.panel.mouse_down(btn, x, y);
                    }
                });
            } else {
                let _ = SetCapture(hwnd);
                with(|a| {
                    a.capture = true;
                    let (x, y) = a.cursor();
                    if kind == KIND_ISLAND {
                        a.island.mouse_down(btn, x, y, mods(wp), msg == WM_LBUTTONDBLCLK);
                    } else {
                        a.dock.mouse_down(btn, x, y);
                    }
                });
            }
            pump(hinst);
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
            let btn = match msg {
                WM_RBUTTONUP => Btn::Right,
                WM_MBUTTONUP => Btn::Middle,
                _ => Btn::Left,
            };
            if kind == KIND_SETTINGS {
                let (x, y) = lp_xy(lp);
                with(|a| {
                    if let Some(sw) = &mut a.settings {
                        sw.panel.mouse_up(btn, x, y);
                    }
                });
            } else {
                with(|a| a.capture = false);
                let _ = ReleaseCapture();
                with(|a| {
                    let (x, y) = a.cursor();
                    if kind == KIND_ISLAND {
                        a.island.mouse_up(btn, x, y);
                    } else {
                        a.dock.mouse_up(btn, x, y);
                    }
                });
            }
            pump(hinst);
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            if kind == KIND_SETTINGS {
                let (x, y) = lp_xy(lp);
                with(|a| {
                    if let Some(sw) = &mut a.settings {
                        sw.panel.mouse_move(x, y);
                    }
                });
            } else {
                with(|a| {
                    let (x, y) = a.cursor();
                    if kind == KIND_ISLAND {
                        a.island.mouse_move(x, y);
                    } else {
                        a.dock.mouse_move(x, y);
                    }
                });
            }
            poke();
            return LRESULT(0);
        }
        WM_CAPTURECHANGED => {
            with(|a| {
                if a.capture {
                    a.capture = false;
                    if kind == KIND_ISLAND {
                        a.island.capture_lost();
                    } else if kind == KIND_DOCK && GetKeyState(VK_LBUTTON.0 as i32) >= 0 {
                        a.dock.capture_lost();
                    }
                }
            });
            poke();
            return LRESULT(0);
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp.0 >> 16) & 0xffff) as i16 as f32 / 120.0;
            with(|a| {
                if kind == KIND_SETTINGS {
                    if let Some(sw) = &mut a.settings {
                        sw.panel.wheel(delta);
                    }
                } else {
                    let (x, y) = a.cursor();
                    a.island.wheel(delta, x, y);
                }
            });
            pump(hinst);
            return LRESULT(0);
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
            let vk = VIRTUAL_KEY(wp.0 as u16);
            let key = match vk {
                VK_BACK => Some(Key::Backspace),
                VK_DELETE => Some(Key::Delete),
                VK_LEFT => Some(Key::Left),
                VK_RIGHT => Some(Key::Right),
                VK_HOME => Some(Key::Home),
                VK_END => Some(Key::End),
                VK_RETURN => Some(Key::Enter),
                VK_ESCAPE => Some(Key::Escape),
                _ if ctrl && vk.0 == 'A' as u16 => Some(Key::SelectAll),
                _ => None,
            };
            let paste = ctrl && vk.0 == 'V' as u16;
            with(|a| {
                let clip = if paste { clipboard_text(hwnd) } else { None };
                match kind {
                    KIND_ISLAND => {
                        if let Some(k) = key {
                            a.island.key(k);
                        }
                        if let Some(c) = &clip {
                            a.island.paste(c);
                        }
                    }
                    KIND_DOCK => {
                        if let Some(k) = key {
                            a.dock.key(k);
                        }
                    }
                    _ => {
                        if let Some(sw) = &mut a.settings {
                            if let Some(k) = key {
                                sw.panel.key(k);
                            }
                            if let Some(c) = &clip {
                                sw.panel.paste(c);
                            }
                        }
                    }
                }
            });
            pump(hinst);
            return LRESULT(0);
        }
        WM_CHAR => {
            let u = wp.0 as u16;
            let ch = with(|a| {
                if (0xD800..0xDC00).contains(&u) {
                    a.surrogate = u;
                    None
                } else if (0xDC00..0xE000).contains(&u) {
                    let hi = std::mem::take(&mut a.surrogate);
                    char::decode_utf16([hi, u]).next().and_then(|r| r.ok())
                } else {
                    char::from_u32(u as u32)
                }
            })
            .flatten();
            if let Some(c) = ch {
                if !c.is_control() {
                    with(|a| match kind {
                        KIND_ISLAND => a.island.key(Key::Char(c)),
                        KIND_DOCK => a.dock.key(Key::Char(c)),
                        _ => {
                            if let Some(sw) = &mut a.settings {
                                sw.panel.key(Key::Char(c));
                            }
                        }
                    });
                }
            }
            pump(hinst);
            return LRESULT(0);
        }
        WM_KILLFOCUS => {
            with(|a| {
                if kind == KIND_ISLAND {
                    a.island.blur_todo();
                } else if kind == KIND_DOCK {
                    a.dock.end_edit(true);
                }
                if a.kb == kind {
                    a.kb = 0;
                }
            });
            poke();
        }
        WM_CLIPBOARDUPDATE => {
            with(|a| {
                if a.now() > a.ignore_clip_until {
                    if let Some(t) = clipboard_text(hwnd) {
                        a.island.clip_push(&t);
                    }
                }
            });
            poke();
            return LRESULT(0);
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DPICHANGED => {
            with(|a| {
                let (scr, work) = primary_rects();
                a.screen = scr;
                a.work = work;
                a.dock.kick();
                a.island.kick();
            });
            poke();
        }
        WM_CLOSE if kind == KIND_SETTINGS => {
            let _ = DestroyWindow(hwnd);
            return LRESULT(0);
        }
        WM_CLOSE => {
            // the installer / uninstaller asks DockX to quit: save and exit cleanly
            quit_app();
            return LRESULT(0);
        }
        WM_DESTROY => {
            if kind == KIND_SETTINGS {
                with(|a| {
                    if let Some(mut sw) = a.settings.take() {
                        sw.surf.free();
                    }
                });
            }
            return LRESULT(0);
        }
        WM_ENDSESSION => {
            with(|a| {
                a.save_cfg_at = a.save_cfg_at.max(0.001);
                a.save_data_at = a.save_data_at.max(0.001);
                a.save_now();
            });
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

// ───────────────────────── frame clock ─────────────────────────

fn frame_thread(period_ms: f64) {
    std::thread::Builder::new()
        .name("DockX frames".into())
        .spawn(move || {
            let mut hires = false;
            loop {
                if !ANIM.load(Ordering::SeqCst) {
                    if hires {
                        unsafe { timeEndPeriod(1) };
                        hires = false;
                    }
                    let (m, cv) = &WAKE;
                    let mut g = m.lock().unwrap();
                    while !*g {
                        g = cv.wait(g).unwrap();
                    }
                    *g = false;
                    continue;
                }
                if !hires {
                    unsafe { timeBeginPeriod(1) };
                    hires = true;
                }
                let t0 = Instant::now();
                let ok = unsafe { DwmFlush().is_ok() }; // blocks until the next DWM present = vsync
                let spent = t0.elapsed().as_secs_f64() * 1000.0;
                if !ok || spent < period_ms * 0.4 {
                    let wait = period_ms - spent;
                    if wait > 0.0 {
                        std::thread::sleep(Duration::from_secs_f64(wait / 1000.0));
                    }
                }
                if !FRAME_PENDING.swap(true, Ordering::SeqCst) {
                    let h = MAIN_HWND.load(Ordering::Relaxed);
                    unsafe {
                        if PostMessageW(HWND(h as *mut _), WM_APP_FRAME, WPARAM(0), LPARAM(0)).is_err() {
                            FRAME_PENDING.store(false, Ordering::SeqCst);
                        }
                    }
                }
            }
        })
        .ok();
}

fn scan_thread(tx: Sender<HashMap<String, Vec<u32>>>) {
    std::thread::Builder::new()
        .name("DockX scan".into())
        .spawn(move || loop {
            if tx.send(shell::scan_processes()).is_err() {
                return;
            }
            post_wake();
            let fast = FAST_SCAN.load(Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(if fast { 350 } else { 2000 }));
        })
        .ok();
}

// ───────────────────────── startup ─────────────────────────

fn migrate_old() {
    let dir = appdata_dir();
    let ini = dir.join("dock.ini");
    if !ini.exists() {
        let old = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("X44TO Dock").join("dock.ini");
        if old.exists() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(old, &ini);
        }
    }
    if autostart_get("X44TO Dock") {
        unsafe {
            let mut key = windows::Win32::System::Registry::HKEY::default();
            if windows::Win32::System::Registry::RegOpenKeyExW(
                windows::Win32::System::Registry::HKEY_CURRENT_USER,
                &h("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                windows::Win32::System::Registry::KEY_WRITE,
                &mut key,
            )
            .is_ok()
            {
                let _ = windows::Win32::System::Registry::RegDeleteValueW(key, &h("X44TO Dock"));
                let _ = windows::Win32::System::Registry::RegCloseKey(key);
            }
        }
        autostart_set("DockX", true);
    }
}

pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        log_error("panic", &info.to_string());
    }));
    unsafe {
        let _ = CreateMutexW(None, true, &h("DockX.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE).is_err() {
            let _ = SetProcessDPIAware();
        }
        let _ = OleInitialize(None);
        menu::enable_dark_mode();
    }
    migrate_old();
    let s = unsafe {
        let dc = GetDC(None);
        let dpi = GetDeviceCaps(dc, LOGPIXELSX);
        ReleaseDC(None, dc);
        (dpi as f32 / 96.0).max(1.0)
    };
    let fonts = Rc::new(load_fonts());
    let dir = appdata_dir();
    let ini = std::fs::read_to_string(dir.join("dock.ini")).ok();
    let (mut cfg, item_defs) = match &ini {
        Some(t) => settings::from_ini(t),
        None => (AppSettings::default(), default_items()),
    };
    cfg.autostart = autostart_get("DockX");
    let items: Vec<DockItem> = item_defs.iter().map(|(p, n)| make_item(p, n, &fonts)).collect();
    let dock = Dock::new(cfg.dock.clone(), items, fonts.clone());
    let mut island = Island::new(cfg.island.clone(), fonts.clone());
    if let Ok(t) = std::fs::read_to_string(dir.join("island.dat")) {
        let shelf = island.load_data(&t);
        for p in shelf.iter().filter(|p| shell::exists(p)) {
            island.shelf.push(ShelfItem::new(p, shell::file_thumb(p)));
        }
    }
    let (screen, work) = primary_rects();

    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(wide("DockXWindow").as_ptr()),
            ..Default::default()
        };
        let cname = wide("DockXWindow");
        let class = WNDCLASSEXW { lpszClassName: PCWSTR(cname.as_ptr()), ..class };
        RegisterClassExW(&class);
        let mk = |title: &str, kind: isize| -> HWND {
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                PCWSTR(cname.as_ptr()),
                &h(title),
                WS_POPUP,
                -32000,
                -32000,
                1,
                1,
                None,
                None,
                hinst,
                None,
            )
            .unwrap_or_default();
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, kind);
            hwnd
        };
        let hwnd_dock = mk("DockX", KIND_DOCK);
        let hwnd_island = mk("DockX Island", KIND_ISLAND);
        MAIN_HWND.store(hwnd_dock.0 as isize, Ordering::Relaxed);

        let sink: &'static Sink = Box::leak(Box::new(Sink));
        let dt_dock: IDropTarget = dnd::DropTarget::new(DropKind::Dock, sink).into();
        let dt_isl: IDropTarget = dnd::DropTarget::new(DropKind::Island, sink).into();
        let _ = RegisterDragDrop(hwnd_dock, &dt_dock);
        let _ = RegisterDragDrop(hwnd_island, &dt_isl);
        std::mem::forget(dt_dock);
        std::mem::forget(dt_isl);
        let _ = AddClipboardFormatListener(hwnd_island);

        let (media_tx, media_rx) = channel();
        let (cmd_tx, cmd_rx) = channel();
        media::spawn(media_tx, cmd_rx, post_wake);
        let (lreq_tx, lreq_rx) = channel();
        let (lres_tx, lres_rx) = channel();
        media::spawn_lyrics(lreq_rx, lres_tx, post_wake);
        let (sys_tx, sys_rx) = channel();
        sysmon::spawn_monitor(sys_tx, post_wake);
        let (scan_tx, scan_rx) = channel();
        scan_thread(scan_tx);
        let (ctl_tx, ctl_cmd_rx) = channel();
        let (ctl_snap_tx, ctl_rx) = channel();
        controls::spawn(ctl_cmd_rx, ctl_snap_tx, post_wake);

        let app = App {
            s,
            fonts,
            cfg,
            dock,
            island,
            settings: None,
            hwnd_dock,
            hwnd_island,
            surf_dock: Surface::new(),
            surf_island: Surface::new(),
            t0: Instant::now(),
            screen,
            work,
            media_rx,
            media_cmd: cmd_tx,
            lyrics_req: lreq_tx,
            lyrics_rx: lres_rx,
            sys_rx,
            scan_rx,
            running: HashMap::new(),
            art: None,
            art_id: 0,
            lyrics_key: String::new(),
            volume: sysmon::Volume::new(),
            last_vol: None,
            vol_set_at: -10.0,
            last_caps: None,
            last_layout: None,
            last_poll: -1.0,
            last_battery: -10.0,
            last_fs: -1.0,
            fullscreen: false,
            idle: 0.0,
            menu_open: false,
            internal_drag: false,
            kb: 0,
            ignore_clip_until: 0.0,
            save_cfg_at: if ini.is_none() { 0.5 } else { 0.0 },
            save_data_at: 0.0,
            modal: Vec::new(),
            capture: false,
            surrogate: 0,
            ctl_tx,
            ctl_rx,
            ctl_snap: controls::Snapshot::default(),
            ctl_awake: false,
            gamma: controls::Gamma::new(),
            ctl_was_open: false,
            ctl_local_at: -10.0,
            fs_since: -1.0,
            taskbar_hidden: false,
            last_taskbar: 0.0,
            dock_covered: false,
            last_cover: 0.0,
        };
        APP.with(|a| *a.borrow_mut() = Some(app));
        with(|a| a.apply_taskbar());

        let hz = {
            let dc = GetDC(None);
            let v = GetDeviceCaps(dc, VREFRESH);
            ReleaseDC(None, dc);
            if (30..=500).contains(&v) {
                v
            } else {
                60
            }
        };
        frame_thread(1000.0 / hz as f64);
        SetTimer(hwnd_dock, 1, 50, None);
        if std::env::args().any(|a| a == "--settings") {
            with(|a| a.open_settings(hinst));
        }
        pump(hinst);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        with(|a| {
            a.save_cfg_at = a.save_cfg_at.max(0.001);
            a.save_data_at = a.save_data_at.max(0.001);
            a.save_now();
        });
        let _ = time::local(0.0, 0);
    }
}
