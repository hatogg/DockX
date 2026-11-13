//! Dynamic Island in DockX's black-glass style.
//! Platform-independent: the host feeds input, media, battery and system events, and executes `IslandAction`s.

mod pages;

use super::anim::*;
use super::gfx::*;
use super::icons::{self, Icon};
use super::lrc::{self, LrcLine};
use super::mascot::{self, Mood};
use super::text_field::*;
use super::theme::*;
use super::time::{self, Local};
use super::{prayer, Btn, Mods};
use std::rc::Rc;

// ───────────────────────── data ─────────────────────────

#[derive(Clone, Default)]
pub struct MediaInfo {
    pub has: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app: String,
    pub art: Option<Image>,
    pub playing: bool,
    /// seconds at `sampled_at` (island clock)
    pub position: f64,
    pub duration: f64,
    pub sampled_at: f64,
    pub can_prev: bool,
    pub can_next: bool,
    /// all players Windows knows about (app names) and which one is shown
    pub players: Vec<String>,
    pub player_idx: usize,
}

impl MediaInfo {
    pub fn key(&self) -> String {
        format!("{}\u{1}{}", self.title, self.artist)
    }
    pub fn pos_at(&self, now: f64) -> f64 {
        let p = if self.playing { self.position + (now - self.sampled_at) } else { self.position };
        if self.duration > 0.0 {
            p.clamp(0.0, self.duration)
        } else {
            p.max(0.0)
        }
    }
}

