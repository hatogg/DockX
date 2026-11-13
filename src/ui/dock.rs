//! DockX dock: a 1:1 port of DockX 1.1 (C#) — same springs, timings, layout and drawing.
//! Platform-independent: the Windows layer feeds input and executes `DockAction`s.

use super::anim::*;
use super::gfx::*;
use super::text_field::*;
use super::theme::*;
use super::Btn;
use std::rc::Rc;

// animation timings (seconds) — tuned to feel like the macOS dock
const MAGNIFY_TIME: f32 = 0.07;
const PRESENCE_TIME: f32 = 0.10;
const SLIDE_TIME: f32 = 0.08;
const GAP_TIME: f32 = 0.07;
const SHOW_TIME: f64 = 0.34;
const HIDE_TIME: f64 = 0.30;
const HIDE_DELAY: f64 = 0.6;
const BOUNCE_CYCLE: f64 = 0.56;
pub const POOF_TIME: f64 = 0.42;

pub fn is_program(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".exe") || p.ends_with(".lnk") || p.ends_with(".bat") || p.ends_with(".cmd")
}

pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

pub struct DockItem {
    pub path: String,
    pub name: String,
    /// resolved .exe (for the running dot and focusing)
    pub target: Option<String>,
    pub mips: Vec<Image>,
    pub running: bool,
    pub missing: bool,

    pub scale: Spring,
    pub presence: Spring,
    pub appear: Spring,
    pub offset_x: Spring,
    pub gap_before: Spring,
    pub dot: Spring,
    pub press: Spring,
    pub bounce_start: f64,
    pub bounce_cycles: i32,
    pub bounce_offset: f32,
    pub removing: bool,
    pub hidden: bool,
}

impl DockItem {
    pub fn new(path: &str, name: &str, mips: Vec<Image>) -> DockItem {
        DockItem {
            path: path.to_string(),
            name: name.to_string(),
            target: None,
            mips,
            running: false,
            missing: false,
            scale: Spring::new(1.0),
            presence: Spring::new(1.0),
            appear: Spring::new(1.0),
            offset_x: Spring::new(0.0),
            gap_before: Spring::new(0.0),
            dot: Spring::new(0.0),
            press: Spring::new(0.0),
            bounce_start: -1.0,
            bounce_cycles: 0,
            bounce_offset: 0.0,
            removing: false,
            hidden: false,
        }
    }
    pub fn is_explorer(&self) -> bool {
        let t = self.target.as_deref().unwrap_or(&self.path);
        file_name(t).eq_ignore_ascii_case("explorer.exe")
    }
}

#[derive(Clone, Debug)]
pub struct DockSettings {
    pub icon_size: i32,
    pub magnification: f32,
    pub auto_hide: bool,
    /// hide while a window covers the dock (the cursor at the bottom edge brings it back)
    pub smart_hide: bool,
    pub hide_in_fullscreen: bool,
}

impl Default for DockSettings {
    fn default() -> Self {
        DockSettings { icon_size: 52, magnification: 1.7, auto_hide: false, smart_hide: true, hide_in_fullscreen: true }
    }
}

#[derive(Clone, Debug)]
pub enum DockAction {
    Activate { idx: usize, new_instance: bool, admin: bool },
    OpenWith { idx: usize, files: Vec<String> },
    AddPaths { paths: Vec<String>, index: usize, into_gap: bool },
    Menu { idx: Option<usize>, x: f32, y: f32 },
    Save,
    /// the label became a text box: the window needs keyboard focus
    WantKeyboard(bool),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DropEffect {
    None,
    Copy,
    Link,
}

struct Poof {
    rel_x: f32,
    from_bottom: f32,
    start: f64,
}

pub struct DockEnv {
    pub now: f64,
    pub cursor: (f32, f32),
    pub work: RectF,
    pub screen: RectF,
    pub s: f32,
    /// a full-screen game/video is in front (host checks it twice a second)
    pub fullscreen: bool,
    pub menu_open: bool,
    /// the active window overlaps `rest_rect()` (host checks it a few times a second)
    pub covered: bool,
}

pub struct Dock {
    pub cfg: DockSettings,
    pub items: Vec<DockItem>,
    pub fonts: Rc<Fonts>,
    pub actions: Vec<DockAction>,
    pub canvas: Canvas,
    pub win_x: i32,
    pub win_y: i32,
    pub win_alpha: u8,
    pub win_visible: bool,
    pub rendered: u64,
    pub empty_text: String,

