//! «Керування» page: quick toggles (Wi-Fi, Bluetooth, airplane mode, hotspot, keep awake, dark
//! theme, microphone, eye protection, screenshot, colour picker, lock, settings) and sliders for
//! screen brightness and volume with an audio-output switcher.
//! Platform-independent: the host feeds `Controls` and executes `Ctl` commands.

use super::*;
use tiny_skia::{LineCap, PathBuilder};

/// System state shown on the page. `None` = not available on this PC (no radio, no WMI brightness…).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Controls {
    pub wifi: Option<bool>,
    pub ssid: Option<String>,
    pub bt: Option<bool>,
    /// names of connected Bluetooth devices
    pub bt_devices: Vec<String>,
    pub airplane: Option<bool>,
    pub hotspot: Option<bool>,
    pub hotspot_clients: u32,
    pub awake: bool,
    pub dark: Option<bool>,
    pub mic_muted: Option<bool>,
    pub night: bool,
    /// 0..1
    pub brightness: Option<f32>,
    pub audio_out: String,
}

/// Commands for the host.
#[derive(Clone, Debug, PartialEq)]
pub enum Ctl {
    Wifi(bool),
    WifiMore,
    Bluetooth(bool),
    BluetoothMore,
    Airplane(bool),
    Hotspot(bool),
    HotspotMore,
    Awake(bool),
    Dark(bool),
    MicMute(bool),
    Night(bool),
    Snip,
    Picker,
    Lock,
    Settings,
    Brightness(f32),
    /// open the output-device menu at this screen point
    AudioOut { x: f32, y: f32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tile {
    Wifi,
    Bluetooth,
    Airplane,
    Hotspot,
    Awake,
    Dark,
    Mic,
    Night,
    Snip,
    Picker,
    Lock,
    Settings,
}

pub(super) const TILES: [Tile; 12] = [
    Tile::Wifi,
    Tile::Bluetooth,
    Tile::Airplane,
    Tile::Hotspot,
    Tile::Awake,
    Tile::Dark,
    Tile::Mic,
    Tile::Night,
    Tile::Snip,
    Tile::Picker,
    Tile::Lock,
    Tile::Settings,
];

impl Tile {
    /// toggle + «>» button
    fn split(self) -> bool {
        matches!(self, Tile::Wifi | Tile::Bluetooth | Tile::Hotspot)
    }
}

impl Controls {
    /// On/off of a toggle tile (`None` = unavailable). For the microphone «on» means muted.
    pub(super) fn tile(&self, t: Tile) -> Option<bool> {
        match t {
            Tile::Wifi => self.wifi,
            Tile::Bluetooth => self.bt,
            Tile::Airplane => self.airplane,
            Tile::Hotspot => self.hotspot,
            Tile::Awake => Some(self.awake),
            Tile::Dark => self.dark,
            Tile::Mic => self.mic_muted,
            Tile::Night => Some(self.night),
            Tile::Snip | Tile::Picker | Tile::Lock | Tile::Settings => Some(false),
        }
    }

    fn set_tile(&mut self, t: Tile, v: Option<bool>) {
        match t {
            Tile::Wifi => self.wifi = v,
            Tile::Bluetooth => self.bt = v,
            Tile::Airplane => self.airplane = v,
            Tile::Hotspot => self.hotspot = v,
            Tile::Awake => self.awake = v.unwrap_or(false),
            Tile::Dark => self.dark = v,
            Tile::Mic => self.mic_muted = v,
            Tile::Night => self.night = v.unwrap_or(false),
            _ => {}
        }
    }
}

/// «1 пристрій», «2 пристрої», «5 пристроїв»
fn devices_word(n: u32) -> String {
    let w = if n % 10 == 1 && n % 100 != 11 {
        "пристрій"
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        "пристрої"
    } else {
        "пристроїв"
    };
    format!("{n} {w}")
}

impl Island {
    pub fn control_open(&self) -> bool {
        self.view == View::Expanded(Page::Control)
    }

    /// New state from the host. Tiles the user just switched keep their optimistic value
    /// until the system agrees (or a few seconds pass), so nothing flickers back.
    pub fn set_controls(&mut self, mut c: Controls) {
        for (i, t) in TILES.iter().enumerate() {
            if self.ctl_pending[i] > self.now {
                let mine = self.ctl.tile(*t);
                if c.tile(*t) == mine {
                    self.ctl_pending[i] = 0.0;
                } else {
                    c.set_tile(*t, mine);
                }
            }
        }
        if self.now - self.bright_set_at < 2.0 || self.bright_drag {
            c.brightness = self.ctl.brightness;
        }
        if c != self.ctl {
            self.ctl = c;
            self.kick();
        }
    }

    fn tile_label(&self, t: Tile) -> String {
        let c = &self.ctl;
        match t {
            Tile::Wifi => match c.wifi {
                Some(false) => "Wi-Fi вимкнено".into(),
                Some(true) => c.ssid.clone().unwrap_or_else(|| "Не підключено".into()),
                None => "Wi-Fi".into(),
            },
            Tile::Bluetooth => match c.bt {
                Some(true) if !c.bt_devices.is_empty() => {
                    let more = c.bt_devices.len() - 1;
                    if more > 0 {
                        format!("{} +{}", c.bt_devices[0], more)
                    } else {
                        c.bt_devices[0].clone()
                    }
                }
                Some(false) => "Bluetooth вимк.".into(),
                _ => "Bluetooth".into(),
            },
            Tile::Airplane => "Режим польоту".into(),
            Tile::Hotspot => {
                if c.hotspot == Some(true) && c.hotspot_clients > 0 {
                    devices_word(c.hotspot_clients)
                } else {
                    "Точка доступу".into()
                }
            }
            Tile::Awake => "Не засинати".into(),
            Tile::Dark => "Темна тема".into(),
            Tile::Mic => {
                if c.mic_muted == Some(true) {
                    "Мікрофон вимк.".into()
                } else {
                    "Мікрофон".into()
                }
            }
            Tile::Night => "Захист очей".into(),
            Tile::Snip => "Знімок екрана".into(),
            Tile::Picker => "Піпетка".into(),
            Tile::Lock => "Блокування".into(),
            Tile::Settings => "Параметри".into(),
        }
    }

    /// Animates tile fills; returns true while something moves.
    pub(super) fn step_controls(&mut self, dt: f32) -> bool {
        let mut anim = false;
        for (i, t) in TILES.iter().enumerate() {
            let on = self.ctl.tile(*t) == Some(true);
            anim |= self.ctl_fill[i].step(if on { 1.0 } else { 0.0 }, 0.07, dt, 0.004);
            let pressed = self.pressed == Some(Hot::CtlTile(i as u8)) && self.hot == Some(Hot::CtlTile(i as u8));
            anim |= self.ctl_press[i].step(if pressed { 1.0 } else { 0.0 }, 0.035, dt, 0.004);
        }
        anim
    }

    pub(super) fn draw_control(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let pad = 18.0 * s;
        let gap = 9.0 * s;
        let tile_w = (r.w - 2.0 * pad - 5.0 * gap) / 6.0;
        let tile_h = 40.0 * s;
        let row_h = tile_h + 30.0 * s;
        let y0 = r.y + 14.0 * s;

        for (i, t) in TILES.iter().enumerate() {
            let (col, row) = ((i % 6) as f32, (i / 6) as f32);
            let press = self.ctl_press[i].v;
            let shrink = press * 2.0 * s;
            let tr = rect(r.x + pad + col * (tile_w + gap), y0 + row * row_h, tile_w, tile_h).inflate(-shrink, -shrink * 0.8);
            let state = self.ctl.tile(*t);
            let avail = state.is_some();
            let fill = self.ctl_fill[i].v;
            let hot_main = self.is_hot(Hot::CtlTile(i as u8));
            let hot_more = self.is_hot(Hot::CtlMore(i as u8));
            let base = CONTROL.mix(ACCENT_DARK, fill);
            let lift = if hot_main || hot_more { if fill > 0.5 { 0.22 } else { 0.0 } } else { 0.0 };
            let col_fill = if (hot_main || hot_more) && fill <= 0.5 { CONTROL_HOVER } else { base.mix(ACCENT, lift) };
            g.fill_rrect(tr, 12.0 * s, col_fill.alpha(a * if avail { 1.0 } else { 0.55 }));
            if fill > 0.01 {
                // subtle top sheen on active tiles
                g.line(tr.x + 12.0 * s, tr.y + 1.2 * s, tr.r() - 12.0 * s, tr.y + 1.2 * s, rgba(255, 255, 255, (40.0 * fill * a) as u8), s.max(1.0));
            }
            let icol = if avail { TEXT } else { TEXT3 };
            let (main, more) = if t.split() {
                let mw = tr.w - 26.0 * s;
                (rect(tr.x, tr.y, mw, tr.h), Some(rect(tr.x + mw, tr.y, tr.w - mw, tr.h)))
            } else {
                (tr, None)
            };
            if let Some(m) = more {
                if hot_more {
                    g.fill_rrect(rect(m.x + 1.0 * s, m.y + 3.0 * s, m.w - 4.0 * s, m.h - 6.0 * s), 9.0 * s, rgba(255, 255, 255, (30.0 * a) as u8));
                }
                g.line(m.x, m.y + 9.0 * s, m.x, m.b() - 9.0 * s, rgba(0, 0, 0, (90.0 * a) as u8), 1.2 * s);
                chevron(g, m.cx() - 1.0 * s, m.cy(), 4.0 * s, icol.alpha(a * 0.9), s);
                self.zone(m, Hot::CtlMore(i as u8), live);
            }
            self.zone(main, Hot::CtlTile(i as u8), live);
            tile_icon(g, *t, main.cx(), main.cy(), 17.0 * s, icol.alpha(a), s, state == Some(true));

            let label = self.tile_label(*t);
            let label = fonts.regular.ellipsize(&label, 11.5 * s, tile_w + gap - 2.0 * s);
            let lc = if fill > 0.5 { TEXT } else { TEXT2 };
            g.text_center(&fonts.regular, &label, 11.5 * s, tr.cx(), tr.b() + 17.0 * s, lc.alpha(a));
        }

        // sliders card
        let card = rect(r.x + pad, y0 + 2.0 * row_h + 2.0 * s, r.w - 2.0 * pad, 76.0 * s);
        g.fill_rrect(card, 16.0 * s, CONTROL.alpha(a));
        let track_x = card.x + 46.0 * s;

        // brightness
        let cy1 = card.y + 22.0 * s;
        let bright = self.ctl.brightness;
        let sun_col = if bright.is_some() { TEXT } else { TEXT3 };
        sun_outline(g, card.x + 22.0 * s, cy1, 6.0 * s, sun_col.alpha(a), s);
        let tr1 = rect(track_x, cy1 - 3.0 * s, card.r() - 58.0 * s - track_x, 6.0 * s);
        let bv = bright.unwrap_or(0.0);
        let hot_b = self.is_hot(Hot::Bright) || self.bright_drag;
        slider(g, tr1, bv, bright.is_some(), hot_b, a, s);
        let bt = match bright {
            Some(v) => format!("{}", (v * 100.0).round()),
            None => "—".into(),
        };
        g.text_in(&fonts.semibold, &bt, 12.5 * s, rect(tr1.r() + 8.0 * s, cy1 - 10.0 * s, card.r() - tr1.r() - 22.0 * s, 20.0 * s), Align::Right, TEXT2.alpha(a));
        if bright.is_some() {
            self.zone(rect(tr1.x - 6.0 * s, cy1 - 12.0 * s, tr1.w + 12.0 * s, 24.0 * s), Hot::Bright, live);
            // frac_in() measures against the zone: keep the zone == track
            if let Some(z) = self.zones.iter_mut().rev().find(|(_, h)| *h == Hot::Bright) {
                z.0 = rect(tr1.x, cy1 - 12.0 * s, tr1.w, 24.0 * s);
            }
        }

        // volume + output device
        let cy2 = card.y + 54.0 * s;
        let vol_icon = rect(card.x + 8.0 * s, cy2 - 12.0 * s, 28.0 * s, 24.0 * s);
        if self.is_hot(Hot::VolIcon) {
            g.fill_rrect(vol_icon, 8.0 * s, CONTROL_HOVER.alpha(a));
        }
        icons::draw(g, if self.muted || self.volume < 0.005 { Icon::SpeakerMute } else { Icon::Speaker }, vol_icon.cx(), cy2, 15.0 * s, TEXT.alpha(a));
        self.zone(vol_icon, Hot::VolIcon, live);
        let out_w = 52.0 * s;
        let out = rect(card.r() - 10.0 * s - out_w, cy2 - 13.0 * s, out_w, 26.0 * s);
        let tr2 = rect(track_x, cy2 - 3.0 * s, out.x - 44.0 * s - track_x, 6.0 * s);
        let vv = if self.muted { 0.0 } else { self.volume };
        let hot_v = self.is_hot(Hot::Volume) || self.vol_drag;
        slider(g, tr2, vv, true, hot_v, a, s);
        self.zone(rect(tr2.x, cy2 - 12.0 * s, tr2.w, 24.0 * s), Hot::Volume, live);
        let vt = format!("{}", (vv * 100.0).round());
        g.text_in(&fonts.semibold, &vt, 12.5 * s, rect(tr2.r() + 6.0 * s, cy2 - 10.0 * s, out.x - tr2.r() - 14.0 * s, 20.0 * s), Align::Right, TEXT2.alpha(a));
        let hot_o = self.is_hot(Hot::AudioOut);
        g.fill_rrect(out, 9.0 * s, if hot_o { CONTROL_HOVER } else { rgba(255, 255, 255, 14) }.alpha(a));
        headphones(g, out.x + 18.0 * s, cy2, 7.0 * s, TEXT.alpha(a), s);
        chevron(g, out.r() - 14.0 * s, cy2, 3.5 * s, TEXT2.alpha(a), s);
        self.zone(out, Hot::AudioOut, live);
        if hot_o && !self.ctl.audio_out.is_empty() {
            let name = fonts.regular.ellipsize(&self.ctl.audio_out, 11.0 * s, 260.0 * s);
            let tw = fonts.regular.measure(&name, 11.0 * s);
            let lr = rect(out.r() - tw - 16.0 * s, out.y - 25.0 * s, tw + 16.0 * s, 19.0 * s);
            g.fill_rrect(lr, lr.h / 2.0, PILL_BG.alpha(a));
            g.stroke_rrect(lr, lr.h / 2.0, RIM.alpha(a), s.max(1.0));
            g.text_in(&fonts.regular, &name, 11.0 * s, lr, Align::Center, TEXT.alpha(a));
        }
    }

    pub(super) fn control_click(&mut self, h: Hot) {
        match h {
            Hot::CtlTile(i) => {
                let Some(t) = TILES.get(i as usize).copied() else { return };
                let state = self.ctl.tile(t);
                let cmd = match t {
                    Tile::Wifi | Tile::Bluetooth | Tile::Hotspot if state.is_none() => {
                        self.expanded = false;
                        match t {
                            Tile::Wifi => Ctl::WifiMore,
                            Tile::Bluetooth => Ctl::BluetoothMore,
                            _ => Ctl::HotspotMore,
                        }
                    }
                    Tile::Snip => {
                        self.expanded = false;
                        Ctl::Snip
                    }
                    Tile::Picker => {
                        self.expanded = false;
                        Ctl::Picker
                    }
                    Tile::Lock => Ctl::Lock,
                    Tile::Settings => {
                        self.expanded = false;
                        Ctl::Settings
                    }
                    _ => {
                        let v = !state.unwrap_or(false);
                        self.ctl.set_tile(t, Some(v));
                        self.ctl_pending[i as usize] = self.now + 4.0;
                        if t == Tile::Airplane {
                            // radios follow airplane mode
                            self.ctl.wifi = self.ctl.wifi.map(|_| !v);
                            self.ctl.bt = self.ctl.bt.map(|_| !v);
                            self.ctl_pending[0] = self.now + 4.0;
                            self.ctl_pending[1] = self.now + 4.0;
                        }
                        match t {
                            Tile::Wifi => Ctl::Wifi(v),
                            Tile::Bluetooth => Ctl::Bluetooth(v),
                            Tile::Airplane => Ctl::Airplane(v),
                            Tile::Hotspot => Ctl::Hotspot(v),
                            Tile::Awake => Ctl::Awake(v),
                            Tile::Dark => Ctl::Dark(v),
                            Tile::Mic => Ctl::MicMute(v),
                            _ => Ctl::Night(v),
                        }
                    }
                };
                self.actions.push(IslandAction::Control(cmd));
            }
            Hot::CtlMore(i) => {
                let cmd = match TILES.get(i as usize) {
                    Some(Tile::Wifi) => Ctl::WifiMore,
                    Some(Tile::Bluetooth) => Ctl::BluetoothMore,
                    _ => Ctl::HotspotMore,
                };
                self.expanded = false;
                self.actions.push(IslandAction::Control(cmd));
            }
            Hot::AudioOut => {
                if let Some(z) = self.zones.iter().find(|(_, h)| *h == Hot::AudioOut).map(|(r, _)| *r) {
                    let (x, y) = (self.win_x as f32 + z.x, self.win_y as f32 + z.b() + 4.0 * self.s);
                    self.actions.push(IslandAction::Control(Ctl::AudioOut { x, y }));
                }
            }
            _ => {}
        }
        self.kick();
    }

    pub(super) fn change_brightness(&mut self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        self.ctl.brightness = Some(v);
        self.bright_set_at = self.now;
        self.actions.push(IslandAction::Control(Ctl::Brightness(v)));
        self.kick();
    }
}

// ───────────────────────── drawing helpers ─────────────────────────

fn slider(g: &mut Canvas, tr: RectF, v: f32, enabled: bool, hot: bool, a: f32, s: f32) {
    g.fill_rrect(tr, tr.h / 2.0, rgba(255, 255, 255, (34.0 * a) as u8));
    if !enabled {
        return;
    }
    let w = (tr.w * v.clamp(0.0, 1.0)).max(tr.h);
    g.fill_rrect(rect(tr.x, tr.y, w, tr.h), tr.h / 2.0, TEXT.alpha(a));
    if hot {
        let kx = tr.x + tr.w * v.clamp(0.0, 1.0);
        g.fill_circle(kx, tr.cy(), 7.5 * s, rgba(0, 0, 0, (60.0 * a) as u8));
        g.fill_circle(kx, tr.cy(), 6.5 * s, TEXT.alpha(a));
    }
}

fn stroke_pts(g: &mut Canvas, pts: &[(f32, f32)], col: Rgba, w: f32, close: bool) {
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(*x, *y);
        } else {
            pb.line_to(*x, *y);
        }
    }
    if close {
        pb.close();
    }
    if let Some(p) = pb.finish() {
        g.stroke_path_cap(&p, col, w, LineCap::Round);
    }
}