#[derive(Clone)]
pub enum Lyrics {
    None,
    Loading,
    NotFound,
    Synced(Rc<Vec<LrcLine>>),
    Plain(Rc<Vec<String>>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub plugged: bool,
}

pub struct ShelfItem {
    pub path: String,
    pub name: String,
    pub icon: Option<Image>,
    pub selected: bool,
    appear: Spring,
}

impl ShelfItem {
    pub fn new(path: &str, icon: Option<Image>) -> ShelfItem {
        let name = super::dock::file_name(path).to_string();
        ShelfItem { path: path.to_string(), name, icon, selected: false, appear: Spring::new(0.0) }
    }
}

pub struct ClipItem {
    pub text: String,
    pub unix: f64,
    appear: Spring,
}

pub struct TodoItem {
    pub text: String,
    pub done: bool,
    /// day number (days since 1970) the task belongs to
    pub day: i64,
    /// reminder, minutes of that day
    pub remind: Option<u32>,
    pub fired: bool,
    appear: Spring,
    check: Spring,
    offset: Spring,
    removing: bool,
}

impl TodoItem {
    pub fn new(text: &str, done: bool, day: i64, remind: Option<u32>) -> TodoItem {
        TodoItem {
            text: text.into(),
            done,
            day,
            remind,
            fired: false,
            appear: Spring::new(1.0),
            check: Spring::new(if done { 1.0 } else { 0.0 }),
            offset: Spring::new(0.0),
            removing: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct IslandSettings {
    pub enabled: bool,
    /// 0 small, 1 medium, 2 large
    pub size: u8,
    /// floating pill (detached from the edge) instead of a notch
    pub floating: bool,
    /// thin line while nothing happens
    pub minimal: bool,
    /// translucent glass panel
    pub glass: bool,
    pub clock: bool,
    /// collapsed status ring: 0 off, 1 battery, 2 water
    pub ring: u8,
    /// 0 off, 1 «Іскра», 2 eyes only
    pub mascot: u8,
    pub lyrics_pill: bool,
    pub lyrics_online: bool,
    pub hide_in_fullscreen: bool,
    pub clip_on: bool,
    pub clip_max: u32,
    pub water_on: bool,
    pub water_interval: u32,
    pub cup_ml: u32,
    pub goal_ml: u32,
    pub focus_min: u32,
    pub break_min: u32,
    pub long_break_min: u32,
    pub battery_notify: bool,
    pub battery_low: bool,
    pub prayer_on: bool,
    pub prayer_city: u8,
    pub prayer_method: u8,
    pub prayer_hanafi: bool,
    pub prayer_remind: bool,
    pub prayer_pause: bool,
    pub widget_a: u8,
    pub widget_b: u8,
    pub world_city: u8,
    pub countdown_title: String,
    pub countdown_date: String,
    pub hud_volume: bool,
    pub hud_brightness: bool,
    pub hud_caps: bool,
    pub hud_layout: bool,
    pub card_wifi: bool,
    pub card_audio: bool,
}

impl Default for IslandSettings {
    fn default() -> Self {
        IslandSettings {
            enabled: true,
            size: 1,
            floating: false,
            minimal: false,
            glass: false,
            clock: true,
            ring: 1,
            mascot: 1,
            lyrics_pill: true,
            lyrics_online: true,
            hide_in_fullscreen: true,
            clip_on: true,
            clip_max: 20,
            water_on: true,
            water_interval: 60,
            cup_ml: 250,
            goal_ml: 2000,
            focus_min: 25,
            break_min: 5,
            long_break_min: 15,
            battery_notify: true,
            battery_low: true,
            prayer_on: false,
            prayer_city: 0,
            prayer_method: 0,
            prayer_hanafi: false,
            prayer_remind: true,
            prayer_pause: true,
            widget_a: 0,
            widget_b: 2,
            world_city: 14,
            countdown_title: "Новий рік".into(),
            countdown_date: "2027-01-01".into(),
            hud_volume: true,
            hud_brightness: true,
            hud_caps: true,
            hud_layout: true,
            card_wifi: true,
            card_audio: true,
        }
    }
}

pub const WIDGETS: [&str; 7] = ["Календар тижня", "Світовий час", "Фаза місяця", "Зворотний відлік", "Батарея", "Вода", "Фокус"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Today,
    Music,
    Shelf,
    Focus,
    Water,
    Todo,
    Widgets,
    Prayer,
}

impl Page {
    fn icon(self) -> Icon {
        match self {
            Page::Today => Icon::Bell,
            Page::Music => Icon::Music,
            Page::Shelf => Icon::Shelf,
            Page::Focus => Icon::Timer,
            Page::Water => Icon::Drop,
            Page::Todo => Icon::List,
            Page::Widgets => Icon::Select,
            Page::Prayer => Icon::Lyrics,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Page::Today => "Сьогодні",
            Page::Music => "Музика",
            Page::Shelf => "Полиця",
            Page::Focus => "Фокус",
            Page::Water => "Вода",
            Page::Todo => "Справи",
            Page::Widgets => "Віджети",
            Page::Prayer => "Молитва",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusMode {
    Focus,
    Break,
    LongBreak,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoticeKind {
    Water,
    FocusDone,
    BreakDone,
    LowBattery,
    Todo,
    Prayer,
    // compact cards
    Charging,
    WifiOn,
    WifiOff,
    Headphones,
}

impl NoticeKind {
    fn compact(self) -> bool {
        matches!(self, NoticeKind::Charging | NoticeKind::WifiOn | NoticeKind::WifiOff | NoticeKind::Headphones)
    }
}

struct Notice {
    kind: NoticeKind,
    text: String,
    start: f64,
    hovered_time: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Hud {
    Volume(f32, bool),
    Brightness(f32),
    Caps(bool),
    Layout(String),
}

#[derive(Clone, Debug)]
pub enum IslandAction {
    MediaToggle,
    MediaPause,
    MediaNext,
    MediaPrev,
    MediaSeek(f64),
    NextPlayer,
    SetVolume(f32),
    ToggleMute,
    OpenFile(String),
    DragOut(Vec<String>),
    DragText(String),
    CopyText(String),
    ShelfAdd(Vec<String>),
    SaveData,
    SaveSettings,
    WantKeyboard(bool),
    Menu { x: f32, y: f32 },
    Sound(NoticeKind),
}

pub struct IslandEnv {
    pub now: f64,
    pub cursor: (f32, f32),
    /// primary monitor bounds
    pub screen: RectF,
    pub s: f32,
    pub fullscreen: bool,
    pub menu_open: bool,
    /// wall clock (UTC seconds) and local offset
    pub unix: f64,
    pub tz_min: i32,
    /// seconds since the last keyboard/mouse input
    pub idle_secs: f64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hot {
    PlayPause,
    Next,
    Prev,
    Seek,
    Volume,
    VolIcon,
    Art,
    LyricsBtn,
    Line(usize),
    Page(usize),
    Dots,
    Tab(u8),
    Tile(usize),
    TileX(usize),
    ShelfSelect,
    ShelfRemove,
    ShelfClear,
    Clip(usize),
    ClipX(usize),
    FocusStart,
    FocusReset,
    FocusSkip,
    WaterDrink,
    WaterUndo,
    WaterToggle,
    TodoRow(usize),
    TodoDel(usize),
    TodoBell(usize),
    TodoInput,
    Day(i64),
    WeekPrev,
    WeekNext,
    NoticeA,
    NoticeB,
    Widget(u8),
    WidgetGo(u8),
    Panel,
}

impl Hot {
    fn name(&self) -> String {
        match self {
            Hot::PlayPause => "play".into(),
            Hot::Next => "next".into(),
            Hot::Prev => "prev".into(),
            Hot::Seek => "seek".into(),
            Hot::Volume => "volume".into(),
            Hot::VolIcon => "mute".into(),
            Hot::Art => "art".into(),
            Hot::LyricsBtn => "lyrics_btn".into(),
            Hot::Line(i) => format!("line{i}"),
            Hot::Page(i) => format!("page{i}"),
            Hot::Dots => "dots".into(),
            Hot::Tab(i) => format!("tab{i}"),
            Hot::Tile(i) => format!("tile{i}"),
            Hot::TileX(i) => format!("tilex{i}"),
            Hot::ShelfSelect => "select".into(),
            Hot::ShelfRemove => "remove".into(),
            Hot::ShelfClear => "clear".into(),
            Hot::Clip(i) => format!("clip{i}"),
            Hot::ClipX(i) => format!("clipx{i}"),
            Hot::FocusStart => "focus_start".into(),
            Hot::FocusReset => "focus_reset".into(),
            Hot::FocusSkip => "focus_skip".into(),
            Hot::WaterDrink => "drink".into(),
            Hot::WaterUndo => "water_undo".into(),
            Hot::WaterToggle => "water_toggle".into(),
            Hot::TodoRow(i) => format!("todo{i}"),
            Hot::TodoDel(i) => format!("tododel{i}"),
            Hot::TodoBell(i) => format!("todobell{i}"),
            Hot::TodoInput => "todo_input".into(),
            Hot::Day(i) => format!("day{i}"),
            Hot::WeekPrev => "week_prev".into(),
            Hot::WeekNext => "week_next".into(),
            Hot::NoticeA => "noticeA".into(),
            Hot::NoticeB => "noticeB".into(),
            Hot::Widget(i) => format!("widget{i}"),
            Hot::WidgetGo(i) => format!("widgetgo{i}"),
            Hot::Panel => "panel".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    Hidden,
    Collapsed,
    Expanded(Page),
    Notice,
    Hud,
    Drop,
}

struct Event {
    unix: f64,
    icon: Icon,
    text: String,
}

pub struct Island {
    pub cfg: IslandSettings,
    pub fonts: Rc<Fonts>,
    pub actions: Vec<IslandAction>,
    pub canvas: Canvas,
    pub win_x: i32,
    pub win_y: i32,
    pub win_visible: bool,
    pub rendered: u64,

    pub media: MediaInfo,
    pub lyrics: Lyrics,
    pub volume: f32,
    pub muted: bool,
    pub shelf: Vec<ShelfItem>,
    pub clips: Vec<ClipItem>,
    pub todos: Vec<TodoItem>,
    /// (local hour as f32, ml) of today's drinks
    pub drinks: Vec<(f32, u32)>,
    pub focus_done: u32,
    pub data_day: i64,
    pub battery: Option<Battery>,
    low_warned: u8,

    pub focus_mode: FocusMode,
    pub focus_running: bool,
    focus_remaining: f64,
    focus_ends: f64,
    focus_round: u32,

    water_due: f64,
    notices: Vec<Notice>,
    hud: Option<(Hud, f64)>,
    events: Vec<Event>,
    prayer_fired: [i64; 6],

    s: f32,
    screen: RectF,
    now: f64,
    unix: f64,
    tz_min: i32,
    idle_secs: f64,
    lt: Local,
    last_step: f64,
    started: bool,
    view: View,
    prev_view: View,
    view_changed: f64,
    pub page: Page,
    expanded: bool,
    hover_since: f64,
    leave_since: f64,
    ext_drag: bool,
    fs_hidden: bool,
    need_render: bool,
    last_render: f64,

    w: Bouncy,
    h: Bouncy,
    rad: Spring,
    top: Spring,
    ear: Spring,
    split: Spring,
    shape: RectF,
    split_rects: Option<(RectF, RectF)>,

    zones: Vec<(RectF, Hot)>,
    hot: Option<Hot>,
    pressed: Option<Hot>,
    press_pt: (f32, f32),
    scrub: Option<f32>,
    vol_drag: bool,
    drag_src: Option<(Hot, (f32, f32))>,
    dragging_out: bool,
    reorder: Option<(usize, f32)>,
    shelf_tab: u8,
    shelf_select: bool,
    shelf_anchor: Option<usize>,
    shelf_scroll: Spring,
    clip_scroll: f32,
    copied_flash: (usize, f64),
    todo_field: Option<TextField>,
    todo_draft: String,
    todo_scroll: Spring,
    todo_scroll_raw: f32,
    todo_day: i64,
    dots_open: Spring,
    pub lyrics_open: bool,
    lyric_scroll: Spring,
    lyric_idx: i32,
    pill_w: Spring,
    pill_line: String,
    pill_prev: String,
    pill_change: f64,
    art_color: Rgba,
    art_ptr: usize,
    happy_until: f64,
    greet_until: f64,
    mascot_on: bool,
    active_since: f64,
    hold_until: f64,
    cursor_local: (f32, f32),
    vol_flash: f64,
}

impl Island {
    pub fn new(cfg: IslandSettings, fonts: Rc<Fonts>) -> Island {
        Island {
            cfg,
            fonts,
            actions: Vec::new(),
            canvas: Canvas::new(1, 1),
            win_x: 0,
            win_y: 0,
            win_visible: false,
            rendered: 0,
            media: MediaInfo::default(),
            lyrics: Lyrics::None,
            volume: 0.5,
            muted: false,
            shelf: Vec::new(),
            clips: Vec::new(),
            todos: Vec::new(),
            drinks: Vec::new(),
            focus_done: 0,
            data_day: 0,
            battery: None,
            low_warned: 100,
            focus_mode: FocusMode::Focus,
            focus_running: false,
            focus_remaining: 25.0 * 60.0,
            focus_ends: 0.0,
            focus_round: 0,
            water_due: 0.0,
            notices: Vec::new(),
            hud: None,
            events: Vec::new(),
            prayer_fired: [0; 6],
            s: 1.0,
            screen: rect(0.0, 0.0, 1920.0, 1080.0),
            now: 0.0,
            unix: 0.0,
            tz_min: 0,
            idle_secs: 0.0,
            lt: time::local(0.0, 0),
            last_step: 0.0,
            started: false,
            view: View::Collapsed,
            prev_view: View::Collapsed,
            view_changed: -10.0,
            page: Page::Music,
            expanded: false,
            hover_since: 0.0,
            leave_since: 0.0,
            ext_drag: false,
            fs_hidden: false,
            need_render: true,
            last_render: -1.0,
            w: Bouncy::new(0.0),
            h: Bouncy::new(0.0),
            rad: Spring::new(0.0),
            top: Spring::new(0.0),
            ear: Spring::new(0.0),
            split: Spring::new(0.0),
            shape: RectF::default(),
            split_rects: None,
            zones: Vec::new(),
            hot: None,
            pressed: None,
            press_pt: (0.0, 0.0),
            scrub: None,
            vol_drag: false,
            drag_src: None,
            dragging_out: false,
            reorder: None,
            shelf_tab: 0,
            shelf_select: false,
            shelf_anchor: None,
            shelf_scroll: Spring::new(0.0),
            clip_scroll: 0.0,
            copied_flash: (usize::MAX, -10.0),
            todo_field: None,
            todo_draft: String::new(),
            todo_scroll: Spring::new(0.0),
            todo_scroll_raw: 0.0,
            todo_day: 0,
            dots_open: Spring::new(0.0),
            lyrics_open: true,
            lyric_scroll: Spring::new(0.0),
            lyric_idx: -1,
            pill_w: Spring::new(0.0),
            pill_line: String::new(),
            pill_prev: String::new(),
            pill_change: -10.0,
            art_color: ACCENT,
            art_ptr: 0,
            happy_until: 0.0,
            greet_until: 0.0,
            mascot_on: false,
            active_since: 0.0,
            hold_until: 0.0,
            cursor_local: (-1000.0, -1000.0),
            vol_flash: -10.0,
        }
    }

    pub fn kick(&mut self) {
        self.need_render = true;
    }

    pub fn pages(&self) -> Vec<Page> {
        let mut v = vec![Page::Today, Page::Music, Page::Shelf, Page::Focus, Page::Water, Page::Todo, Page::Widgets];
        if self.cfg.prayer_on {
            v.push(Page::Prayer);
        }
        v
    }

    fn log(&mut self, icon: Icon, text: String) {
        self.events.insert(0, Event { unix: self.unix, icon, text });
        self.events.truncate(30);
    }

    // ───────── host → island ─────────

    pub fn set_media(&mut self, m: MediaInfo) {
        let changed = m.key() != self.media.key() || m.has != self.media.has;
        let ptr = m.art.as_ref().map(|a| Rc::as_ptr(a) as usize).unwrap_or(0);
        if ptr != self.art_ptr {
            self.art_ptr = ptr;
            self.art_color = m.art.as_ref().map(|a| dominant(a)).unwrap_or(ACCENT);
        }
        if changed {
            self.lyrics = Lyrics::None;
            self.lyric_idx = -1;
            self.lyric_scroll.snap(0.0);
            if m.has && !m.title.is_empty() {
                let t = if m.artist.is_empty() { m.title.clone() } else { format!("{} — {}", m.artist, m.title) };
                self.log(Icon::Music, t);
            }
        }
        self.media = m;
        self.kick();
    }

    pub fn set_lyrics(&mut self, key: &str, l: Lyrics) {
        if key == self.media.key() {
            self.lyrics = l;
            self.lyric_idx = -1;
            self.kick();
        }
    }

    /// Volume from the system mixer; `show_hud` when it changed outside the island.
    pub fn set_volume(&mut self, v: f32, muted: bool, show_hud: bool) {
        if (v - self.volume).abs() > 0.001 || muted != self.muted {
            self.volume = v;
            self.muted = muted;
            if show_hud && self.cfg.hud_volume && self.now - self.vol_flash > 0.6 {
                self.hud = Some((Hud::Volume(v, muted), self.now));
            }
            self.kick();
        }
    }

    /// Brightness / Caps Lock / keyboard layout changes noticed by the host.
    pub fn show_hud(&mut self, h: Hud) {
        let on = match &h {
            Hud::Volume(..) => self.cfg.hud_volume,
            Hud::Brightness(_) => self.cfg.hud_brightness,
            Hud::Caps(_) => self.cfg.hud_caps,
            Hud::Layout(_) => self.cfg.hud_layout,
        };
        if on {
            self.hud = Some((h, self.now));
            self.kick();
        }
    }

    /// Wi-Fi connected (Some(ssid)) / lost (None).
    pub fn wifi_changed(&mut self, ssid: Option<String>) {
        let (kind, text) = match ssid {
            Some(s) => (NoticeKind::WifiOn, s),
            None => (NoticeKind::WifiOff, String::new()),
        };
        self.log(Icon::Bell, if kind == NoticeKind::WifiOn { format!("Wi-Fi: {}", text) } else { "Wi-Fi відключено".into() });
        if self.cfg.card_wifi {
            self.push_notice(kind, text);
        }
    }

    /// A new default audio output (e.g. Bluetooth headphones) appeared.
    pub fn audio_device(&mut self, name: &str) {
        self.log(Icon::Speaker, format!("Звук: {}", name));
        if self.cfg.card_audio {
            self.push_notice(NoticeKind::Headphones, name.to_string());
        }
    }

    /// None on desktops without a battery.
    pub fn set_battery(&mut self, b: Option<Battery>) {
        if b == self.battery {
            return;
        }
        let prev = self.battery;
        self.battery = b;
        if let (Some(p), Some(n)) = (prev, b) {
            if n.plugged && !p.plugged {
                self.log(Icon::Bell, format!("Зарядка підключена · {}%", n.percent));
                if self.cfg.battery_notify {
                    self.push_notice(NoticeKind::Charging, String::new());
                }
            }
            if n.plugged {
                self.low_warned = 100;
                self.notices.retain(|x| x.kind != NoticeKind::LowBattery);
            }
            if !n.plugged && !p.plugged && self.cfg.battery_low {
                for lvl in [20u8, 10] {
                    if n.percent <= lvl && p.percent > lvl && self.low_warned > lvl {
                        self.low_warned = lvl;
                        self.push_notice(NoticeKind::LowBattery, String::new());
                    }
                }
            }
        }
        self.kick();
    }

    pub fn clip_push(&mut self, text: &str) {
        if !self.cfg.clip_on {
            return;
        }
        let t = text.trim();
        if t.is_empty() || t.len() > 20000 {
            return;
        }
        self.clips.retain(|c| c.text != t);
        self.clips.insert(0, ClipItem { text: t.to_string(), unix: self.unix, appear: Spring::new(0.0) });
        self.clips.truncate(self.cfg.clip_max.max(1) as usize);
        self.kick();
    }

    pub fn add_shelf_items(&mut self, items: Vec<ShelfItem>) {
        let n = items.len();
        for it in items {
            if !self.shelf.iter().any(|x| x.path.eq_ignore_ascii_case(&it.path)) {
                self.shelf.push(it);
            }
        }
        if n > 0 {
            self.log(Icon::Shelf, format!("На полиці: +{} файл(и)", n));
        }
        self.actions.push(IslandAction::SaveData);
        self.kick();
    }

    /// After a drag out of the shelf: files that really left are removed, cancelled ones stay.
    pub fn drag_out_finished(&mut self, paths: &[String], left: bool) {
        self.dragging_out = false;
        if left {
            self.shelf.retain(|x| !paths.iter().any(|p| p.eq_ignore_ascii_case(&x.path)));
            self.shelf_anchor = None;
            self.actions.push(IslandAction::SaveData);
        }
        self.kick();
    }

    pub fn drag_text_finished(&mut self) {
        self.dragging_out = false;
        self.kick();
    }

    pub fn is_focused_input(&self) -> bool {
        self.todo_field.is_some()
    }

    // ───────── persistence ─────────

    pub fn data_text(&self) -> String {
        let mut s = String::from("# DockX island data\n");
        s += &format!("Day={}\nFocusDone={}\n", time::iso_from_days(self.data_day), self.focus_done);
        for (h, ml) in &self.drinks {
            s += &format!("Drink={:.3}|{}\n", h, ml);
        }
        for t in &self.todos {
            if !t.removing {
                let r = t.remind.map(|m| format!("{:02}:{:02}", m / 60, m % 60)).unwrap_or_default();
                s += &format!(
                    "Todo={}|{}|{}|{}|{}\n",
                    if t.done { 1 } else { 0 },
                    time::iso_from_days(t.day),
                    r,
                    if t.fired { 1 } else { 0 },
                    t.text.replace('\n', " ")
                );
            }
        }
        for f in &self.shelf {
            s += &format!("Shelf={}\n", f.path);
        }
        s
    }

    /// Returns the shelf paths (the host loads their icons).
    pub fn load_data(&mut self, text: &str) -> Vec<String> {
        let mut shelf = Vec::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "Day" => self.data_day = time::parse_iso(v).unwrap_or(0),
                "FocusDone" => self.focus_done = v.trim().parse().unwrap_or(0),
                "Drink" => {
                    if let Some((h, ml)) = v.split_once('|') {
                        if let (Ok(h), Ok(ml)) = (h.trim().parse(), ml.trim().parse()) {
                            self.drinks.push((h, ml));
                        }
                    }
                }
                "Todo" => {
                    let p: Vec<&str> = v.splitn(5, '|').collect();
                    if p.len() == 5 && !p[4].trim().is_empty() {
                        let day = time::parse_iso(p[1]).unwrap_or(self.data_day);
                        let mut t = TodoItem::new(p[4].trim(), p[0] == "1", day, time::parse_hm(p[2]));
                        t.fired = p[3] == "1";
                        self.todos.push(t);
                    }
                }
                "Shelf" => shelf.push(v.trim().to_string()),
                _ => {}
            }
        }
        shelf
    }

    // ───────── sizes ─────────

    fn collapsed_base(&self) -> (f32, f32) {
        let s = self.s;
        let i = self.cfg.size.min(2) as usize;
        if self.cfg.floating {
            ([150.0, 172.0, 196.0][i] * s, [30.0, 34.0, 38.0][i] * s)
        } else {
            ([184.0, 212.0, 244.0][i] * s, [30.0, 34.0, 38.0][i] * s)
        }
    }

    fn timer_active(&self) -> bool {
        self.focus_running || (self.focus_remaining < self.mode_len() - 0.5)
    }

    fn show_music_slot(&self) -> bool {
        self.media.has && (self.media.playing || self.now - self.media.sampled_at < 600.0)
    }

    fn mascot_visible(&self) -> bool {
        self.mascot_on && self.cfg.mascot != 0 && !self.show_music_slot() && !self.timer_active()
    }

    /// The mascot appears after 45 s without input and stays (even while the mouse heads for the
    /// island, so it can say hello) until the island opens or the user keeps working elsewhere.
    fn update_mascot(&mut self, over: bool) {
        let can = self.cfg.mascot != 0 && !self.show_music_slot() && !self.timer_active() && self.notices.is_empty();
        if !can || self.expanded {
            if self.mascot_on && self.expanded {
                self.happy_until = self.happy_until.max(self.now);
            }
            self.mascot_on = false;
            self.active_since = 0.0;
            return;
        }
        if self.idle_secs > 45.0 {
            if !self.mascot_on {
                self.mascot_on = true;
                self.kick();
            }
            self.active_since = 0.0;
        } else if self.mascot_on && self.idle_secs < 1.0 {
            if self.active_since == 0.0 {
                self.active_since = self.now;
            }
            if self.now - self.active_since > 4.0 && !over {
                self.mascot_on = false;
                self.kick();
            }
        }
    }

    /// Thin black line while nothing happens (minimal notch).
    fn line_mode(&self) -> bool {
        self.cfg.minimal && !self.cfg.floating && !self.show_music_slot() && !self.timer_active() && !self.mascot_visible()
    }

    fn collapsed_slots(&self) -> (f32, f32) {
        let (_, h) = self.collapsed_base();
        let s = self.s;
        let music = self.show_music_slot();
        if music || self.timer_active() {
            let left = h - 8.0 * s;
            let right = if self.timer_active() { 46.0 * s } else { 26.0 * s };
            return (left, right);
        }
        (0.0, 0.0)
    }

    fn target_size(&self, v: View) -> (f32, f32, f32) {
        let s = self.s;
        let (bw, bh) = self.collapsed_base();
        let small_r = if self.cfg.floating { bh / 2.0 } else { 12.0 * s };
        match v {
            View::Hidden => (bw * 0.6, 0.0, 0.0),
            View::Collapsed => {
                if self.line_mode() {
                    return (bw * 0.72, 5.0 * s, 2.5 * s);
                }
                let (l, r) = self.collapsed_slots();
                let extra = if l > 0.0 || r > 0.0 { (l + r + 28.0 * s) * if self.cfg.floating { 0.9 } else { 0.55 } } else { 0.0 };
                (bw + extra, bh, small_r)
            }
            View::Hud => (bw + 120.0 * s, bh, small_r),
            View::Notice => {
                let k = self.notices.first().map(|n| n.kind).unwrap_or(NoticeKind::Water);
                if k.compact() {
                    (bw + 190.0 * s, bh, small_r)
                } else {
                    (452.0 * s, 86.0 * s, 28.0 * s)
                }
            }
            View::Drop => (560.0 * s, 206.0 * s, 30.0 * s),
            View::Expanded(p) => {
                let (w, h) = match p {
                    Page::Today => (600.0, 236.0),
                    Page::Music => {
                        if self.lyrics_open && self.media.has {
                            (472.0 + 340.0, 214.0)
                        } else {
                            (472.0, 214.0)
                        }
                    }
                    Page::Shelf => (560.0, 206.0),
                    Page::Focus => (440.0, 196.0),
                    Page::Water => (560.0, 236.0),
                    Page::Todo => {
                        let rows = self.day_todos().len().clamp(2, 5) as f32;
                        (500.0, 158.0 + rows * 30.0)
                    }
                    Page::Widgets => (600.0, 236.0),
                    Page::Prayer => (600.0, 236.0),
                };
                (w * s, h * s, 30.0 * s)
            }
        }
    }

    fn mode_len(&self) -> f64 {
        60.0 * match self.focus_mode {
            FocusMode::Focus => self.cfg.focus_min,
            FocusMode::Break => self.cfg.break_min,
            FocusMode::LongBreak => self.cfg.long_break_min,
        } as f64
    }

    fn focus_left(&self) -> f64 {
        if self.focus_running {
            (self.focus_ends - self.now).max(0.0)
        } else {
            self.focus_remaining
        }
    }

    fn water_ml(&self) -> u32 {
        self.drinks.iter().map(|d| d.1).sum()
    }

    fn day_todos(&self) -> Vec<usize> {
        (0..self.todos.len()).filter(|&i| self.todos[i].day == self.todo_day && !self.todos[i].removing).collect()
    }

    fn prayer_times(&self) -> Option<[f64; 6]> {
        if !self.cfg.prayer_on {
            return None;
        }
        let c = time::CITIES.get(self.cfg.prayer_city as usize)?;
        let m = prayer::METHODS.get(self.cfg.prayer_method as usize).unwrap_or(&prayer::METHODS[0]);
        Some(prayer::times(self.lt.y, self.lt.m, self.lt.d, c.lat, c.lon, self.tz_min as f64 / 60.0, m, self.cfg.prayer_hanafi))
    }

    // ───────── step ─────────

    pub fn step(&mut self, env: &IslandEnv) -> bool {
        let now = env.now;
        self.now = now;
        self.unix = env.unix;
        self.tz_min = env.tz_min;
        self.idle_secs = env.idle_secs;
        let prev_minute = self.lt.minutes();
        self.lt = time::local(env.unix, env.tz_min);
        if self.lt.minutes() != prev_minute {
            self.need_render = true; // clock
        }
        if !self.started {
            self.started = true;
            self.last_step = now;
            self.water_due = now + self.cfg.water_interval as f64 * 60.0;
            self.focus_remaining = self.mode_len();
            self.todo_day = self.lt.days;
            let (w, _h, r) = self.target_size(View::Collapsed);
            self.w.snap(w * 0.5);
            self.h.snap(0.0);
            self.rad.snap(r);
            // prayers already past today don't fire on start
            if let Some(t) = self.prayer_times() {
                for (i, h) in t.iter().enumerate() {
                    if *h * 3600.0 <= self.lt.secs {
                        self.prayer_fired[i] = self.lt.days;
                    }
                }
            }
        }
        if env.s != self.s || env.screen != self.screen {
            self.s = env.s;
            self.screen = env.screen;
            self.kick();
        }
        let dt = ((now - self.last_step).clamp(0.001, 0.05)) as f32;
        self.last_step = now;

        // a new day: counters reset, unfinished tasks move to today, finished old ones leave
        let today = self.lt.days;
        if today != self.data_day {
            if self.data_day != 0 {
                self.drinks.clear();
                self.focus_done = 0;
                self.todos.retain(|t| !(t.done && t.day < today));
                for t in &mut self.todos {
                    if t.day < today {
                        t.day = today;
                    }
                }
                if self.todo_day < today {
                    self.todo_day = today;
                }
            }
            self.data_day = today;
            self.actions.push(IslandAction::SaveData);
        }

        self.tick_timers(now);

        let fs = self.cfg.hide_in_fullscreen && env.fullscreen;
        if fs != self.fs_hidden {
            self.fs_hidden = fs;
            self.kick();
        }

        // hover → expand / leave → collapse
        let local = (env.cursor.0 - self.win_x as f32, env.cursor.1 - self.win_y as f32);
        self.cursor_local = local;
        let over = self.win_visible && self.over_shape(local, 4.0 * self.s);
        self.update_mascot(over);
        let busy = self.todo_field.is_some()
            || self.pressed.is_some()
            || env.menu_open
            || self.dragging_out
            || self.scrub.is_some()
            || self.reorder.is_some()
            || now < self.hold_until;
        if !self.expanded {
            if over && !self.fs_hidden {
                if self.hover_since == 0.0 {
                    self.hover_since = now;
                }
                // the mascot says hello first
                let mascot_here = self.view == View::Collapsed && self.mascot_visible();
                if mascot_here && self.greet_until < now {
                    self.greet_until = now + 0.7;
                }
                let delay = if mascot_here { 0.55 } else { 0.07 };
                if now - self.hover_since > delay {
                    self.expanded = true;
                    self.leave_since = 0.0;
                }
            } else {
                self.hover_since = 0.0;
            }
        } else if over || busy {
            self.leave_since = 0.0;
        } else {
            if self.leave_since == 0.0 {
                self.leave_since = now;
            }
            if now - self.leave_since > 0.35 {
                self.expanded = false;
                self.hover_since = 0.0;
                self.hot = None;
            }
        }

        // notices: auto-dismiss (paused while hovered)
        if let Some(n) = self.notices.first_mut() {
            if over {
                n.hovered_time += dt as f64;
            }
            let life = match n.kind {
                NoticeKind::FocusDone => 8.0,
                NoticeKind::BreakDone => 15.0,
                NoticeKind::Water => 20.0,
                NoticeKind::LowBattery => 12.0,
                NoticeKind::Todo => 30.0,
                NoticeKind::Prayer => 30.0,
                _ => 3.2,
            };
            if now - n.start - n.hovered_time > life {
                self.notices.remove(0);
                self.kick();
            }
        }
        if let Some((_, t)) = &self.hud {
            if now - t > 1.6 {
                self.hud = None;
                self.kick();
            }
        }

        let big_notice = self.notices.first().map(|n| !n.kind.compact()).unwrap_or(false);
        let view = if !self.cfg.enabled || self.fs_hidden {
            View::Hidden
        } else if self.ext_drag {
            View::Drop
        } else if big_notice {
            View::Notice
        } else if self.expanded {
            View::Expanded(self.page)
        } else if !self.notices.is_empty() {
            View::Notice
        } else if self.hud.is_some() {
            View::Hud
        } else {
            View::Collapsed
        };
        if view != self.view {
            self.prev_view = self.view;
            self.view = view;
            self.view_changed = now;
            self.dots_open.snap(0.0);
            if !matches!(view, View::Expanded(_)) {
                self.hot = None;
            }
            self.kick();
        }

        let (tw, th, tr) = self.target_size(self.view);
        let mut anim = false;
        anim |= self.w.step(tw, 3.0, 0.74, dt, 0.3);
        anim |= self.h.step(th, 3.2, 0.78, dt, 0.3);
        anim |= self.rad.step(tr, 0.08, dt, 0.05);
        let top_t = if self.cfg.floating { 7.0 * self.s } else { 0.0 };
        anim |= self.top.step(top_t, 0.1, dt, 0.05);
        let ear_t = if self.cfg.floating || self.line_mode() && self.view == View::Collapsed {
            0.0
        } else if matches!(self.view, View::Collapsed | View::Hud) {
            6.0 * self.s
        } else {
            10.0 * self.s
        };
        anim |= self.ear.step(ear_t, 0.1, dt, 0.05);
        let split_t = 0.0; // player and lyrics live in one island
        anim |= self.split.step(split_t, 0.12, dt, 0.003);
        if now - self.view_changed < 0.25 {
            anim = true;
        }

        let dots_t = if matches!(self.hot, Some(Hot::Dots) | Some(Hot::Page(_))) { 1.0 } else { 0.0 };
        anim |= self.dots_open.step(dots_t, 0.08, dt, 0.005);

        // lyrics
        let pos = self.media.pos_at(now);
        if let Lyrics::Synced(lines) = self.lyrics.clone() {
            let (idx, _) = lrc::current(&lines, pos + 0.15);
            if idx != self.lyric_idx {
                self.lyric_idx = idx;
                self.need_render = true;
            }
            let line = if idx >= 0 { lines[idx as usize].text.clone() } else { String::new() };
            if line != self.pill_line {
                self.pill_prev = std::mem::replace(&mut self.pill_line, line);
                self.pill_change = now;
            }
            anim |= self.lyric_scroll.step(idx.max(0) as f32, 0.12, dt, 0.002);
        } else if !self.pill_line.is_empty() {
            self.pill_prev = std::mem::take(&mut self.pill_line);
            self.pill_change = now;
        }
        let pill_on = self.lyrics_pill_visible();
        let ptw = if pill_on { self.fonts.semibold.measure(&self.pill_line, 13.0 * self.s) + 30.0 * self.s } else { 0.0 };
        anim |= self.pill_w.step(ptw.min(560.0 * self.s), 0.09, dt, 0.3);
        if now - self.pill_change < 0.4 {
            anim = true;
        }

        // lists
        for it in &mut self.shelf {
            anim |= it.appear.step(1.0, 0.09, dt, 0.003);
        }
        for c in &mut self.clips {
            anim |= c.appear.step(1.0, 0.09, dt, 0.003);
        }
        let n_before = self.todos.len();
        for t in &mut self.todos {
            anim |= t.appear.step(if t.removing { 0.0 } else { 1.0 }, 0.08, dt, 0.003);
            anim |= t.check.step(if t.done { 1.0 } else { 0.0 }, 0.07, dt, 0.003);
            anim |= t.offset.step(0.0, 0.07, dt, 0.2);
        }
        self.todos.retain(|t| !(t.removing && t.appear.v <= 0.004));
        if self.todos.len() != n_before {
            anim = true;
        }
        let max_scroll = ((self.day_todos().len() as f32 - 5.0) * 30.0 * self.s).max(0.0);
        self.todo_scroll_raw = self.todo_scroll_raw.clamp(0.0, max_scroll);
        anim |= self.todo_scroll.step(self.todo_scroll_raw, 0.08, dt, 0.2);
        let sm = self.shelf_max_scroll();
        anim |= self.shelf_scroll.step(self.shelf_scroll.v.clamp(0.0, sm), 0.08, dt, 0.2);

        // ambient motion
        let showing = self.view != View::Hidden;
        let mut ambient = false;
        if showing
            && ((self.media.playing && self.media.has)
                || self.focus_running
                || self.mascot_visible()
                || self.todo_field.is_some()
                || !self.notices.is_empty()
                || self.view == View::Drop
                || matches!(self.view, View::Expanded(Page::Water | Page::Widgets | Page::Prayer | Page::Today)))
        {
            ambient = true;
        }
        if self.scrub.is_some() || self.vol_drag || now - self.vol_flash < 1.2 || self.reorder.is_some() || now < self.greet_until {
            anim = true;
        }

        if anim || self.need_render {
            self.render();
        } else if ambient && now - self.last_render >= 1.0 / 30.0 {
            // ambient motion (visualiser, mascot, timer) runs at ~30 fps to keep the CPU quiet
            self.render();
        }
        anim || ambient || over || self.ext_drag || self.hud.is_some()
    }

    fn tick_timers(&mut self, now: f64) {
        if self.focus_running && now >= self.focus_ends {
            self.focus_running = false;
            match self.focus_mode {
                FocusMode::Focus => {
                    self.focus_done += 1;
                    self.focus_round += 1;
                    self.focus_mode = if self.focus_round % 4 == 0 { FocusMode::LongBreak } else { FocusMode::Break };
                    self.focus_remaining = self.mode_len();
                    self.focus_running = true; // the break starts by itself
                    self.focus_ends = now + self.focus_remaining;
                    self.push_notice(NoticeKind::FocusDone, String::new());
                    self.log(Icon::Timer, format!("Сесія фокусу №{} завершена", self.focus_done));
                    self.actions.push(IslandAction::SaveData);
                }
                _ => {
                    self.focus_mode = FocusMode::Focus;
                    self.focus_remaining = self.mode_len();
                    self.push_notice(NoticeKind::BreakDone, String::new());
                }
            }
            self.happy_until = now + 3.0;
        }
        if self.cfg.water_on && self.water_due > 0.0 && now >= self.water_due {
            self.water_due = now + self.cfg.water_interval as f64 * 60.0;
            let h = self.lt.hour();
            if self.water_ml() < self.cfg.goal_ml && (7..23).contains(&h) {
                self.push_notice(NoticeKind::Water, String::new());
            }
        }
        // to-do reminders
        let (day, min) = (self.lt.days, self.lt.minutes());
        let mut fire = Vec::new();
        for t in &mut self.todos {
            if let Some(r) = t.remind {
                if !t.fired && !t.done && !t.removing && (t.day < day || (t.day == day && min >= r)) {
                    t.fired = true;
                    fire.push(t.text.clone());
                }
            }
        }
        for f in fire {
            self.log(Icon::List, format!("Нагадування: {}", f));
            self.push_notice(NoticeKind::Todo, f);
            self.actions.push(IslandAction::SaveData);
        }
        // prayer times
        if let Some(t) = self.prayer_times() {
            for i in [0usize, 2, 3, 4, 5] {
                if self.prayer_fired[i] != day && self.lt.secs >= t[i] * 3600.0 {
                    self.prayer_fired[i] = day;
                    // only right at the time (not for prayers that passed before the tab was enabled)
                    if self.cfg.prayer_remind && self.lt.secs < t[i] * 3600.0 + 120.0 {
                        self.push_notice(NoticeKind::Prayer, prayer::NAMES[i].to_string());
                        if self.cfg.prayer_pause && self.media.playing {
                            self.media.position = self.media.pos_at(self.now);
                            self.media.sampled_at = self.now;
                            self.media.playing = false;
                            self.actions.push(IslandAction::MediaPause);
                        }
                    }
                    self.log(Icon::Lyrics, format!("{} · {}", prayer::NAMES[i], prayer::fmt(t[i])));
                }
            }
        }
    }

    fn push_notice(&mut self, kind: NoticeKind, text: String) {
        self.notices.retain(|n| n.kind != kind || (kind == NoticeKind::Todo && n.text != text));
        let n = Notice { kind, text, start: self.now, hovered_time: 0.0 };
        if kind.compact() {
            self.notices.push(n);
        } else {
            // big notices go before compact cards
            let pos = self.notices.iter().position(|x| x.kind.compact()).unwrap_or(self.notices.len());
            self.notices.insert(pos, n);
        }
        if !kind.compact() {
            self.actions.push(IslandAction::Sound(kind));
        }
        self.kick();
    }

    /// Demo/testing hooks.
    pub fn debug_notice(&mut self, kind: NoticeKind, text: &str) {
        self.push_notice(kind, text.to_string());
    }
    pub fn debug_focus_ends_in(&mut self, secs: f64) {
        if self.focus_running {
            self.focus_ends = self.now + secs;
        }
    }

    fn lyrics_pill_visible(&self) -> bool {
        self.cfg.lyrics_pill && self.view == View::Collapsed && self.media.playing && !self.pill_line.is_empty() && self.pill_line != "♪"
    }

    fn shelf_max_scroll(&self) -> f32 {
        let tiles = self.shelf.len() as f32;
        let tile_w = 86.0 * self.s;
        let area = (560.0 - 36.0) * self.s;
        (tiles * tile_w - area).max(0.0)
    }

    fn over_shape(&self, l: (f32, f32), pad: f32) -> bool {
        if let Some((a, b)) = self.split_rects {
            return a.inflate(pad, pad).contains(l.0, l.1) || b.inflate(pad, pad).contains(l.0, l.1);
        }
        // the thin line is hard to hit: give it a taller hover band
        let r = if self.shape.h < 8.0 * self.s { rect(self.shape.x, self.shape.y, self.shape.w, 10.0 * self.s) } else { self.shape };
        r.inflate(pad, pad).contains(l.0, l.1)
    }

    // ───────── render ─────────

    fn render(&mut self) {
        self.need_render = false;
        self.rendered += 1;
        self.last_render = self.now;
        let s = self.s;
        let (w, h) = (self.w.v.max(1.0), self.h.v);
        if self.view == View::Hidden && h < 1.5 {
            self.win_visible = false;
            self.shape = RectF::default();
            self.split_rects = None;
            return;
        }
        let top = self.top.v;
        let shadow_m = 22.0 * s;
        let pill_h = 26.0 * s;
        let pill_vis = self.pill_w.v > 2.0;
        let bw = w.max(self.pill_w.v) + 2.0 * shadow_m;
        let bh = top + h.max(0.0) + shadow_m + if pill_vis { pill_h + 8.0 * s } else { 0.0 };
        let (cw, ch) = (bw.ceil() as u32 + 2, bh.ceil() as u32 + 2);
        self.win_x = (self.screen.cx() - cw as f32 / 2.0).round() as i32;
        self.win_y = self.screen.y as i32;
        let cx = cw as f32 / 2.0;
        self.shape = rect(cx - w / 2.0, top, w, h);

        let mut g = std::mem::replace(&mut self.canvas, Canvas::new(1, 1));
        g.reset(cw, ch);
        self.zones.clear();
        let shape = self.shape;
        if shape.h > 0.5 {
            let rad = self.rad.v;
            let ear = self.ear.v;
            // music page with lyrics: the island splits into two parts
            let k = self.split.v;
            let mut path = None;
            self.split_rects = None;
            if k > 0.01 && false {
                let gap = 12.0 * s * k;
                let lw = (472.0 * s).min(shape.w);
                let rw = (shape.w - lw - gap).max(0.0);
                if rw > 24.0 * s {
                    let a = rect(shape.x, shape.y, lw, shape.h);
                    let b = rect(a.r() + gap, shape.y, rw, shape.h);
                    let pa = self.panel_path(a, ear, rad);
                    let pb = self.panel_path(b, ear, rad);
                    if let (Some(pa), Some(pb)) = (pa, pb) {
                        let mut builder = tiny_skia::PathBuilder::new();
                        builder.push_path(&pa);
                        builder.push_path(&pb);
                        path = builder.finish();
                    }
                    self.split_rects = Some((a, b));
                }
            }
            if path.is_none() {
                path = self.panel_path(shape, ear, rad);
            }
            if let Some((a, b)) = self.split_rects {
                shadow(&mut g, a.inflate(-ear, 0.0), rad, s, 0.9, !self.cfg.floating);
                shadow(&mut g, b.inflate(-ear, 0.0), rad, s, 0.9, !self.cfg.floating);
            } else {
                shadow(&mut g, shape.inflate(-ear, 0.0), rad, s, 0.9, !self.cfg.floating);
            }
            if let Some(p) = &path {
                if self.cfg.glass {
                    g.vgradient(p, shape.y, shape.b(), rgba(36, 36, 42, 178), rgba(12, 12, 16, 200));
                    g.gradient(p, (0.0, shape.y), (0.0, shape.y + shape.h * 0.45), &[(0.0, rgba(255, 255, 255, 26)), (1.0, rgba(255, 255, 255, 0))]);
                    g.stroke_path(p, rgba(255, 255, 255, 70), s.max(1.0));
                } else {
                    g.vgradient(p, shape.y, shape.b(), rgba(14, 14, 16, 252), rgba(4, 4, 5, 255));
                    g.stroke_path(p, RIM, s.max(1.0));
                }
                g.set_clip(Some(p), None);
            }
            // content: new view fades in as the shape settles; the old one fades out quickly
            let (tw, th, _) = self.target_size(self.view);
            let near = 1.0 - (((w - tw).abs() / (0.3 * tw.max(1.0))) + ((h - th).abs() / (0.3 * th.max(1.0)))).min(1.0);
            let a_new = smoothstep(0.25, 1.0, near);
            let since = (self.now - self.view_changed) as f32;
            let a_old = 1.0 - smoothstep(0.0, 0.12, since);
            let inner = rect(shape.x + ear, shape.y, shape.w - 2.0 * ear, shape.h);
            if a_old > 0.01 && self.prev_view != self.view {
                let v = self.prev_view;
                self.draw_view(&mut g, v, inner, a_old, false);
            }
            if a_new > 0.01 {
                let v = self.view;
                self.draw_view(&mut g, v, inner, a_new, true);
            }
            g.no_clip();
        }
        if pill_vis {
            self.draw_lyrics_pill(&mut g, cx, shape.b() + 6.0 * s, pill_h);
        }
        self.canvas = g;
        self.win_visible = true;
        self.update_hot();
    }

    fn panel_path(&self, r: RectF, ear: f32, rad: f32) -> Option<tiny_skia::Path> {
        if self.cfg.floating {
            rrect(r, rad.min(r.h / 2.0))
        } else {
            notch_path(r, ear, rad)
        }
    }

    fn update_hot(&mut self) {
        let (x, y) = self.cursor_local;
        let mut h = None;
        for (r, z) in self.zones.iter().rev() {
            if r.contains(x, y) {
                h = Some(*z);
                break;
            }
        }
        if h != self.hot {
            self.hot = h;
            self.need_render = true;
        }
    }

    fn zone(&mut self, r: RectF, h: Hot, live: bool) {
        if live {
            self.zones.push((r, h));
        }
    }

    fn is_hot(&self, h: Hot) -> bool {
        self.hot == Some(h)
    }

    fn draw_view(&mut self, g: &mut Canvas, v: View, r: RectF, a: f32, live: bool) {
        match v {
            View::Hidden => {}
            View::Collapsed => self.draw_collapsed(g, r, a),
            View::Notice => self.draw_notice(g, r, a, live),
            View::Hud => self.draw_hud(g, r, a),
            View::Drop => self.draw_shelf(g, r, a, live, true),
            View::Expanded(p) => {
                self.zone(r, Hot::Panel, live);
                match p {
                    Page::Today => self.draw_today(g, r, a, live),
                    Page::Music => self.draw_music(g, r, a, live),
                    Page::Shelf => self.draw_shelf(g, r, a, live, false),
                    Page::Focus => self.draw_focus(g, r, a, live),
                    Page::Water => self.draw_water(g, r, a, live),
                    Page::Todo => self.draw_todo(g, r, a, live),
                    Page::Widgets => self.draw_widgets(g, r, a, live),
                    Page::Prayer => self.draw_prayer(g, r, a, live),
                }
                let dots_r = if let Some((left, _)) = self.split_rects { rect(left.x + self.ear.v, left.y, left.w - 2.0 * self.ear.v, left.h) } else { r };
                self.draw_dots(g, dots_r, a, live);
            }
        }
    }

    // ───────── collapsed ─────────

    fn draw_collapsed(&mut self, g: &mut Canvas, r: RectF, a: f32) {
        let s = self.s;
        if self.line_mode() {
            return;
        }
        let (l, rw) = self.collapsed_slots();
        let pad = 9.0 * s;
        let cy = r.cy();
        let sz = r.h - 8.0 * s;
        if l > 0.0 {
            let lr = rect(r.x + pad, cy - sz / 2.0, sz, sz);
            if self.show_music_slot() && !(self.timer_active() && !self.media.playing) {
                if let Some(art) = &self.media.art {
                    g.image_rounded(art, lr, 6.0 * s, a);
                } else {
                    g.fill_rrect(lr, 6.0 * s, self.art_color.alpha(a * 0.8));
                    icons::draw(g, Icon::Music, lr.cx(), lr.cy(), sz * 0.6, TEXT.alpha(a));
                }
            } else {
                self.mini_ring(g, lr.cx(), lr.cy(), sz * 0.42, a);
            }
            let rr = rect(r.r() - pad - rw, cy - sz / 2.0, rw, sz);
            if self.timer_active() {
                let t = lrc::fmt_time(self.focus_left());
                let col = if self.focus_mode == FocusMode::Focus { ACCENT.mix(TEXT, 0.25) } else { rgba(120, 220, 160, 255) };
                g.text_in(&self.fonts.semibold, &t, 13.0 * s, rr, Align::Right, col.alpha(a));
            } else {
                self.bars(g, rr, a);
            }
            return;
        }
        // idle: clock + status ring, or the mascot when nobody is around
        if self.mascot_visible() {
            let mood = if self.now < self.greet_until {
                Mood::Happy
            } else if self.idle_secs > 240.0 || (self.active_since > 0.0 && self.now - self.active_since < 0.4) {
                Mood::Sleep
            } else {
                self.mood()
            };
            let look = ((self.cursor_local.0 - r.cx()) / 300.0).clamp(-1.0, 1.0);
            if self.cfg.mascot == 2 {
                draw_eyes(g, r.cx(), cy, r.h * 0.5, mood, self.now, look, a);
            } else {
                mascot::draw(g, r.cx(), cy + 1.0 * s, r.h * 0.62, mood, self.now, look, a);
            }
            return;
        }
        if self.cfg.ring != 0 {
            let rc = (r.x + pad + sz / 2.0, cy);
            self.status_ring(g, rc.0, rc.1, sz * 0.42, a);
        }
        if self.cfg.clock {
            let t = self.lt.hm();
            let rr = rect(r.r() - pad - 60.0 * s, cy - sz / 2.0, 60.0 * s, sz);
            g.text_in(&self.fonts.semibold, &t, 13.5 * s, rr, Align::Right, TEXT.alpha(a));
        }
    }

    /// Battery or daily-water progress ring with a tiny icon inside.
    fn status_ring(&self, g: &mut Canvas, cx: f32, cy: f32, r: f32, a: f32) {
        let s = self.s;
        let (frac, col, icon) = match self.cfg.ring {
            1 => match self.battery {
                Some(b) => {
                    let col = if b.plugged {
                        rgba(90, 220, 120, 255)
                    } else if b.percent <= 20 {
                        ACCENT
                    } else {
                        TEXT
                    };
                    (b.percent as f32 / 100.0, col, None)
                }
                None => return,
            },
            _ => ((self.water_ml() as f32 / self.cfg.goal_ml.max(1) as f32).min(1.0), WATER, Some(Icon::Drop)),
        };
        g.stroke_circle(cx, cy, r, rgba(255, 255, 255, (40.0 * a) as u8), 2.4 * s);
        g.arc(cx, cy, r, 0.0, frac.max(0.001) * std::f32::consts::TAU, col.alpha(a), 2.4 * s);
        match icon {
            Some(i) => icons::draw(g, i, cx, cy, r * 1.05, col.alpha(a)),
            None => {
                if let Some(b) = self.battery {
                    if b.charging {
                        bolt(g, cx, cy, r * 0.75, col.alpha(a));
                    } else {
                        let t = format!("{}", b.percent);
                        let f = &self.fonts.bold;
                        let fs = r * 0.95;
                        let w = f.measure(&t, fs);
                        g.text(f, &t, fs, cx - w / 2.0, cy + f.cap_height(fs) / 2.0, col.alpha(a));
                    }
                }
            }
        }
    }

    fn mood(&self) -> Mood {
        if self.now < self.happy_until {
            Mood::Happy
        } else if self.notices.iter().any(|n| n.kind == NoticeKind::Water) {
            Mood::Thirsty
        } else if self.media.playing {
            Mood::Music
        } else if self.focus_running && self.focus_mode == FocusMode::Focus {
            Mood::Focus
        } else {
            Mood::Idle
        }
    }

    fn mini_ring(&self, g: &mut Canvas, cx: f32, cy: f32, r: f32, a: f32) {
        let s = self.s;
        let frac = (1.0 - self.focus_left() / self.mode_len()).clamp(0.0, 1.0) as f32;
        g.stroke_circle(cx, cy, r, rgba(255, 255, 255, (40.0 * a) as u8), 2.6 * s);
        let col = if self.focus_mode == FocusMode::Focus { ACCENT } else { rgba(120, 220, 160, 255) };
        g.arc(cx, cy, r, 0.0, frac.max(0.001) * std::f32::consts::TAU, col.alpha(a), 2.6 * s);
    }

    fn bars(&self, g: &mut Canvas, r: RectF, a: f32) {
        let s = self.s;
        let n = 4;
        let bw = 3.2 * s;
        let gap = (r.w - n as f32 * bw) / (n as f32 - 1.0);
        let t = self.now as f32;
        let col = self.art_color.mix(TEXT, 0.15);
        for i in 0..n {
            let f = i as f32;
            let lvl = if self.media.playing {
                0.35 + 0.65 * (0.5 + 0.5 * (t * (6.0 + f * 1.7) + f * 1.3).sin()) * (0.6 + 0.4 * (t * (2.3 + f * 0.9)).sin().abs())
            } else {
                0.18
            };
            let bh = (r.h * 0.75 * lvl).max(bw);
            let x = r.x + f * (bw + gap);
            g.fill_rrect(rect(x, r.cy() - bh / 2.0, bw, bh), bw / 2.0, col.alpha(a));
        }
    }

    fn draw_lyrics_pill(&mut self, g: &mut Canvas, cx: f32, y: f32, h: f32) {
        let s = self.s;
        let w = self.pill_w.v;
        let r = rect(cx - w / 2.0, y, w, h);
        let a = (w / (40.0 * s)).min(1.0);
        if let Some(p) = rrect(r, h / 2.0) {
            g.vgradient(&p, r.y, r.b(), rgba(14, 14, 16, (245.0 * a) as u8), rgba(4, 4, 5, (250.0 * a) as u8));
            g.stroke_path(&p, RIM.alpha(a), s.max(1.0));
            g.set_clip(Some(&p), Some(r));
        }
        let k = smoothstep(0.0, 0.32, (self.now - self.pill_change) as f32);
        let f = &self.fonts.semibold;
        let size = 13.0 * s;
        let base = r.cy() + f.cap_height(size) / 2.0;
        if k < 1.0 && !self.pill_prev.is_empty() {
            let pw = f.measure(&self.pill_prev, size);
            g.text(f, &self.pill_prev.clone(), size, cx - pw / 2.0, base - k * h * 0.7, TEXT.alpha(a * (1.0 - k)));
        }
        let lw = f.measure(&self.pill_line, size);
        let line = self.pill_line.clone();
        g.text(f, &line, size, cx - lw / 2.0, base + (1.0 - k) * h * 0.7, TEXT.alpha(a * k));
        g.no_clip();
    }

    // ───────── HUD (volume, brightness, caps, layout) ─────────

    fn draw_hud(&mut self, g: &mut Canvas, r: RectF, a: f32) {
        let s = self.s;
        let Some((h, _)) = self.hud.clone() else { return };
        let pad = 12.0 * s;
        let cy = r.cy();
        let ic = (r.x + pad + 9.0 * s, cy);
        let bar = |g: &mut Canvas, v: f32, col: Rgba| {
            let br = rect(r.x + pad + 28.0 * s, cy - 3.0 * s, r.w - 2.0 * pad - 70.0 * s, 6.0 * s);
            g.fill_rrect(br, 3.0 * s, rgba(255, 255, 255, (40.0 * a) as u8));
            g.fill_rrect(rect(br.x, br.y, br.w * v.clamp(0.0, 1.0), br.h), 3.0 * s, col.alpha(a));
            let t = format!("{}%", (v * 100.0).round());
            g.text_in(&self.fonts.semibold, &t, 12.5 * s, rect(br.r(), r.y, r.r() - pad - br.r(), r.h), Align::Right, TEXT.alpha(a));
        };
        match h {
            Hud::Volume(v, m) => {
                icons::draw(g, if m || v < 0.005 { Icon::SpeakerMute } else { Icon::Speaker }, ic.0, ic.1, 15.0 * s, TEXT.alpha(a));
                bar(g, if m { 0.0 } else { v }, TEXT);
            }
            Hud::Brightness(v) => {
                sun_icon(g, ic.0, ic.1, 7.0 * s, rgba(255, 210, 90, 255).alpha(a), s);
                bar(g, v, rgba(255, 210, 90, 255));
            }
            Hud::Caps(on) => {
                caps_icon(g, ic.0, ic.1, 14.0 * s, if on { ACCENT } else { TEXT2 }.alpha(a), s);
                let t = if on { "Caps Lock увімкнено" } else { "Caps Lock вимкнено" };
                g.text_in(&self.fonts.semibold, t, 13.0 * s, rect(r.x + pad + 28.0 * s, r.y, r.w, r.h), Align::Left, TEXT.alpha(a));
            }
            Hud::Layout(name) => {
                let chip = rect(ic.0 - 12.0 * s, cy - 10.0 * s, 26.0 * s, 20.0 * s);
                g.stroke_rrect(chip, 5.0 * s, TEXT2.alpha(a), 1.3 * s);
                let short: String = name.chars().take(2).collect();
                g.text_in(&self.fonts.bold, &short.to_uppercase(), 10.0 * s, chip, Align::Center, TEXT.alpha(a));
                g.text_in(&self.fonts.semibold, &name, 13.0 * s, rect(r.x + pad + 32.0 * s, r.y, r.w, r.h), Align::Left, TEXT.alpha(a));
            }
        }
    }

    // ───────── notices ─────────

    fn draw_notice(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let Some((kind, ntext)) = self.notices.first().map(|n| (n.kind, n.text.clone())) else { return };
        let fonts = self.fonts.clone();
        if kind.compact() {
            let pad = 12.0 * s;
            let green = rgba(90, 220, 120, 255);
            let (icon_col, title, value): (Rgba, String, String) = match kind {
                NoticeKind::Charging => {
                    let b = self.battery.unwrap_or(Battery { percent: 0, charging: true, plugged: true });
                    (green, "Заряджається".into(), format!("{}%", b.percent))
                }
                NoticeKind::WifiOn => (TEXT, "Wi-Fi".into(), ntext.clone()),
                NoticeKind::WifiOff => (ACCENT, "Wi-Fi відключено".into(), String::new()),
                _ => (TEXT, "Підключено".into(), ntext.clone()),
            };
            let ic = (r.x + pad + 9.0 * s, r.cy());
            match kind {
                NoticeKind::Charging => bolt(g, ic.0, ic.1, 9.0 * s, green.alpha(a)),
                NoticeKind::WifiOn | NoticeKind::WifiOff => wifi_icon(g, ic.0, ic.1 + 3.0 * s, 9.0 * s, icon_col.alpha(a), s),
                _ => headphones_icon(g, ic.0, ic.1, 9.0 * s, TEXT.alpha(a), s),
            }
            let tx = r.x + pad + 26.0 * s;
            let tw = g.text(&fonts.semibold, &title, 13.0 * s, tx, r.cy() + fonts.semibold.cap_height(13.0 * s) / 2.0, icon_col.mix(TEXT, 0.3).alpha(a));
            let avail = r.r() - pad - (tx + tw + 10.0 * s);
            let v = fonts.regular.ellipsize(&value, 12.5 * s, avail);
            g.text_in(&fonts.regular, &v, 12.5 * s, rect(r.r() - pad - avail, r.y, avail, r.h), Align::Right, TEXT2.alpha(a));
            return;
        }
        let ic = (r.x + 40.0 * s, r.cy());
        let (title, sub, ba, bb): (String, String, &str, Option<&str>) = match kind {
            NoticeKind::Water => (
                "Час попити води".into(),
                format!("Сьогодні {} з {} мл · чашка {} мл", self.water_ml(), self.cfg.goal_ml, self.cfg.cup_ml),
                "Випити",
                Some("Пізніше"),
            ),
            NoticeKind::FocusDone => ("Фокус завершено!".into(), format!("Перерва {} хв уже почалась", self.break_len_min()), "Ок", None),
            NoticeKind::BreakDone => ("Перерва закінчилась".into(), "Готовий до наступної сесії?".into(), "Почати", Some("Пізніше")),
            NoticeKind::LowBattery => (
                "Низький заряд батареї".into(),
                format!("Залишилось {}% — підключи зарядку", self.battery.map(|b| b.percent).unwrap_or(0)),
                "Ок",
                None,
            ),
            NoticeKind::Todo => ("Нагадування".into(), ntext.clone(), "Готово", Some("Ок")),
            NoticeKind::Prayer => (format!("Час молитви: {}", ntext), self.prayer_city_name(), "Ок", None),
            _ => (String::new(), String::new(), "Ок", None),
        };
        if self.cfg.mascot != 0 {
            let mood = match kind {
                NoticeKind::Water => Mood::Thirsty,
                NoticeKind::LowBattery => Mood::Sleep,
                NoticeKind::Todo | NoticeKind::Prayer => Mood::Idle,
                _ => Mood::Happy,
            };
            if self.cfg.mascot == 2 {
                draw_eyes(g, ic.0, ic.1, 26.0 * s, mood, self.now, 0.3, a);
            } else {
                mascot::draw(g, ic.0, ic.1 + 4.0 * s, 40.0 * s, mood, self.now, 0.3, a);
            }
        } else {
            let col = if kind == NoticeKind::Water { WATER } else { ACCENT };
            g.fill_circle(ic.0, ic.1, 20.0 * s, col.alpha(a));
            icons::draw(g, if kind == NoticeKind::Water { Icon::Drop } else { Icon::Bell }, ic.0, ic.1, 18.0 * s, TEXT.alpha(a));
        }
        let tx = r.x + 74.0 * s;
        let mut bx = r.r() - 16.0 * s;
        let mut btns: Vec<(Hot, &str, bool)> = Vec::new();
        if let Some(b) = bb {
            btns.push((Hot::NoticeB, b, false));
        }
        btns.push((Hot::NoticeA, ba, true));
        for (hot, label, primary) in btns {
            let tw = fonts.semibold.measure(label, 12.5 * s);
            let br = rect(bx - tw - 26.0 * s, r.cy() - 15.0 * s, tw + 26.0 * s, 30.0 * s);
            self.zone(br, hot, live);
            if primary && kind == NoticeKind::Water {
                g.fill_rrect(br, br.h / 2.0, if self.is_hot(hot) { rgba(84, 170, 255, 255) } else { WATER }.alpha(a));
            } else {
                chip(g, br, self.is_hot(hot), primary, a);
            }
            g.text_in(&fonts.semibold, label, 12.5 * s, br, Align::Center, TEXT.alpha(a));
            bx = br.x - 8.0 * s;
        }
        let avail = bx - tx - 6.0 * s;
        let title = fonts.semibold.ellipsize(&title, 15.0 * s, avail);
        let sub = fonts.regular.ellipsize(&sub, 12.0 * s, avail);
        g.text(&fonts.semibold, &title, 15.0 * s, tx, r.cy() - 3.0 * s, TEXT.alpha(a));
        g.text(&fonts.regular, &sub, 12.0 * s, tx, r.cy() + 15.0 * s, TEXT2.alpha(a));
    }

    fn prayer_city_name(&self) -> String {
        time::CITIES.get(self.cfg.prayer_city as usize).map(|c| c.name.to_string()).unwrap_or_default()
    }

    fn break_len_min(&self) -> u32 {
        if self.focus_mode == FocusMode::LongBreak {
            self.cfg.long_break_min
        } else {
            self.cfg.break_min
        }
    }

    // ───────── input ─────────

    fn local(&self, x: f32, y: f32) -> (f32, f32) {
        (x - self.win_x as f32, y - self.win_y as f32)
    }

    fn hit(&self, l: (f32, f32)) -> Option<Hot> {
        self.zones.iter().rev().find(|(r, _)| r.contains(l.0, l.1)).map(|(_, h)| *h)
    }

    /// Screen rectangle of a control by name (used by the demo script to aim the cursor).
    pub fn zone_rect(&self, name: &str) -> Option<RectF> {
        if name == "island" {
            let r = self.split_rects.map(|(a, _)| a).unwrap_or(self.shape);
            return Some(r.offset(self.win_x as f32, self.win_y as f32));
        }
        self.zones.iter().rev().find(|(_, h)| h.name() == name).map(|(r, _)| r.offset(self.win_x as f32, self.win_y as f32))
    }

    /// Index of the lyric line being sung (-1 if none).
    pub fn current_lyric(&self) -> i32 {
        self.lyric_idx
    }

    /// True if the island (its visible shape) is under this screen point.
    pub fn contains_screen(&self, x: f32, y: f32) -> bool {
        let l = self.local(x, y);
        self.win_visible && self.over_shape(l, 2.0)
    }

    pub fn mouse_down(&mut self, btn: Btn, x: f32, y: f32, mods: Mods, double: bool) {
        let l = self.local(x, y);
        self.cursor_local = l;
        let hit = self.hit(l);
        if btn != Btn::Left {
            return;
        }
        if self.todo_field.is_some() && hit != Some(Hot::TodoInput) {
            self.blur_todo();
        }
        self.pressed = hit;
        self.press_pt = (x, y);
        match hit {
            Some(Hot::Seek) => self.scrub = Some(self.frac_in(l.0, Hot::Seek)),
            Some(Hot::Volume) => {
                self.vol_drag = true;
                let v = self.frac_in(l.0, Hot::Volume);
                self.change_volume(v);
            }
            Some(Hot::Tile(i)) => {
                if double {
                    if let Some(it) = self.shelf.get(i) {
                        self.actions.push(IslandAction::OpenFile(it.path.clone()));
                    }
                    self.pressed = None;
                } else {
                    self.drag_src = Some((Hot::Tile(i), (x, y)));
                    self.tile_click(i, mods);
                }
            }
            Some(Hot::Clip(i)) => self.drag_src = Some((Hot::Clip(i), (x, y))),
            Some(Hot::TodoRow(i)) => self.drag_src = Some((Hot::TodoRow(i), (x, y))),
            _ => {}
        }
        self.kick();
    }

    fn tile_click(&mut self, i: usize, mods: Mods) {
        if mods.shift {
            let a = self.shelf_anchor.unwrap_or(i);
            let (lo, hi) = (a.min(i), a.max(i));
            for (k, it) in self.shelf.iter_mut().enumerate() {
                it.selected = k >= lo && k <= hi;
            }
        } else if mods.ctrl || self.shelf_select {
            if let Some(it) = self.shelf.get_mut(i) {
                it.selected = !it.selected;
            }
            self.shelf_anchor = Some(i);
        } else {
            if !self.shelf.get(i).map(|x| x.selected).unwrap_or(false) {
                for it in &mut self.shelf {
                    it.selected = false;
                }
            }
            self.shelf_anchor = Some(i);
        }
    }

    fn frac_in(&self, lx: f32, h: Hot) -> f32 {
        let r = self.zones.iter().find(|(_, z)| *z == h).map(|(r, _)| *r).unwrap_or_default();
        ((lx - r.x) / r.w.max(1.0)).clamp(0.0, 1.0)
    }

    fn change_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
        self.muted = false;
        self.vol_flash = self.now;
        self.actions.push(IslandAction::SetVolume(self.volume));
    }

    pub fn mouse_move(&mut self, x: f32, y: f32) {
        let l = self.local(x, y);
        self.cursor_local = l;
        if self.scrub.is_some() {
            self.scrub = Some(self.frac_in(l.0, Hot::Seek));
            self.kick();
        }
        if self.vol_drag {
            let v = self.frac_in(l.0, Hot::Volume);
            self.change_volume(v);
            self.kick();
        }
        if let Some((i, start_y)) = self.reorder {
            self.reorder_move(i, start_y, y);
        }
        if let Some((src, p)) = self.drag_src {
            let moved = (x - p.0).abs() > 6.0 * self.s || (y - p.1).abs() > 6.0 * self.s;
            if moved {
                self.drag_src = None;
                match src {
                    Hot::Tile(i) => {
                        self.pressed = None;
                        let mut paths: Vec<String> = self.shelf.iter().filter(|t| t.selected).map(|t| t.path.clone()).collect();
                        if paths.is_empty() || !self.shelf.get(i).map(|t| t.selected).unwrap_or(false) {
                            paths = self.shelf.get(i).map(|t| vec![t.path.clone()]).unwrap_or_default();
                        }
                        if !paths.is_empty() {
                            self.dragging_out = true;
                            self.actions.push(IslandAction::DragOut(paths));
                        }
                    }
                    Hot::Clip(i) => {
                        self.pressed = None;
                        if let Some(c) = self.clips.get(i) {
                            self.dragging_out = true;
                            self.actions.push(IslandAction::DragText(c.text.clone()));
                        }
                    }
                    Hot::TodoRow(i) => {
                        self.pressed = None;
                        self.reorder = Some((i, p.1));
                    }
                    _ => {}
                }
            }
        }
        self.update_hot();
    }

    /// Dragging a task up/down swaps it with neighbours (same day), neighbours slide.
    fn reorder_move(&mut self, i: usize, start_y: f32, y: f32) {
        let row = 30.0 * self.s;
        let list = self.day_todos();
        let Some(pos) = list.iter().position(|&k| k == i) else { return };
        let dy = y - start_y;
        let steps = (dy / row).round() as i32;
        if steps != 0 {
            let to = (pos as i32 + steps).clamp(0, list.len() as i32 - 1) as usize;
            if to != pos {
                let target_idx = list[to];
                let item = self.todos.remove(i);
                let insert_at = if target_idx > i { target_idx } else { target_idx };
                self.todos.insert(insert_at.min(self.todos.len()), item);
                let new_i = insert_at.min(self.todos.len() - 1);
                // neighbours slide into place
                let moved = to as i32 - pos as i32;
                for (k, &idx) in self.day_todos().iter().enumerate() {
                    if idx != new_i {
                        let between = if moved > 0 { k >= pos && k < to } else { k > to && k <= pos };
                        if between {
                            self.todos[idx].offset.v += if moved > 0 { row } else { -row };
                        }
                    }
                }
                self.reorder = Some((new_i, start_y + moved as f32 * row));
                self.actions.push(IslandAction::SaveData);
            }
        }
        self.kick();
    }

    pub fn mouse_up(&mut self, btn: Btn, x: f32, y: f32) {
        let l = self.local(x, y);
        if btn == Btn::Right {
            if self.contains_screen(x, y) {
                self.actions.push(IslandAction::Menu { x, y });
            }
            return;
        }
        if btn != Btn::Left {
            return;
        }
        let hit = self.hit(l);
        let pressed = self.pressed.take();
        self.drag_src = None;
        if self.reorder.take().is_some() {
            self.kick();
            return;
        }
        if let Some(f) = self.scrub.take() {
            let d = self.media.duration;
            if d > 0.0 {
                let t = f as f64 * d;
                self.media.position = t;
                self.media.sampled_at = self.now;
                self.actions.push(IslandAction::MediaSeek(t));
            }
            self.kick();
            return;
        }
        if self.vol_drag {
            self.vol_drag = false;
            self.kick();
            return;
        }
        if hit.is_none() || hit != pressed {
            self.kick();
            return;
        }
        self.click(hit.unwrap());
        self.kick();
    }

    pub fn focus_toggle(&mut self) {
        if self.focus_running {
            self.focus_remaining = (self.focus_ends - self.now).max(0.0);
            self.focus_running = false;
        } else {
            self.focus_running = true;
            self.focus_ends = self.now + self.focus_remaining;
        }
        self.kick();
    }

    pub fn focus_reset(&mut self) {
        self.focus_running = false;
        self.focus_mode = FocusMode::Focus;
        self.focus_remaining = self.mode_len();
        self.kick();
    }

    pub fn drink(&mut self) {
        if !self.cfg.water_on {
            return;
        }
        let h = self.lt.secs as f32 / 3600.0;
        self.drinks.push((h, self.cfg.cup_ml));
        self.water_due = self.now + self.cfg.water_interval as f64 * 60.0;
        self.happy_until = self.now + 2.0;
        self.notices.retain(|n| n.kind != NoticeKind::Water);
        self.log(Icon::Drop, format!("Вода +{} мл ({} / {})", self.cfg.cup_ml, self.water_ml(), self.cfg.goal_ml));
        self.actions.push(IslandAction::SaveData);
        self.kick();
    }

    pub fn wheel(&mut self, delta: f32, x: f32, y: f32) {
        let l = self.local(x, y);
        self.cursor_local = l;
        if !self.contains_screen(x, y) {
            return;
        }
        let hit = self.hit(l);
        match (self.view, hit) {
            (View::Expanded(_), Some(Hot::Dots)) | (View::Expanded(_), Some(Hot::Page(_))) => {
                let pages = self.pages();
                let cur = pages.iter().position(|p| *p == self.page).unwrap_or(0) as i32;
                let i = (cur - delta.signum() as i32).clamp(0, pages.len() as i32 - 1);
                self.page = pages[i as usize];
            }
            (View::Expanded(Page::Todo), _) => self.todo_scroll_raw = (self.todo_scroll_raw - delta * 30.0 * self.s).max(0.0),
            (View::Expanded(Page::Shelf), _) => {
                if self.shelf_tab == 0 {
                    let v = (self.shelf_scroll.v - delta * 60.0 * self.s).clamp(0.0, self.shelf_max_scroll());
                    self.shelf_scroll.v = v;
                } else {
                    self.clip_scroll = (self.clip_scroll - delta * 28.0 * self.s).max(0.0);
                }
            }
            (View::Expanded(Page::Music), Some(Hot::Line(_))) | (View::Expanded(Page::Music), Some(Hot::Panel)) if self.in_lyrics(l) => {}
            _ => {
                let v = self.volume + delta * 0.04;
                self.change_volume(v);
            }
        }
        self.kick();
    }

    fn in_lyrics(&self, l: (f32, f32)) -> bool {
        self.split_rects.map(|(_, b)| b.contains(l.0, l.1)).unwrap_or(false)
    }

    pub fn capture_lost(&mut self) {
        self.pressed = None;
        self.scrub = None;
        self.vol_drag = false;
        self.drag_src = None;
        self.reorder = None;
        self.kick();
    }

    pub fn drag_over(&mut self, _x: f32, _y: f32) -> bool {
        if !self.ext_drag {
            self.ext_drag = true;
            self.kick();
        }
        true
    }

    pub fn drag_leave(&mut self) {
        if self.ext_drag {
            self.ext_drag = false;
            self.kick();
        }
    }

    pub fn drop_files(&mut self, files: Vec<String>) {
        self.ext_drag = false;
        self.expanded = true;
        self.page = Page::Shelf;
        self.shelf_tab = 0;
        self.leave_since = 0.0;
        if !files.is_empty() {
            self.actions.push(IslandAction::ShelfAdd(files));
        }
        self.kick();
    }

    pub fn key(&mut self, k: Key) {
        let now = self.now;
        let day = self.todo_day;
        let Some(f) = &mut self.todo_field else { return };
        match f.key(k, now) {
            FieldResult::Submit(s) => {
                if !s.is_empty() {
                    let (text, remind) = time::split_reminder(&s);
                    let mut t = TodoItem::new(&text, false, day, remind);
                    t.appear.snap(0.0);
                    self.todos.push(t);
                    self.actions.push(IslandAction::SaveData);
                    self.todo_scroll_raw = 1e6;
                }
                let mut nf = TextField::new("", false);
                nf.last_input = now;
                self.todo_field = Some(nf);
                self.todo_draft.clear();
            }
            FieldResult::Cancel => self.blur_todo(),
            FieldResult::None => {}
        }
        self.kick();
    }

    pub fn paste(&mut self, s: &str) {
        let now = self.now;
        if let Some(f) = &mut self.todo_field {
            f.paste(s, now);
            self.kick();
        }
    }

    pub fn blur_todo(&mut self) {
        if let Some(f) = self.todo_field.take() {
            self.todo_draft = f.string();
            self.actions.push(IslandAction::WantKeyboard(false));
            self.kick();
        }
    }

    /// Opens the island on a page (menu item "show island", demo).
    pub fn open_page(&mut self, p: Page) {
        self.page = p;
        self.expanded = true;
        self.leave_since = 0.0;
        self.kick();
    }

    pub fn collapse(&mut self) {
        self.expanded = false;
        self.kick();
    }

    pub fn todo_add(&mut self, text: &str, day_offset: i64, remind: Option<u32>) {
        let day = if self.data_day != 0 { self.data_day } else { self.lt.days } + day_offset;
        let mut t = TodoItem::new(text, false, day, remind);
        t.appear.snap(1.0);
        self.todos.push(t);
        self.kick();
    }

    pub fn set_today_for_demo(&mut self, unix: f64, tz_min: i32) {
        self.lt = time::local(unix, tz_min);
        self.data_day = self.lt.days;
        self.todo_day = self.lt.days;
    }
}

/// Average colour of an image, boosted so it reads as an accent on black.
fn dominant(img: &tiny_skia::Pixmap) -> Rgba {
    let d = img.data();
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    let step = ((d.len() / 4) / 2048).max(1) * 4;
    let mut i = 0;
    while i + 3 < d.len() {
        let a = d[i + 3] as u64;
        if a > 128 {
            let (pr, pg, pb) = (d[i] as u64, d[i + 1] as u64, d[i + 2] as u64);
            let mx = pr.max(pg).max(pb);
            let mn = pr.min(pg).min(pb);
            let w = 1 + (mx - mn) / 8;
            r += pr * w;
            g += pg * w;
            b += pb * w;
            n += w;
        }
        i += step;
    }
    if n == 0 {
        return ACCENT;
    }
    let (r, g, b) = ((r / n) as f32, (g / n) as f32, (b / n) as f32);
    let mx = r.max(g).max(b).max(1.0);
    let k = 235.0 / mx;
    rgba((r * k).min(255.0) as u8, (g * k).min(255.0) as u8, (b * k).min(255.0) as u8, 255)
}

// ───────── small drawings ─────────

pub(crate) fn bolt(g: &mut Canvas, cx: f32, cy: f32, k: f32, col: Rgba) {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(cx + k * 0.2, cy - k);
    pb.line_to(cx - k * 0.5, cy + k * 0.12);
    pb.line_to(cx - k * 0.02, cy + k * 0.12);
    pb.line_to(cx - k * 0.2, cy + k);
    pb.line_to(cx + k * 0.5, cy - k * 0.12);
    pb.line_to(cx + k * 0.02, cy - k * 0.12);
    pb.close();
    if let Some(p) = pb.finish() {
        g.fill_path(&p, col);
    }
}

fn sun_icon(g: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, s: f32) {
    g.fill_circle(cx, cy, r * 0.55, col);
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::TAU / 8.0;
        g.line(cx + a.sin() * r * 0.85, cy - a.cos() * r * 0.85, cx + a.sin() * r * 1.25, cy - a.cos() * r * 1.25, col, 1.5 * s);
    }
}

fn caps_icon(g: &mut Canvas, cx: f32, cy: f32, size: f32, col: Rgba, s: f32) {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(cx, cy - size * 0.45);
    pb.line_to(cx + size * 0.42, cy);
    pb.line_to(cx + size * 0.18, cy);
    pb.line_to(cx + size * 0.18, cy + size * 0.22);
    pb.line_to(cx - size * 0.18, cy + size * 0.22);
    pb.line_to(cx - size * 0.18, cy);
    pb.line_to(cx - size * 0.42, cy);
    pb.close();
    if let Some(p) = pb.finish() {
        g.stroke_path(&p, col, 1.5 * s);
    }
    g.line(cx - size * 0.18, cy + size * 0.42, cx + size * 0.18, cy + size * 0.42, col, 1.5 * s);
}

pub(crate) fn wifi_icon(g: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, s: f32) {
    for k in 1..=3 {
        let rr = r * k as f32 / 3.0 * 1.25;
        g.arc(cx, cy, rr, -0.8, 1.6, col, 1.7 * s);
    }
    g.fill_circle(cx, cy, 1.6 * s, col);
}

pub(crate) fn headphones_icon(g: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, s: f32) {
    g.arc(cx, cy + r * 0.2, r, -1.6, 3.2, col, 1.8 * s);
    g.fill_rrect(rect(cx - r - 1.5 * s, cy + r * 0.1, 4.5 * s, r * 0.9), 2.0 * s, col);
    g.fill_rrect(rect(cx + r - 3.0 * s, cy + r * 0.1, 4.5 * s, r * 0.9), 2.0 * s, col);
}

/// Mascot: just a pair of eyes (blink, look, sleep, greet).
pub(crate) fn draw_eyes(g: &mut Canvas, cx: f32, cy: f32, size: f32, mood: Mood, t: f64, look: f32, a: f32) {
    let ew = size * 0.34;
    let eh = size * 0.5;
    let gap = size * 0.5;
    let white = rgba(255, 255, 255, (245.0 * a) as u8);
    let blink = (t % 3.9) > 3.76;
    for sx in [-1.0f32, 1.0] {
        let ex = cx + sx * gap / 2.0 + look * size * 0.12;
        match mood {
            Mood::Sleep => {
                g.arc(ex, cy - eh * 0.1, ew * 0.55, 1.9, 2.5, white, size * 0.08);
            }
            Mood::Happy => {
                g.arc(ex, cy + eh * 0.15, ew * 0.55, -1.2, 2.4, white, size * 0.09);
            }
            _ => {
                let h = if blink { size * 0.06 } else { eh };
                g.fill_rrect(rect(ex - ew / 2.0, cy - h / 2.0, ew, h), ew / 2.0, white);
            }
        }
    }
    if mood == Mood::Sleep {
        let zt = (t * 0.7).fract() as f32;
        let col = rgba(255, 255, 255, (190.0 * a * (1.0 - zt)) as u8);
        let zs = size * 0.25;
        let zx = cx + gap * 0.9 + zt * size * 0.3;
        let zy = cy - size * 0.2 - zt * size * 0.4;
        g.line(zx, zy, zx + zs, zy, col, size * 0.05);
        g.line(zx + zs, zy, zx, zy + zs, col, size * 0.05);
        g.line(zx, zy + zs, zx + zs, zy + zs, col, size * 0.05);
    }
}