    s: f32,
    work: RectF,
    screen: RectF,
    // layout
    w: i32,
    h: i32,
    shelf: RectF,
    rects: Vec<RectF>,
    press_index: i32,
    press_index_any: i32,
    drag_index: i32,
    drop_insert: i32,
    drop_onto: i32,
    label_index: i32,
    label_alpha: Spring,
    end_gap: Spring,
    drag_offset_x: f32,
    drag_dy: f32,
    remove_armed: bool,
    poofs: Vec<Poof>,
    now: f64,
    // behaviour
    last_step: f64,
    last_hover: f64,
    hovering: bool,
    shown: bool,
    fs_hidden: bool,
    need_render: bool,
    hide_t: f32,
    hide_from: f32,
    hide_to: f32,
    hide_start: f64,
    icon_size: Spring,
    press_pt: (f32, f32),
    press_btn: Option<Btn>,
    dragging: bool,
    ext_drag: bool,
    cursor: (f32, f32),
    menu_open: bool,
    pub editing: Option<(usize, TextField)>,
    // shelf slices cache
    shelf_cache: Option<(f32, f32, Pixmap3)>,
}

struct Pixmap3 {
    l: tiny_skia::Pixmap,
    m: tiny_skia::Pixmap,
    r: tiny_skia::Pixmap,
    side: i32,
    ch: i32,
    m_pad: i32,
}

impl Dock {
    pub fn new(cfg: DockSettings, items: Vec<DockItem>, fonts: Rc<Fonts>) -> Dock {
        let shown = !cfg.auto_hide;
        let icon = cfg.icon_size as f32;
        Dock {
            cfg,
            items,
            fonts,
            actions: Vec::new(),
            canvas: Canvas::new(1, 1),
            win_x: 0,
            win_y: 0,
            win_alpha: 255,
            win_visible: false,
            rendered: 0,
            empty_text: "Перетягніть сюди програми або папки".into(),
            s: 1.0,
            work: rect(0.0, 0.0, 1920.0, 1040.0),
            screen: rect(0.0, 0.0, 1920.0, 1080.0),
            w: 1,
            h: 1,
            shelf: RectF::default(),
            rects: Vec::new(),
            press_index: -1,
            press_index_any: -1,
            drag_index: -1,
            drop_insert: -1,
            drop_onto: -1,
            label_index: -1,
            label_alpha: Spring::new(0.0),
            end_gap: Spring::new(0.0),
            drag_offset_x: 0.0,
            drag_dy: 0.0,
            remove_armed: false,
            poofs: Vec::new(),
            now: 0.0,
            last_step: 0.0,
            last_hover: 0.0,
            hovering: false,
            shown,
            fs_hidden: false,
            need_render: true,
            hide_t: 1.0,
            hide_from: 1.0,
            hide_to: 1.0,
            hide_start: 0.0,
            icon_size: Spring::new(icon),
            press_pt: (0.0, 0.0),
            press_btn: None,
            dragging: false,
            ext_drag: false,
            cursor: (0.0, 0.0),
            menu_open: false,
            editing: None,
            shelf_cache: None,
        }
    }

    // ───────── geometry ─────────

    fn b(&self) -> f32 {
        self.icon_size.v * self.s
    }
    fn gap(&self) -> f32 {
        8.0 * self.s
    }
    fn bottom_gap(&self) -> f32 {
        6.0 * self.s
    }
    fn center_x(&self) -> f32 {
        self.work.x + self.work.w / 2.0
    }
    fn hpad(&self) -> f32 {
        9.0 * self.s
    }
    fn vpad(&self) -> f32 {
        9.0 * self.s
    }
    fn shadow_m(&self) -> f32 {
        16.0 * self.s
    }
    fn label_room(&self) -> f32 {
        44.0 * self.s
    }
    fn live_count(&self) -> usize {
        self.items.iter().filter(|i| !i.removing).count()
    }
    fn base_shelf_width(&self) -> f32 {
        let n = self.live_count() as f32;
        if n == 0.0 {
            return (300.0 * self.s).max(self.b() * 4.0);
        }
        n * self.b() + (n - 1.0) * self.gap() + 18.0 * self.s
    }
    /// Where the shown dock sits at rest (unmagnified), in screen coordinates.
    pub fn rest_rect(&self) -> RectF {
        let w = self.base_shelf_width();
        let h = self.shelf_height();
        rect(self.center_x() - w / 2.0, self.work.b() - self.bottom_gap() - h, w, h)
    }

    /// Top edge of the visible dock (for the island to avoid it, unused) and the reserved height.
    pub fn is_shown(&self) -> bool {
        self.hide_t < 1.0
    }
    pub fn kick(&mut self) {
        self.need_render = true;
    }

    /// Item centres relative to the dock centre, before reorder offsets.
    fn centers(&self) -> (Vec<f32>, f32) {
        let n = self.items.len();
        let mut c = vec![0.0; n];
        let mut x = 0.0;
        let (b, gap) = (self.b(), self.gap());
        for (i, it) in self.items.iter().enumerate() {
            let cell = (b * it.scale.v + gap) * it.presence.v;
            x += it.gap_before.v;
            c[i] = x + cell / 2.0;
            x += cell;
        }
        x += self.end_gap.v;
        let content_w = (x - gap).max(0.0);
        let half = x / 2.0;
        for v in c.iter_mut() {
            *v -= half;
        }
        (c, content_w)
    }

    fn shelf_height(&self) -> f32 {
        (self.b() + 2.0 * self.vpad()).round()
    }