fn fill_pts(g: &mut Canvas, pts: &[(f32, f32)], col: Rgba) {
    let mut pb = PathBuilder::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            pb.move_to(*x, *y);
        } else {
            pb.line_to(*x, *y);
        }
    }
    pb.close();
    if let Some(p) = pb.finish() {
        g.fill_path(&p, col);
    }
}

fn chevron(g: &mut Canvas, cx: f32, cy: f32, k: f32, col: Rgba, s: f32) {
    stroke_pts(g, &[(cx - k * 0.5, cy - k), (cx + k * 0.5, cy), (cx - k * 0.5, cy + k)], col, 1.6 * s, false);
}

fn sun_outline(g: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, s: f32) {
    g.stroke_circle(cx, cy, r * 0.62, col, 1.6 * s);
    for k in 0..8 {
        let an = k as f32 * std::f32::consts::TAU / 8.0;
        g.line(cx + an.sin() * r * 1.05, cy - an.cos() * r * 1.05, cx + an.sin() * r * 1.4, cy - an.cos() * r * 1.4, col, 1.6 * s);
    }
}

fn headphones(g: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, s: f32) {
    g.arc(cx, cy + r * 0.2, r, -1.75, 3.5, col, 1.6 * s);
    g.fill_rrect(rect(cx - r * 1.08, cy + r * 0.05, r * 0.55, r * 0.95), r * 0.2, col);
    g.fill_rrect(rect(cx + r * 0.53, cy + r * 0.05, r * 0.55, r * 0.95), r * 0.2, col);
}