    fn layout(&mut self) {
        let n = self.items.len();
        let b = self.b();
        let (centers, content_w) = self.centers();
        let empty = self.live_count() == 0;
        let mut shelf_w = content_w + 2.0 * self.hpad();
        if empty {
            shelf_w = shelf_w.max((300.0 * self.s).max(b * 4.0));
        }
        let shelf_h = self.shelf_height();
        let sm = self.shadow_m();
        self.w = (shelf_w + 2.0 * sm).ceil() as i32;
        self.h = (self.label_room() + b * 0.55 + b * self.cfg.magnification.max(1.0) + self.vpad() + sm).ceil() as i32;
        self.shelf = rect(sm, self.h as f32 - sm - shelf_h, self.w as f32 - 2.0 * sm, shelf_h);
        if self.rects.len() != n {
            self.rects = vec![RectF::default(); n];
        }
        let mid = self.w as f32 / 2.0;
        let bottom = self.shelf.b() - self.vpad();
        for (i, it) in self.items.iter().enumerate() {
            let d = b * it.scale.v * it.presence.v.min(1.0) * ease_out_cubic(it.appear.v);
            let cx = mid + centers[i] + it.offset_x.v;
            self.rects[i] = rect(cx - d / 2.0, bottom - d - it.bounce_offset, d, d);
        }
        if self.drag_index >= 0 && (self.drag_index as usize) < n {
            let di = self.drag_index as usize;
            let r = self.rects[di];
            let cx = (mid + self.drag_offset_x).min(self.w as f32 - r.w / 2.0).max(r.w / 2.0);
            let y = (r.y + self.drag_dy.min(0.0)).max(2.0);
            self.rects[di] = rect(cx - r.w / 2.0, y, r.w, r.h);
        }
    }

    // ───────── one animation step ─────────

    /// Advances every animation and renders if needed. Returns true while frames are still needed.
    pub fn step(&mut self, env: &DockEnv) -> bool {
        let now = env.now;
        self.now = now;
        if env.s != self.s || env.work != self.work || env.screen != self.screen {
            self.s = env.s;
            self.work = env.work;
            self.screen = env.screen;
            self.need_render = true;
        }
        let dt = ((now - self.last_step).clamp(0.001, 0.05)) as f32;
        self.last_step = now;
        self.cursor = env.cursor;
        self.menu_open = env.menu_open;

        let fs = self.cfg.hide_in_fullscreen && env.fullscreen;
        if fs != self.fs_hidden {
            self.fs_hidden = fs;
            if fs {
                self.win_visible = false;
                self.hide_t = 1.0;
                self.hide_from = 1.0;
                self.hide_to = 1.0;
            } else {
                self.need_render = true;
            }
        }
        if self.fs_hidden {
            return false;
        }

        let p = env.cursor;
        let over = self.win_visible && self.is_over(p);
        let editing = self.editing.is_some();
        if over || self.dragging || self.ext_drag || self.menu_open || editing {
            self.last_hover = now;
        }

        let want_hidden = self.cfg.auto_hide || (self.cfg.smart_hide && env.covered);
        if !want_hidden {
            self.shown = true;
        } else if !self.shown && self.at_reveal_edge(p) {
            self.shown = true;
            self.last_hover = now;
        } else if self.shown && now - self.last_hover > HIDE_DELAY {
            self.shown = false;
        }

        let mut anim = false;

        // show / hide: eased slide like macOS
        let hide_target = if self.shown { 0.0 } else { 1.0 };
        if hide_target != self.hide_to {
            self.hide_from = self.hide_t;
            self.hide_to = hide_target;
            self.hide_start = now;
        }
        if self.hide_t != self.hide_to {
            let dur = if self.hide_to > self.hide_from { HIDE_TIME } else { SHOW_TIME };
            let k = ((now - self.hide_start) / dur).min(1.0) as f32;
            self.hide_t = self.hide_from + (self.hide_to - self.hide_from) * ease_in_out_cubic(k);
            if k >= 1.0 {
                self.hide_t = self.hide_to;
            }
            anim = true;
        }

        let new_hover = (over || self.dragging || self.ext_drag) && !self.menu_open && self.hide_t < 0.3 && !editing;
        if new_hover != self.hovering {
            self.hovering = new_hover;
            self.need_render = true;
        }

        anim |= self.icon_size.step(self.cfg.icon_size as f32, 0.1, dt, 0.05);
        if self.dragging {
            self.update_drag(p);
            anim = true;
        }

        let (b, gap, mag) = (self.b(), self.gap(), self.cfg.magnification);
        let live = self.live_count() as f32;
        let base_w = live * b + (live - 1.0).max(0.0) * gap;
        let cx = self.center_x();
        let mut slot = 0.0;
        let mut bouncing = false;
        let n = self.items.len();
        for i in 0..n {
            let ext = self.ext_drag && self.drop_insert == i as i32;
            let pressed = i as i32 == self.press_index;
            let it = &mut self.items[i];
            let mut target = 1.0;
            if !it.removing {
                if self.hovering && mag > 1.01 {
                    let c = slot * (b + gap) + b / 2.0 - base_w / 2.0;
                    let d = ((p.0 - cx) - c).abs();
                    let rr = b * 2.6;
                    if d < rr {
                        let t = (d / rr * std::f32::consts::FRAC_PI_2).cos();
                        target = 1.0 + (mag - 1.0) * t * t;
                    }
                }
                slot += 1.0;
            }
            anim |= it.scale.step(target, MAGNIFY_TIME, dt, 0.0005);
            anim |= it.presence.step(if it.removing { 0.0 } else { 1.0 }, PRESENCE_TIME, dt, 0.002);
            anim |= it.appear.step(1.0, 0.09, dt, 0.002);
            anim |= it.offset_x.step(0.0, SLIDE_TIME, dt, 0.2);
            anim |= it.gap_before.step(if ext { b + gap } else { 0.0 }, GAP_TIME, dt, 0.2);
            anim |= it.dot.step(if it.running { 1.0 } else { 0.0 }, 0.12, dt, 0.01);
            anim |= it.press.step(if pressed { 1.0 } else { 0.0 }, 0.035, dt, 0.01);
            let bo = bounce(it, now, b);
            if bo != 0.0 || it.bounce_offset != 0.0 || it.bounce_start >= 0.0 {
                anim = true;
                bouncing |= it.bounce_start >= 0.0;
            }
            it.bounce_offset = bo;
        }
        let end_t = if self.ext_drag && self.drop_insert == n as i32 { b + gap } else { 0.0 };
        anim |= self.end_gap.step(end_t, GAP_TIME, dt, 0.2);
        let _ = bouncing;

        // finish removals
        let mut i = self.items.len();
        while i > 0 {
            i -= 1;
            if !self.items[i].removing || self.items[i].presence.v > 0.004 {
                continue;
            }
            self.items.remove(i);
            shift_index(&mut self.label_index, i);
            shift_index(&mut self.drag_index, i);
            shift_index(&mut self.press_index, i);
            if let Some((ei, _)) = &self.editing {
                if *ei == i {
                    self.editing = None;
                }
            }
            self.rects.clear();
            anim = true;
        }

        // name label: fades in/out, switches instantly between icons
        let hi = if self.hovering && !self.dragging { self.hit_item(p) } else { -1 };
        if hi >= 0 && hi != self.label_index {
            self.label_index = hi;
            self.need_render = true;
        }
        let label_on = hi >= 0 || self.editing.is_some();
        if let Some((ei, _)) = &self.editing {
            self.label_index = *ei as i32;
        }
        anim |= self.label_alpha.step(if label_on { 1.0 } else { 0.0 }, 0.045, dt, 0.01);

        self.poofs.retain(|pf| now - pf.start <= POOF_TIME);
        if !self.poofs.is_empty() {
            anim = true;
        }
        if editing {
            anim = true; // caret blink
        }

        if anim || self.need_render {
            self.render();
        }
        anim || over || self.dragging || self.ext_drag
    }

    /// Any item still bouncing (host scans processes faster meanwhile).
    pub fn bouncing(&self) -> bool {
        self.items.iter().any(|i| i.bounce_start >= 0.0)
    }

    pub fn start_bounce(&mut self, idx: usize, new_instance: bool) {
        if let Some(it) = self.items.get_mut(idx) {
            if it.bounce_start < 0.0 && (!it.running || new_instance) {
                it.bounce_start = self.now;
                it.bounce_cycles = 1;
                if new_instance {
                    it.running = false; // keep bouncing until the scan sees it
                }
            }
        }
        self.kick();
    }

    pub fn bounce_once(&mut self, idx: usize) {
        if let Some(it) = self.items.get_mut(idx) {
            if it.bounce_start < 0.0 {
                it.bounce_start = self.now;
                it.bounce_cycles = 1;
            }
        }
    }

    fn render(&mut self) {
        self.need_render = false;
        self.rendered += 1;
        if self.hide_t >= 1.0 {
            self.win_visible = false;
            return;
        }
        self.layout();
        self.win_x = (self.center_x() - self.w as f32 / 2.0).round() as i32;
        let slide = self.hide_t * (self.shelf.h + self.bottom_gap() + self.shadow_m());
        self.win_y = (self.work.b() - self.bottom_gap() - (self.h as f32 - self.shadow_m()) + slide).round() as i32;
        let (w, h) = (self.w as u32, self.h as u32);
        let mut canvas = std::mem::replace(&mut self.canvas, Canvas::new(1, 1));
        canvas.reset(w, h);
        self.draw(&mut canvas);
        self.canvas = canvas;
        self.win_alpha = (255.0 * (1.0 - smoothstep(0.7, 1.0, self.hide_t))).round() as u8;
        self.win_visible = true;
    }

    // ───────── hit testing ─────────

    fn hit_item(&self, p: (f32, f32)) -> i32 {
        let (lx, ly) = (p.0 - self.win_x as f32, p.1 - self.win_y as f32);
        let n = self.items.len().min(self.rects.len());
        for i in 0..n {
            if i as i32 == self.drag_index || self.items[i].removing || self.items[i].hidden {
                continue;
            }
            let r = self.rects[i];
            if lx >= r.x - self.gap() / 2.0 && lx < r.r() + self.gap() / 2.0 && ly >= r.y - 4.0 * self.s && ly <= self.shelf.b() {
                return i as i32;
            }
        }
        -1
    }

    fn is_over(&self, p: (f32, f32)) -> bool {
        let (lx, ly) = (p.0 - self.win_x as f32, p.1 - self.win_y as f32);
        let sh = self.shelf;
        if lx < sh.x || lx > sh.r() || ly > sh.b() {
            return false;
        }
        if ly >= sh.y {
            return true;
        }
        if !self.hovering {
            return false;
        }
        let mut top = sh.y;
        for (i, r) in self.rects.iter().enumerate() {
            if i as i32 != self.drag_index {
                top = top.min(r.y);
            }
        }
        ly >= top - 6.0 * self.s
    }