/// Sliders glyph for the page switcher.
pub(super) fn sliders_icon(g: &mut Canvas, cx: f32, cy: f32, size: f32, col: Rgba, s: f32) {
    let k = size / 2.0;
    let w = 1.5 * s;
    for (row, kx) in [(-0.55f32, 0.35f32), (0.0, -0.35), (0.55, 0.15)] {
        let y = cy + row * k;
        g.line(cx - k, y, cx + k, y, col, w);
        g.fill_circle(cx + kx * k, y, 2.6 * s, col);
        g.fill_circle(cx + kx * k, y, 1.2 * s, rgba(10, 10, 12, col.a));
    }
}

fn tile_icon(g: &mut Canvas, t: Tile, cx: f32, cy: f32, size: f32, col: Rgba, s: f32, on: bool) {
    let k = size / 2.0;
    let w = 1.7 * s;
    match t {
        Tile::Wifi => {
            for i in 1..=3 {
                let rr = k * 0.42 * i as f32;
                g.arc(cx, cy + k * 0.62, rr, -0.78, 1.56, col, w);
            }
            g.fill_circle(cx, cy + k * 0.62, 1.7 * s, col);
        }
        Tile::Bluetooth => {
            let h = k * 0.95;
            stroke_pts(
                g,
                &[(cx - k * 0.45, cy - h * 0.48), (cx + k * 0.42, cy + h * 0.45), (cx, cy + h), (cx, cy - h), (cx + k * 0.42, cy - h * 0.45), (cx - k * 0.45, cy + h * 0.48)],
                col,
                w,
                false,
            );
        }
        Tile::Airplane => {
            // plane pointing up-right
            let (sn, cs) = (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
            let rot = |x: f32, y: f32| (cx + (x * cs - y * sn) * k, cy + (x * sn + y * cs) * k);
            let body = [
                (0.0, -1.0),
                (0.14, -0.82),
                (0.14, -0.25),
                (0.95, 0.2),
                (0.95, 0.38),
                (0.14, 0.15),
                (0.14, 0.62),
                (0.42, 0.85),
                (0.42, 0.98),
                (0.0, 0.86),
                (-0.42, 0.98),
                (-0.42, 0.85),
                (-0.14, 0.62),
                (-0.14, 0.15),
                (-0.95, 0.38),
                (-0.95, 0.2),
                (-0.14, -0.25),
                (-0.14, -0.82),
            ];
            let pts: Vec<(f32, f32)> = body.iter().map(|(x, y)| rot(*x, *y)).collect();
            fill_pts(g, &pts, col);
        }
        Tile::Hotspot => {
            g.fill_circle(cx, cy - k * 0.2, 2.2 * s, col);
            g.line(cx, cy - k * 0.2, cx, cy + k * 0.95, col, w);
            for (rr, ww) in [(k * 0.5, w), (k * 0.92, w)] {
                g.arc(cx, cy - k * 0.2, rr, -2.4, 1.3, col, ww);
                g.arc(cx, cy - k * 0.2, rr, 1.1, 1.3, col, ww);
            }
        }
        Tile::Awake => {
            let cup = rect(cx - k * 0.62, cy - k * 0.1, k * 1.0, k * 0.85);
            if let Some(p) = rrect4(cup, 1.0 * s, 1.0 * s, k * 0.35, k * 0.35) {
                if on {
                    g.fill_path(&p, col);
                } else {
                    g.stroke_path(&p, col, w);
                }
            }
            g.arc(cup.r(), cup.y + cup.h * 0.38, k * 0.24, 0.0, 3.14, col, w);
            for dx in [-0.32f32, 0.0, 0.32] {
                let x = cx - k * 0.12 + dx * k;
                stroke_pts(g, &[(x, cy - k * 0.3), (x + k * 0.1, cy - k * 0.55), (x, cy - k * 0.8)], col, 1.3 * s, false);
            }
        }
        Tile::Dark => {
            let r = k * 0.78;
            g.stroke_circle(cx, cy, r, col, w);
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy - r);
            for i in 0..=24 {
                let an = std::f32::consts::PI * i as f32 / 24.0;
                pb.line_to(cx - an.sin() * r, cy - an.cos() * r);
            }
            pb.close();
            if let Some(p) = pb.finish() {
                g.fill_path(&p, col);
            }
        }
        Tile::Mic => {
            let cap = rect(cx - k * 0.3, cy - k * 0.95, k * 0.6, k * 1.15);
            g.fill_rrect(cap, k * 0.3, col);
            g.arc(cx, cy - k * 0.2, k * 0.62, 1.75, 2.8, col, w);
            g.line(cx, cy + k * 0.45, cx, cy + k * 0.85, col, w);
            g.line(cx - k * 0.3, cy + k * 0.88, cx + k * 0.3, cy + k * 0.88, col, w);
            if on {
                g.line(cx - k * 0.8, cy - k * 0.8, cx + k * 0.8, cy + k * 0.8, rgba(14, 14, 16, col.a), w * 2.6);
                g.line(cx - k * 0.8, cy - k * 0.8, cx + k * 0.8, cy + k * 0.8, col, w);
            }
        }
        Tile::Night => {
            // eye
            let mut pb = PathBuilder::new();
            pb.move_to(cx - k * 0.95, cy);
            pb.quad_to(cx, cy - k * 1.05, cx + k * 0.95, cy);
            pb.quad_to(cx, cy + k * 1.05, cx - k * 0.95, cy);
            pb.close();
            if let Some(p) = pb.finish() {
                g.stroke_path(&p, col, w);
            }
            g.fill_circle(cx, cy, k * 0.3, col);
        }
        Tile::Snip => {
            stroke_pts(g, &[(cx - k * 0.5, cy - k * 0.95), (cx - k * 0.5, cy + k * 0.5), (cx + k * 0.95, cy + k * 0.5)], col, w, false);
            stroke_pts(g, &[(cx - k * 0.95, cy - k * 0.5), (cx + k * 0.5, cy - k * 0.5), (cx + k * 0.5, cy + k * 0.95)], col, w, false);
        }
        Tile::Picker => {
            // pipette, tip down-left
            let (dx, dy) = (std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2);
            let p0 = (cx - k * 0.85, cy + k * 0.85);
            let p1 = (cx + dx * k * 0.25, cy + dy * k * 0.25);
            g.line(p0.0, p0.1, p1.0, p1.1, col, w * 1.4);
            g.line(cx + dx * k * 0.2 - dy * k * 0.35, cy + dy * k * 0.2 + dx * k * 0.35, cx + dx * k * 0.2 + dy * k * 0.35, cy + dy * k * 0.2 - dx * k * 0.35, col, w * 1.2);
            let bulb = (cx + dx * k * 0.62, cy + dy * k * 0.62);
            g.fill_circle(bulb.0, bulb.1, k * 0.36, col);
        }
        Tile::Lock => {
            let body = rect(cx - k * 0.68, cy - k * 0.12, k * 1.36, k * 1.02);
            g.fill_rrect(body, k * 0.2, col);
            g.arc(cx, cy - k * 0.15, k * 0.42, -1.5708, 3.1416, col, w * 1.15);
            g.line(cx - k * 0.42, cy - k * 0.15, cx - k * 0.42, cy - k * 0.05, col, w * 1.15);
            g.line(cx + k * 0.42, cy - k * 0.15, cx + k * 0.42, cy - k * 0.05, col, w * 1.15);
            g.fill_circle(cx, cy + k * 0.35, k * 0.14, rgba(14, 14, 16, col.a));
        }
        Tile::Settings => {
            let r = k * 0.62;
            for i in 0..8 {
                let an = i as f32 * std::f32::consts::TAU / 8.0;
                g.line(cx + an.sin() * r, cy - an.cos() * r, cx + an.sin() * k * 0.95, cy - an.cos() * k * 0.95, col, w * 1.6);
            }
            g.fill_circle(cx, cy, r, col);
            g.fill_circle(cx, cy, r * 0.42, rgba(14, 14, 16, col.a));
        }
    }
}