    fn at_reveal_edge(&self, p: (f32, f32)) -> bool {
        let edge = (p.1 >= self.work.b() - 2.0 && p.1 <= self.work.b() + 1.0) || p.1 >= self.screen.b() - 1.0;
        edge && (p.0 - self.center_x()).abs() <= self.base_shelf_width() / 2.0 + 12.0 * self.s
    }

    /// Screen rectangle of item `i` as currently laid out (demo script / tooltips).
    pub fn item_rect(&self, i: usize) -> Option<RectF> {
        self.rects.get(i).map(|r| r.offset(self.win_x as f32, self.win_y as f32))
    }

    /// True if the point is over the visible dock (host uses it for click routing).
    pub fn contains_screen(&self, x: f32, y: f32) -> bool {
        self.win_visible && self.is_over((x, y))
    }

    // ───────── mouse ─────────

    pub fn mouse_down(&mut self, btn: Btn, x: f32, y: f32) {
        if self.editing.is_some() {
            self.end_edit(true);
        }
        self.press_btn = Some(btn);
        self.press_pt = (x, y);
        let hit = self.hit_item((x, y));
        self.dragging = false;
        self.remove_armed = false;
        self.press_index = if btn == Btn::Left { hit } else { -1 };
        self.press_index_any = hit;
        self.kick();
    }

    pub fn mouse_move(&mut self, x: f32, y: f32) {
        self.cursor = (x, y);
        if self.press_btn != Some(Btn::Left) || self.press_index_any < 0 {
            return;
        }
        if !self.dragging && ((x - self.press_pt.0).abs() > 6.0 * self.s || (y - self.press_pt.1).abs() > 6.0 * self.s) {
            self.dragging = true;
            self.drag_index = self.press_index_any;
            self.press_index = -1;
            self.kick();
        }
    }

    fn update_drag(&mut self, p: (f32, f32)) {
        let n = self.items.len();
        if self.drag_index < 0 || self.drag_index as usize >= n {
            return;
        }
        let (b, gap) = (self.b(), self.gap());
        let base_w = n as f32 * b + (n as f32 - 1.0) * gap;
        let cx = self.center_x();
        let idx = ((p.0 - cx + base_w / 2.0 - b / 2.0) / (b + gap)).round().clamp(0.0, (n - 1) as f32) as i32;
        if idx != self.drag_index {
            self.move_item(self.drag_index as usize, idx as usize);
        }
        self.drag_offset_x = p.0 - cx;
        self.drag_dy = p.1 - self.press_pt.1;
        self.remove_armed = self.press_pt.1 - p.1 > b * 1.25;
        self.need_render = true;
    }

    /// Reorders and gives every shifted neighbour an offset that springs back to 0 = smooth slide.
    fn move_item(&mut self, from: usize, to: usize) {
        let (before, _) = self.centers();
        let old: Vec<f32> = before;
        let moving = self.items.remove(from);
        self.items.insert(to, moving);
        // map old centres to new order
        let mut order: Vec<usize> = (0..self.items.len()).collect();
        let m = order.remove(from);
        order.insert(to, m);
        self.drag_index = to as i32;
        let (after, _) = self.centers();
        for (i, &oi) in order.iter().enumerate() {
            if i != to {
                self.items[i].offset_x.v += old[oi] - after[i];
            }
        }
    }

    pub fn mouse_up(&mut self, btn: Btn, x: f32, y: f32) {
        let p = (x, y);
        match btn {
            Btn::Left => {
                if self.dragging {
                    let di = self.drag_index;
                    if self.remove_armed && di >= 0 && (di as usize) < self.items.len() {
                        let r = self.rects.get(di as usize).copied().unwrap_or_default();
                        self.poofs.push(Poof {
                            rel_x: r.x + r.w / 2.0 - self.w as f32 / 2.0,
                            from_bottom: self.h as f32 - (r.y + r.h / 2.0),
                            start: self.now,
                        });
                        self.items[di as usize].hidden = true;
                        self.remove_item(di as usize);
                    } else if di >= 0 && (di as usize) < self.items.len() && (di as usize) < self.rects.len() {
                        // land: the icon glides from the cursor back into its slot
                        let (c, _) = self.centers();
                        let slot_x = c[di as usize];
                        let r = self.rects[di as usize];
                        self.items[di as usize].offset_x.v = (r.x + r.w / 2.0 - self.w as f32 / 2.0) - slot_x;
                    }
                    self.dragging = false;
                    self.remove_armed = false;
                    self.drag_index = -1;
                    self.actions.push(DockAction::Save);
                } else if self.press_index_any >= 0 && self.hit_item(p) == self.press_index_any {
                    self.actions.push(DockAction::Activate { idx: self.press_index_any as usize, new_instance: false, admin: false });
                }
            }
            Btn::Middle => {
                let i = self.hit_item(p);
                if i >= 0 && i == self.press_index_any {
                    self.actions.push(DockAction::Activate { idx: i as usize, new_instance: true, admin: false });
                }
            }
            Btn::Right => {
                let i = self.hit_item(p);
                self.hovering = false;
                self.actions.push(DockAction::Menu { idx: if i >= 0 { Some(i as usize) } else { None }, x, y });
            }
        }
        self.press_index = -1;
        self.press_index_any = -1;
        self.press_btn = None;
        self.kick();
    }

    pub fn capture_lost(&mut self) {
        if self.dragging {
            self.dragging = false;
            self.remove_armed = false;
            self.drag_index = -1;
            self.press_index = -1;
            self.press_index_any = -1;
            self.press_btn = None;
            self.actions.push(DockAction::Save);
            self.kick();
        }
    }

    pub fn menu_closed(&mut self, now: f64) {
        self.last_hover = now;
        self.kick();
    }

    // ───────── drag & drop from Explorer ─────────

    pub fn drag_over(&mut self, x: f32, _y: f32, allow_copy: bool, allow_link: bool) -> DropEffect {
        self.ext_drag = true;
        let lx = x - self.win_x as f32;
        let n = self.items.len().min(self.rects.len());
        let (mut onto, mut insert) = (-1, self.items.len() as i32);
        for i in 0..n {
            if self.items[i].removing {
                continue;
            }
            let r = self.rects[i];
            let it = &self.items[i];
            if lx >= r.x + r.w * 0.22 && lx <= r.r() - r.w * 0.22 && is_program(&it.path) && !it.is_explorer() {
                onto = i as i32;
                break;
            }
            if lx < r.x + r.w / 2.0 {
                insert = i as i32;
                break;
            }
        }
        if onto >= 0 {
            insert = -1;
        }
        if onto != self.drop_onto || insert != self.drop_insert {
            self.kick();
        }
        self.drop_onto = onto;
        self.drop_insert = insert;
        if onto >= 0 {
            if allow_copy {
                DropEffect::Copy
            } else {
                DropEffect::Link
            }
        } else if allow_link {
            DropEffect::Link
        } else {
            DropEffect::Copy
        }
    }

    pub fn drag_leave(&mut self) {
        self.ext_drag = false;
        self.drop_insert = -1;
        self.drop_onto = -1;
        self.kick();
    }

    pub fn drop_files(&mut self, files: Vec<String>) {
        let (onto, insert) = (self.drop_onto, self.drop_insert);
        self.drag_leave();
        if files.is_empty() {
            return;
        }
        if onto >= 0 && (onto as usize) < self.items.len() {
            self.actions.push(DockAction::OpenWith { idx: onto as usize, files });
        } else if insert >= 0 {
            self.actions.push(DockAction::AddPaths { paths: files, index: insert as usize, into_gap: true });
        } else {
            let n = self.items.len();
            self.actions.push(DockAction::AddPaths { paths: files, index: n, into_gap: false });
        }
    }

    // ───────── items ─────────

    /// Inserts prepared items (icons already loaded) — the host calls this for AddPaths.
    pub fn insert_items(&mut self, new_items: Vec<DockItem>, index: usize, into_gap: bool) {
        let mut index = index.min(self.items.len());
        let mut first = true;
        for mut it in new_items {
            if self.items.iter().any(|x| !x.removing && x.path.eq_ignore_ascii_case(&it.path)) {
                continue;
            }
            if into_gap && first {
                // the opened gap becomes the icon: full width at once, icon pops in
                if index < self.items.len() {
                    self.items[index].gap_before.snap(0.0);
                } else {
                    self.end_gap.snap(0.0);
                }
                it.presence.snap(1.0);
                it.appear.snap(0.3);
            } else {
                it.presence.snap(0.0);
                it.appear.snap(1.0);
            }
            self.items.insert(index, it);
            index += 1;
            first = false;
        }
        self.rects.clear();
        self.actions.push(DockAction::Save);
        self.kick();
    }

    pub fn remove_item(&mut self, idx: usize) {
        if let Some(it) = self.items.get_mut(idx) {
            it.removing = true; // shrinks out, then gets dropped in step()
        }
        self.actions.push(DockAction::Save);
        self.kick();
    }

    pub fn has_path(&self, path: &str) -> bool {
        self.items.iter().any(|x| !x.removing && x.path.eq_ignore_ascii_case(path))
    }

    // ───────── rename (inline, in the label pill) ─────────

    pub fn begin_rename(&mut self, idx: usize) {
        if let Some(it) = self.items.get(idx) {
            let mut f = TextField::new(&it.name, true);
            f.last_input = self.now;
            self.editing = Some((idx, f));
            self.actions.push(DockAction::WantKeyboard(true));
            self.kick();
        }
    }

    pub fn key(&mut self, k: Key) {
        let now = self.now;
        let Some((idx, f)) = &mut self.editing else { return };
        match f.key(k, now) {
            FieldResult::Submit(s) => {
                let idx = *idx;
                if !s.is_empty() {
                    if let Some(it) = self.items.get_mut(idx) {
                        it.name = s;
                    }
                    self.actions.push(DockAction::Save);
                }
                self.editing = None;
                self.actions.push(DockAction::WantKeyboard(false));
            }
            FieldResult::Cancel => {
                self.editing = None;
                self.actions.push(DockAction::WantKeyboard(false));
            }
            FieldResult::None => {}
        }
        self.kick();
    }

    pub fn end_edit(&mut self, commit: bool) {
        if self.editing.is_none() {
            return;
        }
        if commit {
            self.key(Key::Enter);
        } else {
            self.key(Key::Escape);
        }
    }

    // ───────── drawing ─────────

    fn draw(&mut self, g: &mut Canvas) {
        self.draw_shelf_cached(g);
        let n = self.items.len();
        let s = self.s;
        if self.live_count() == 0 && self.drop_insert < 0 {
            let f = &self.fonts.regular;
            let t = self.empty_text.clone();
            g.text_in(f, &t, 13.5 * s, self.shelf, Align::Center, rgba(255, 255, 255, 150));
        }
        for i in 0..n {
            let it = &self.items[i];
            if i as i32 == self.drag_index || it.hidden {
                continue;
            }
            let r = self.rects[i];
            if r.w < 1.0 {
                continue;
            }
            let mut alpha = if it.missing { 0.4 } else { 1.0 };
            if it.removing {
                alpha *= (it.presence.v * 1.5).min(1.0);
            }
            if let Some(img) = mip_for(&it.mips, r.w) {
                g.image_dimmed(img, r, alpha, 1.0 - 0.4 * it.press.v);
            }
            if it.dot.v > 0.01 {
                let d = 4.6 * s;
                let cx = r.x + r.w / 2.0;
                let cy = self.shelf.b() - self.vpad() / 2.0;
                g.fill_circle(cx, cy, d / 2.0, rgba(236, 236, 236, (230.0 * it.dot.v * alpha) as u8));
            }
        }
        if self.drop_onto >= 0 && (self.drop_onto as usize) < n {
            let r = self.rects[self.drop_onto as usize].inflate(3.0 * s, 3.0 * s);
            g.stroke_rrect(r, r.w * 0.24, ACCENT, 2.2 * s);
        }
        self.draw_poofs(g);
        if self.drag_index >= 0 && (self.drag_index as usize) < n {
            let di = self.drag_index as usize;
            let r = self.rects[di];
            if let Some(img) = mip_for(&self.items[di].mips, r.w) {
                g.image(img, r, if self.remove_armed { 0.45 } else { 0.94 });
            }
            if self.remove_armed {
                self.draw_label(g, "Відпустіть — прибрати з Dock", r, true, 1.0, None);
            }
        } else if self.label_index >= 0
            && (self.label_index as usize) < n
            && self.label_alpha.v > 0.01
            && !self.items[self.label_index as usize].removing
        {
            let li = self.label_index as usize;
            let it = &self.items[li];
            let r = self.rects[li];
            if let Some((ei, f)) = &self.editing {
                if *ei == li {
                    let (txt, caret, sel) = (f.string(), f.before_caret(), f.all_selected);
                    let vis = f.caret_visible(self.now);
                    self.draw_label(g, &txt, r, false, self.label_alpha.v, Some((caret, sel, vis)));
                    return;
                }
            }
            let mut name: String = it.name.clone();
            if name.chars().count() > 42 {
                name = name.chars().take(40).collect::<String>() + "…";
            }
            if it.missing {
                name += " (не знайдено)";
            }
            self.draw_label(g, &name, r, false, self.label_alpha.v, None);
        }
    }

    fn draw_shelf_cached(&mut self, g: &mut Canvas) {
        let s = self.s;
        let shelf_h = self.shelf.h;
        let rad = (shelf_h / 2.0).min(18.0 * s);
        let m = self.shadow_m().ceil() as i32;
        let side = m + rad.ceil() as i32 + 3;
        let cw = side * 2 + 8;
        let ch = shelf_h.ceil() as i32 + 2 * m;
        let valid = matches!(&self.shelf_cache, Some((h, sc, _)) if *h == shelf_h && *sc == s);
        if !valid {
            let mut full = Canvas::new(cw as u32, ch as u32);
            let r = rect(m as f32, m as f32, (cw - 2 * m) as f32, shelf_h);
            shadow(&mut full, r, rad, s, 1.0, false);
            if let Some(p) = rrect(r, rad) {
                panel(&mut full, &p, r, s, 1.0, false);
            }
            let inner = r.inflate(-0.5 * s, -0.5 * s);
            let _ = inner;
            full.line(r.x + rad, r.y + 1.8 * s, r.r() - rad, r.y + 1.8 * s, SHEEN, s.max(1.0));
            let slice = |x: i32, w: i32| -> tiny_skia::Pixmap {
                let mut p = tiny_skia::Pixmap::new(w as u32, ch as u32).unwrap();
                p.draw_pixmap(
                    -x,
                    0,
                    full.pm.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
                p
            };
            let p3 = Pixmap3 { l: slice(0, side), m: slice(cw / 2 - 2, 4), r: slice(cw - side, side), side, ch, m_pad: m };
            self.shelf_cache = Some((shelf_h, s, p3));
        }
        let Some((_, _, p3)) = &self.shelf_cache else { return };
        let x0 = self.shelf.x.round() as i32 - p3.m_pad;
        let x1 = self.shelf.r().round() as i32 + p3.m_pad;
        let y0 = self.shelf.y.round() as i32 - p3.m_pad;
        let mut side = p3.side;
        let mut mid_w = x1 - x0 - 2 * side;
        if mid_w < 0 {
            side = (x1 - x0) / 2;
            mid_w = x1 - x0 - 2 * side;
        }
        let q = tiny_skia::FilterQuality::Nearest;
        g.image_q(&p3.l, rect(x0 as f32, y0 as f32, side as f32, p3.ch as f32), 1.0, q);
        g.image_q(&p3.r, rect((x1 - side) as f32, y0 as f32, side as f32, p3.ch as f32), 1.0, q);
        if mid_w > 0 {
            g.image_q(&p3.m, rect((x0 + side) as f32, y0 as f32, mid_w as f32, p3.ch as f32), 1.0, q);
        }
    }

    fn draw_poofs(&self, g: &mut Canvas) {
        let b = self.b();
        for p in &self.poofs {
            let t = (self.now - p.start) / POOF_TIME;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let e = 1.0 - (1.0 - t as f32).powi(3);
            let (cx, cy) = (self.w as f32 / 2.0 + p.rel_x, self.h as f32 - p.from_bottom);
            let alpha = (215.0 * (1.0 - t).powf(1.4)) as u8;
            for k in 0..8 {
                let ang = k as f32 * std::f32::consts::TAU / 8.0 + 0.4;
                let even = k % 2 == 0;
                let dist = b * (0.08 + 0.42 * e) * if even { 1.0 } else { 0.78 };
                let rad = b * (0.16 + 0.16 * e) * if even { 1.0 } else { 0.82 };
                puff(g, cx + ang.cos() * dist, cy + ang.sin() * dist * 0.85, rad, alpha);
            }
            puff(g, cx, cy, b * (0.22 + 0.1 * e), (alpha as f32 * 0.9) as u8);
        }
    }

    fn draw_label(&self, g: &mut Canvas, text: &str, anchor: RectF, danger: bool, alpha: f32, edit: Option<(String, bool, bool)>) {
        let s = self.s;
        let f = &self.fonts.regular;
        let size = 13.5 * s;
        let tw = f.measure(text, size).max(if edit.is_some() { 60.0 * s } else { 0.0 });
        let th = size * 1.25;
        let (pw, ph) = (tw + 24.0 * s, th + 11.0 * s);
        let cx = anchor.x + anchor.w / 2.0;
        let x = (cx - pw / 2.0).min(self.w as f32 - pw - 2.0).max(2.0);
        let y = (anchor.y - ph - 9.0 * s + (1.0 - alpha) * 5.0 * s).max(2.0);
        let pr = rect(x, y, pw, ph);
        let bg = if danger { rgba(150, 24, 32, (242.0 * alpha) as u8) } else { rgba(20, 20, 23, (240.0 * alpha) as u8) };
        g.fill_rrect(pr, ph / 2.0, bg);
        let rim = if edit.is_some() { ACCENT.alpha(alpha) } else { rgba(255, 255, 255, (50.0 * alpha) as u8) };
        g.stroke_rrect(pr, ph / 2.0, rim, s.max(1.0));
        let tx = x + (pw - f.measure(text, size)) / 2.0;
        let tx = if edit.is_some() { x + 12.0 * s } else { tx };
        let base = pr.cy() + f.cap_height(size) / 2.0;
        if let Some((_, true, _)) = &edit {
            let sw = f.measure(text, size);
            g.fill_rrect(rect(tx - 2.0 * s, pr.cy() - th / 2.0, sw + 4.0 * s, th), 3.0 * s, ACCENT_DARK.alpha(alpha));
        }
        g.text(f, text, size, tx, base, rgba(242, 242, 242, (242.0 * alpha) as u8));
        if let Some((before, sel, vis)) = edit {
            if vis && !sel {
                let cxp = tx + f.measure(&before, size);
                g.fill_rect(rect(cxp, pr.cy() - th * 0.42, 1.4 * s, th * 0.84), rgba(255, 255, 255, (230.0 * alpha) as u8));
            }
        }
    }
}

fn puff(g: &mut Canvas, x: f32, y: f32, r: f32, alpha: u8) {
    if r < 1.0 || alpha == 0 {
        return;
    }
    g.radial(x, y, r, rgba(236, 236, 240, alpha), rgba(200, 200, 206, 0));
}

fn shift_index(idx: &mut i32, removed: usize) {
    let r = removed as i32;
    if *idx == r {
        *idx = -1;
    } else if *idx > r {
        *idx -= 1;
    }
}

fn bounce(it: &mut DockItem, now: f64, b: f32) -> f32 {
    if it.bounce_start < 0.0 {
        return 0.0;
    }
    let t = now - it.bounce_start;
    let k = (t / BOUNCE_CYCLE) as i32;
    if k >= it.bounce_cycles {
        // macOS keeps bouncing until the app has actually started
        let more = is_program(&it.path) && !it.running && k < 4;
        if more {
            it.bounce_cycles = k + 1;
        } else {
            it.bounce_start = -1.0;
            return 0.0;
        }
    }
    let l = (t - k as f64 * BOUNCE_CYCLE) / BOUNCE_CYCLE;
    (b as f64 * 0.5 * 4.0 * l * (1.0 - l)) as f32
}
