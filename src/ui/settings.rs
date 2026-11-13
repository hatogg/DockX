//! Settings: the shared model (+ dock.ini format) and the settings window, drawn in the same style.

use super::anim::*;
use super::dock::{DockItem, DockSettings};
use super::gfx::*;
use super::icons::{self, Icon};
use super::island::{IslandSettings, WIDGETS};
use super::text_field::*;
use super::theme::*;
use super::{time, Btn};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub dock: DockSettings,
    pub island: IslandSettings,
    pub autostart: bool,
    pub show_dock: bool,
    /// Windows' own taskbar auto-hide (read from and written to the system, not dock.ini)
    pub taskbar_autohide: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings { dock: DockSettings::default(), island: IslandSettings::default(), autostart: false, show_dock: true, taskbar_autohide: false }
    }
}

fn b(v: bool) -> &'static str {
    if v {
        "1"
    } else {
        "0"
    }
}

/// dock.ini — keeps the DockX 1.1 keys (IconSize, Magnification, AutoHide, HideInFullscreen, Item) and adds the island.
pub fn to_ini(a: &AppSettings, items: &[DockItem]) -> String {
    let d = &a.dock;
    let i = &a.island;
    let mut s = String::from("# DockX 2.0\n");
    s += &format!("IconSize={}\nMagnification={:.2}\nAutoHide={}\nHideInFullscreen={}\nShowDock={}\n", d.icon_size, d.magnification, b(d.auto_hide), b(d.hide_in_fullscreen), b(a.show_dock));
    let kv: Vec<(&str, String)> = vec![
        ("Island", b(i.enabled).into()),
        ("IslandSize", i.size.to_string()),
        ("IslandFloating", b(i.floating).into()),
        ("IslandMinimal", b(i.minimal).into()),
        ("IslandGlass", b(i.glass).into()),
        ("IslandClock", b(i.clock).into()),
        ("IslandRing", i.ring.to_string()),
        ("Mascot", i.mascot.to_string()),
        ("LyricsPill", b(i.lyrics_pill).into()),
        ("LyricsOnline", b(i.lyrics_online).into()),
        ("IslandHideFullscreen", b(i.hide_in_fullscreen).into()),
        ("Clipboard", b(i.clip_on).into()),
        ("ClipboardMax", i.clip_max.to_string()),
        ("Water", b(i.water_on).into()),
        ("WaterInterval", i.water_interval.to_string()),
        ("CupMl", i.cup_ml.to_string()),
        ("GoalMl", i.goal_ml.to_string()),
        ("Focus", i.focus_min.to_string()),
        ("Break", i.break_min.to_string()),
        ("LongBreak", i.long_break_min.to_string()),
        ("BatteryNotify", b(i.battery_notify).into()),
        ("BatteryLow", b(i.battery_low).into()),
        ("WidgetA", i.widget_a.to_string()),
        ("WidgetB", i.widget_b.to_string()),
        ("WorldCity", i.world_city.to_string()),
        ("CountdownTitle", i.countdown_title.replace('\n', " ")),
        ("CountdownDate", i.countdown_date.clone()),
        ("HudVolume", b(i.hud_volume).into()),
        ("HudBrightness", b(i.hud_brightness).into()),
        ("HudCaps", b(i.hud_caps).into()),
        ("HudLayout", b(i.hud_layout).into()),
        ("CardWifi", b(i.card_wifi).into()),
        ("CardAudio", b(i.card_audio).into()),
    ];
    for (k, v) in kv {
        s += &format!("{}={}\n", k, v);
    }
    for it in items {
        if !it.removing {
            s += &format!("Item={}|{}\n", it.path, it.name.replace('|', " "));
        }
    }
    s
}

/// Parses dock.ini; returns settings and (path, name) of dock items.
pub fn from_ini(text: &str) -> (AppSettings, Vec<(String, String)>) {
    let mut a = AppSettings::default();
    let mut items = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        let on = v == "1";
        let u = |d: u32| v.parse::<u32>().unwrap_or(d);
        let u8v = |d: u8| v.parse::<u8>().unwrap_or(d);
        let i = &mut a.island;
        match k {
            "IconSize" => a.dock.icon_size = v.parse::<i32>().unwrap_or(52).clamp(32, 96),
            "Magnification" => a.dock.magnification = v.parse::<f32>().unwrap_or(1.7).clamp(1.0, 2.2),
            "AutoHide" => a.dock.auto_hide = on,
            "HideInFullscreen" => a.dock.hide_in_fullscreen = on,
            "ShowDock" => a.show_dock = on,
            "Island" => i.enabled = on,
            "IslandSize" => i.size = u8v(1).min(2),
            "IslandFloating" => i.floating = on,
            "IslandMinimal" => i.minimal = on,
            "IslandGlass" => i.glass = on,
            "IslandClock" => i.clock = on,
            "IslandRing" => i.ring = u8v(1).min(2),
            "Mascot" => i.mascot = u8v(1).min(2),
            "LyricsPill" => i.lyrics_pill = on,
            "LyricsOnline" => i.lyrics_online = on,
            "IslandHideFullscreen" => i.hide_in_fullscreen = on,
            "Clipboard" => i.clip_on = on,
            "ClipboardMax" => i.clip_max = u(20).clamp(1, 100),
            "Water" => i.water_on = on,
            "WaterInterval" => i.water_interval = u(60).clamp(10, 300),
            "CupMl" => i.cup_ml = u(250).clamp(50, 1500),
            "GoalMl" => i.goal_ml = u(2000).clamp(500, 6000),
            "Focus" => i.focus_min = u(25).clamp(1, 180),
            "Break" => i.break_min = u(5).clamp(1, 60),
            "LongBreak" => i.long_break_min = u(15).clamp(1, 90),
            "BatteryNotify" => i.battery_notify = on,
            "BatteryLow" => i.battery_low = on,
            "WidgetA" => i.widget_a = u8v(0) % WIDGETS.len() as u8,
            "WidgetB" => i.widget_b = u8v(2) % WIDGETS.len() as u8,
            "WorldCity" => i.world_city = u8v(14).min(time::CITIES.len() as u8 - 1),
            "CountdownTitle" => i.countdown_title = v.to_string(),
            "CountdownDate" => i.countdown_date = v.to_string(),
            "HudVolume" => i.hud_volume = on,
            "HudBrightness" => i.hud_brightness = on,
            "HudCaps" => i.hud_caps = on,
            "HudLayout" => i.hud_layout = on,
            "CardWifi" => i.card_wifi = on,
            "CardAudio" => i.card_audio = on,
            "Item" => {
                let mut p = v.splitn(2, '|');
                let path = p.next().unwrap_or("").trim().to_string();
                let name = p.next().unwrap_or("").trim().to_string();
                if !path.is_empty() {
                    items.push((path, name));
                }
            }
            _ => {}
        }
    }
    (a, items)
}

// ───────────────────────── settings window ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsAction {
    Changed,
    Close,
    /// start moving the window (header drag)
    StartMove,
    OpenFolder,
    ResetAll,
    WantKeyboard(bool),
}

#[derive(Clone)]
enum Ctl {
    Toggle(bool),
    /// segmented when it fits, otherwise ‹ value ›
    Choice(Vec<String>, usize),
    Text(String),
    Button(String),
    Info(String),
}

struct Row {
    id: &'static str,
    label: String,
    sub: String,
    ctl: Ctl,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SHot {
    Section(usize),
    Ctl(usize, i32),
    Close,
    Header,
}

const SECTIONS: [&str; 10] = ["Загальне", "Док", "Острів", "Музика", "Полиця і буфер", "Фокус", "Вода", "Віджети", "Індикатори", "Про програму"];

pub struct SettingsPanel {
    pub app: AppSettings,
    pub section: usize,
    pub canvas: Canvas,
    pub actions: Vec<SettingsAction>,
    pub version: String,
    pub rendered: u64,
    fonts: Rc<Fonts>,
    s: f32,
    zones: Vec<(RectF, SHot)>,
    hot: Option<SHot>,
    pressed: Option<SHot>,
    cursor: (f32, f32),
    scroll: f32,
    max_scroll: f32,
    editing: Option<(&'static str, TextField)>,
    now: f64,
    need_render: bool,
    appear: Spring,
    last_step: f64,
}

impl SettingsPanel {
    pub fn new(app: AppSettings, fonts: Rc<Fonts>, s: f32) -> SettingsPanel {
        SettingsPanel {
            app,
            section: 0,
            canvas: Canvas::new(1, 1),
            actions: Vec::new(),
            version: "2.0".into(),
            rendered: 0,
            fonts,
            s,
            zones: Vec::new(),
            hot: None,
            pressed: None,
            cursor: (-1.0, -1.0),
            scroll: 0.0,
            max_scroll: 0.0,
            editing: None,
            now: 0.0,
            need_render: true,
            appear: Spring::new(0.0),
            last_step: -1.0,
        }
    }

    pub fn margin(&self) -> f32 {
        26.0 * self.s
    }
    /// Window size in pixels (panel + shadow margin).
    pub fn size(&self) -> (u32, u32) {
        let m = self.margin();
        ((820.0 * self.s + 2.0 * m) as u32, (580.0 * self.s + 2.0 * m) as u32)
    }

    fn rows(&self) -> Vec<Row> {
        let a = &self.app;
        let i = &a.island;
        let r = |id: &'static str, label: &str, sub: &str, ctl: Ctl| Row { id, label: label.into(), sub: sub.into(), ctl };
        let ch = |opts: &[&str], k: usize| Ctl::Choice(opts.iter().map(|s| s.to_string()).collect(), k);
        let pos = |list: &[u32], v: u32| list.iter().position(|x| *x == v).unwrap_or(0);
        match self.section {
            0 => vec![
                r("autostart", "Запускати разом із Windows", "DockX стартує після входу в систему", Ctl::Toggle(a.autostart)),
                r("fullscreen", "Ховати в повноекранних іграх і відео", "Док і острів не заважають FPS і не додають затримки", Ctl::Toggle(a.dock.hide_in_fullscreen && i.hide_in_fullscreen)),
                r("show_dock", "Показувати док", "Панель програм унизу екрана", Ctl::Toggle(a.show_dock)),
                r("show_island", "Показувати Dynamic Island", "Острів угорі екрана", Ctl::Toggle(i.enabled)),
                r("folder", "Папка налаштувань", "%APPDATA%\\DockX — dock.ini, island.dat, error.log", Ctl::Button("Відкрити".into())),
                r("reset", "Скинути налаштування", "Повернути все як після встановлення (іконки в доку лишаються)", Ctl::Button("Скинути".into())),
            ],
            1 => {
                let sizes = [40u32, 48, 56, 64, 72, 84];
                let si = sizes.iter().enumerate().min_by_key(|(_, s)| (**s as i32 - a.dock.icon_size).abs()).map(|(k, _)| k).unwrap_or(1);
                let mags = [1.0f32, 1.35, 1.7, 2.05];
                let mi = mags.iter().enumerate().min_by(|x, y| (x.1 - a.dock.magnification).abs().partial_cmp(&(y.1 - a.dock.magnification).abs()).unwrap()).map(|(k, _)| k).unwrap_or(2);
                vec![
                    r("icon_size", "Розмір іконок", "", ch(&["40", "48", "56", "64", "72", "84"], si)),
                    r("magnify", "Збільшення при наведенні", "", ch(&["Вимк", "Слабке", "Середнє", "Сильне"], mi)),
                    r("autohide", "Автоприховування", "Док виїжджає, коли курсор торкається нижнього краю", Ctl::Toggle(a.dock.auto_hide)),
                    r("taskbar", "Ховати панель задач Windows", "Панель задач виїжджає від краю, тож док не стоїть над нею", Ctl::Toggle(a.taskbar_autohide)),
                ]
            }
            2 => vec![
                r("isl_size", "Розмір острова", "", ch(&["Малий", "Середній", "Великий"], i.size as usize)),
                r("isl_style", "Вигляд", "Виріз біля краю екрана або плаваюча пігулка", ch(&["Виріз", "Пігулка"], if i.floating { 1 } else { 0 })),
                r("minimal", "Мінімальний виріз", "У спокої — тонка чорна лінія (для ноутбуків з товстими рамками)", Ctl::Toggle(i.minimal)),
                r("glass", "Ефект скла", "Напівпрозора панель замість суцільного чорного", Ctl::Toggle(i.glass)),
                r("clock", "Годинник у згорнутому стані", "", Ctl::Toggle(i.clock)),
                r("ring", "Кільце статусу", "Прогрес навколо значка біля годинника", ch(&["Вимк", "Батарея", "Вода"], i.ring as usize)),
                r("mascot", "Маскот", "Зʼявляється, коли нічого не відбувається; засинає і вітається", ch(&["Вимк", "Іскра", "Очі"], i.mascot as usize)),
            ],
            3 => vec![
                r("lyrics_online", "Тексти пісень (LRCLIB)", "Шукати синхронні тексти в інтернеті за назвою треку", Ctl::Toggle(i.lyrics_online)),
                r("lyrics_pill", "Рядок пісні під островом", "Поточний рядок у згорнутому стані", Ctl::Toggle(i.lyrics_pill)),
                r("players", "Плеєри", "Spotify, YouTube у браузері, Apple Music, будь-який плеєр з медіапанеллю Windows", Ctl::Info("Автоматично".into())),
            ],
            4 => {
                let maxes = [5u32, 10, 20, 50];
                vec![
                    r("clip_on", "Історія буфера обміну", "Тексти й посилання, які ти копіюєш (лише в памʼяті)", Ctl::Toggle(i.clip_on)),
                    r("clip_max", "Скільки зберігати", "", ch(&["5", "10", "20", "50"], pos(&maxes, i.clip_max))),
                    r("shelf_info", "Полиця", "Зберігає лише посилання на файли, нічого не копіює", Ctl::Info("Увімкнена".into())),
                ]
            }
            5 => {
                let f = [15u32, 25, 45, 50];
                let br = [3u32, 5, 10];
                let lb = [10u32, 15, 20, 30];
                vec![
                    r("focus", "Тривалість фокусу", "4 раунди, після 4-го — довга перерва", ch(&["15 хв", "25 хв", "45 хв", "50 хв"], pos(&f, i.focus_min))),
                    r("break", "Перерва", "", ch(&["3 хв", "5 хв", "10 хв"], pos(&br, i.break_min))),
                    r("long_break", "Довга перерва", "", ch(&["10 хв", "15 хв", "20 хв", "30 хв"], pos(&lb, i.long_break_min))),
                ]
            }
            6 => {
                let cups = [150u32, 200, 250, 330, 500];
                let goals = [1500u32, 2000, 2500, 3000];
                let ints = [30u32, 45, 60, 90, 120];
                vec![
                    r("water_on", "Нагадування пити воду", "З 7:00 до 23:00, поки не виконана норма", Ctl::Toggle(i.water_on)),
                    r("cup", "Обʼєм чашки / пляшки", "", ch(&["150", "200", "250", "330", "500 мл"], pos(&cups, i.cup_ml))),
                    r("goal", "Денна норма", "", ch(&["1,5 л", "2 л", "2,5 л", "3 л"], pos(&goals, i.goal_ml))),
                    r("interval", "Нагадувати кожні", "", ch(&["30", "45", "60", "90", "120 хв"], pos(&ints, i.water_interval))),
                ]
            }
            7 => {
                let w: Vec<&str> = WIDGETS.to_vec();
                let cities: Vec<&str> = time::CITIES.iter().map(|c| c.name).collect();
                vec![
                    r("widget_a", "Віджет зліва", "", ch(&w, i.widget_a as usize)),
                    r("widget_b", "Віджет справа", "", ch(&w, i.widget_b as usize)),
                    r("world_city", "Світовий час", "Місто для віджета годинника", ch(&cities, i.world_city as usize)),
                    r("cd_title", "Зворотний відлік: назва", "", Ctl::Text(i.countdown_title.clone())),
                    r("cd_date", "Зворотний відлік: дата", "Формат РРРР-ММ-ДД", Ctl::Text(i.countdown_date.clone())),
                ]
            }
            8 => vec![
                r("hud_volume", "Гучність", "Індикатор при зміні гучності", Ctl::Toggle(i.hud_volume)),
                r("hud_brightness", "Яскравість", "Індикатор при зміні яскравості екрана", Ctl::Toggle(i.hud_brightness)),
                r("hud_caps", "Caps Lock", "", Ctl::Toggle(i.hud_caps)),
                r("hud_layout", "Мова клавіатури", "При перемиканні розкладки", Ctl::Toggle(i.hud_layout)),
                r("card_charge", "Зарядка", "Картка з відсотком при підключенні", Ctl::Toggle(i.battery_notify)),
                r("card_low", "Низький заряд", "Попередження на 20% і 10%", Ctl::Toggle(i.battery_low)),
                r("card_wifi", "Wi-Fi", "Підключення та втрата мережі", Ctl::Toggle(i.card_wifi)),
                r("card_audio", "Навушники та колонки", "Нове аудіопристрій, наприклад Bluetooth-навушники", Ctl::Toggle(i.card_audio)),
            ],
            _ => vec![
                r("about", &format!("DockX {}", self.version), "Док і Dynamic Island для Windows 10/11 · by X44TO", Ctl::Info("Rust".into())),
                r("about2", "Тексти пісень", "LRCLIB — відкрита база синхронних текстів", Ctl::Info("lrclib.net".into())),
                r("about3", "Сповіщення Windows", "Перехоплення системних сповіщень Windows дозволяє лише програмам з Microsoft Store", Ctl::Info("—".into())),
            ],
        }
    }

    fn apply(&mut self, id: &'static str, v: i32) {
        let a = &mut self.app;
        let i = &mut a.island;
        let pick = |list: &[u32], k: i32| list[(k.max(0) as usize).min(list.len() - 1)];
        match id {
            "autostart" => a.autostart = !a.autostart,
            "taskbar" => a.taskbar_autohide = !a.taskbar_autohide,
            "fullscreen" => {
                let on = !(a.dock.hide_in_fullscreen && i.hide_in_fullscreen);
                a.dock.hide_in_fullscreen = on;
                i.hide_in_fullscreen = on;
            }
            "show_dock" => a.show_dock = !a.show_dock,
            "show_island" => i.enabled = !i.enabled,
            "folder" => {
                self.actions.push(SettingsAction::OpenFolder);
                return;
            }
            "reset" => {
                let (keep_autostart, keep_taskbar) = (a.autostart, a.taskbar_autohide);
                *a = AppSettings::default();
                a.autostart = keep_autostart;
                a.taskbar_autohide = keep_taskbar;
                self.actions.push(SettingsAction::ResetAll);
            }
            "icon_size" => a.dock.icon_size = pick(&[40, 48, 56, 64, 72, 84], v) as i32,
            "magnify" => a.dock.magnification = [1.0, 1.35, 1.7, 2.05][v.clamp(0, 3) as usize],
            "autohide" => a.dock.auto_hide = !a.dock.auto_hide,
            "isl_size" => i.size = v.clamp(0, 2) as u8,
            "isl_style" => i.floating = v == 1,
            "minimal" => i.minimal = !i.minimal,
            "glass" => i.glass = !i.glass,
            "clock" => i.clock = !i.clock,
            "ring" => i.ring = v.clamp(0, 2) as u8,
            "mascot" => i.mascot = v.clamp(0, 2) as u8,
            "lyrics_online" => i.lyrics_online = !i.lyrics_online,
            "lyrics_pill" => i.lyrics_pill = !i.lyrics_pill,
            "clip_on" => i.clip_on = !i.clip_on,
            "clip_max" => i.clip_max = pick(&[5, 10, 20, 50], v),
            "focus" => i.focus_min = pick(&[15, 25, 45, 50], v),
            "break" => i.break_min = pick(&[3, 5, 10], v),
            "long_break" => i.long_break_min = pick(&[10, 15, 20, 30], v),
            "water_on" => i.water_on = !i.water_on,
            "cup" => i.cup_ml = pick(&[150, 200, 250, 330, 500], v),
            "goal" => i.goal_ml = pick(&[1500, 2000, 2500, 3000], v),
            "interval" => i.water_interval = pick(&[30, 45, 60, 90, 120], v),
            "widget_a" => i.widget_a = v.rem_euclid(WIDGETS.len() as i32) as u8,
            "widget_b" => i.widget_b = v.rem_euclid(WIDGETS.len() as i32) as u8,
            "world_city" => i.world_city = v.rem_euclid(time::CITIES.len() as i32) as u8,
            "hud_volume" => i.hud_volume = !i.hud_volume,
            "hud_brightness" => i.hud_brightness = !i.hud_brightness,
            "hud_caps" => i.hud_caps = !i.hud_caps,
            "hud_layout" => i.hud_layout = !i.hud_layout,
            "card_charge" => i.battery_notify = !i.battery_notify,
            "card_low" => i.battery_low = !i.battery_low,
            "card_wifi" => i.card_wifi = !i.card_wifi,
            "card_audio" => i.card_audio = !i.card_audio,
            _ => return,
        }
        self.actions.push(SettingsAction::Changed);
        self.need_render = true;
    }

    pub fn step(&mut self, now: f64) -> bool {
        let dt = if self.last_step < 0.0 { 0.016 } else { (now - self.last_step).clamp(0.001, 0.05) } as f32;
        self.last_step = now;
        self.now = now;
        let anim = self.appear.step(1.0, 0.08, dt, 0.002);
        if anim || self.need_render || self.editing.is_some() {
            self.render();
        }
        anim || self.editing.is_some()
    }

    pub fn kick(&mut self) {
        self.need_render = true;
    }

    fn render(&mut self) {
        self.need_render = false;
        self.rendered += 1;
        let (w, h) = self.size();
        let mut g = std::mem::replace(&mut self.canvas, Canvas::new(1, 1));
        g.reset(w, h);
        self.zones.clear();
        let s = self.s;
        let m = self.margin();
        let k = self.appear.v;
        let a = smoothstep(0.0, 1.0, k);
        let panel = rect(m, m + (1.0 - k) * 14.0 * s, w as f32 - 2.0 * m, h as f32 - 2.0 * m);
        shadow(&mut g, panel, 18.0 * s, s * 1.4, a, false);
        let Some(pp) = rrect(panel, 18.0 * s) else { return };
        g.vgradient(&pp, panel.y, panel.b(), rgba(26, 26, 29, (250.0 * a) as u8), rgba(12, 12, 14, (252.0 * a) as u8));
        g.stroke_path(&pp, RIM.alpha(a), s.max(1.0));
        g.set_clip(Some(&pp), None);
        let fonts = self.fonts.clone();
        // header
        let header = rect(panel.x, panel.y, panel.w, 58.0 * s);
        self.zones.push((header, SHot::Header));
        g.fill_circle(header.x + 34.0 * s, header.cy(), 13.0 * s, ACCENT.alpha(a));
        g.text_in(&fonts.bold, "X", 13.0 * s, rect(header.x + 21.0 * s, header.y, 26.0 * s, header.h), Align::Center, TEXT.alpha(a));
        g.text(&fonts.semibold, "Налаштування DockX", 16.0 * s, header.x + 58.0 * s, header.cy() + 6.0 * s, TEXT.alpha(a));
        let close = rect(header.r() - 46.0 * s, header.cy() - 15.0 * s, 30.0 * s, 30.0 * s);
        self.zones.push((close, SHot::Close));
        if self.hot == Some(SHot::Close) {
            g.fill_circle(close.cx(), close.cy(), 15.0 * s, ACCENT_DARK.alpha(a));
        }
        icons::draw(&mut g, Icon::Close, close.cx(), close.cy(), 13.0 * s, TEXT2.alpha(a));
        g.line(panel.x, header.b(), panel.r(), header.b(), rgba(255, 255, 255, (20.0 * a) as u8), 1.0);
        // sidebar
        let side = rect(panel.x, header.b(), 214.0 * s, panel.b() - header.b());
        g.fill_rect(side, rgba(0, 0, 0, (60.0 * a) as u8));
        let mut y = side.y + 12.0 * s;
        let icons_for = [Icon::Select, Icon::Shelf, Icon::Bell, Icon::Music, Icon::Shelf, Icon::Timer, Icon::Drop, Icon::Select, Icon::Lyrics, Icon::Speaker, Icon::Bell];
        for (i, name) in SECTIONS.iter().enumerate() {
            let r = rect(side.x + 10.0 * s, y, side.w - 20.0 * s, 34.0 * s);
            self.zones.push((r, SHot::Section(i)));
            let active = i == self.section;
            if active {
                g.fill_rrect(r, 10.0 * s, ACCENT_DARK.alpha(a));
            } else if self.hot == Some(SHot::Section(i)) {
                g.fill_rrect(r, 10.0 * s, CONTROL.alpha(a));
            }
            icons::draw(&mut g, icons_for[i], r.x + 20.0 * s, r.cy(), 13.0 * s, if active { TEXT } else { TEXT2 }.alpha(a));
            g.text_in(&fonts.regular, name, 13.0 * s, rect(r.x + 40.0 * s, r.y, r.w - 44.0 * s, r.h), Align::Left, if active { TEXT } else { TEXT2 }.alpha(a));
            y += 38.0 * s;
        }
        // content
        let content = rect(side.r() + 28.0 * s, header.b() + 18.0 * s, panel.r() - side.r() - 56.0 * s, panel.b() - header.b() - 36.0 * s);
        g.text(&fonts.bold, SECTIONS[self.section], 21.0 * s, content.x, content.y + 20.0 * s, TEXT.alpha(a));
        let list = rect(content.x, content.y + 40.0 * s, content.w, content.h - 40.0 * s);
        let rows = self.rows();
        let row_h = 62.0 * s;
        self.max_scroll = (rows.len() as f32 * row_h - list.h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
        let mut y = list.y - self.scroll;
        let clip_path = rrect(list, 0.0);
        g.set_clip(clip_path.as_ref(), Some(list));
        for (ri, row) in rows.iter().enumerate() {
            let rr = rect(list.x, y, list.w, row_h);
            y += row_h;
            if rr.b() < list.y || rr.y > list.b() {
                continue;
            }
            if ri > 0 {
                g.line(rr.x, rr.y, rr.r(), rr.y, rgba(255, 255, 255, (16.0 * a) as u8), 1.0);
            }
            let ctl_w = self.draw_ctl(&mut g, ri, row, rr, a);
            let lw = rr.w - ctl_w - 20.0 * s;
            let label = fonts.semibold.ellipsize(&row.label, 14.0 * s, lw);
            if row.sub.is_empty() {
                g.text_in(&fonts.semibold, &label, 14.0 * s, rr, Align::Left, TEXT.alpha(a));
            } else {
                g.text(&fonts.semibold, &label, 14.0 * s, rr.x, rr.cy() - 3.0 * s, TEXT.alpha(a));
                let sub = fonts.regular.ellipsize(&row.sub, 11.5 * s, lw);
                g.text(&fonts.regular, &sub, 11.5 * s, rr.x, rr.cy() + 15.0 * s, TEXT3.alpha(a));
            }
        }
        g.no_clip();
        self.canvas = g;
        self.update_hot();
    }

    /// Draws the control at the right of the row; returns its width.
    fn draw_ctl(&mut self, g: &mut Canvas, ri: usize, row: &Row, rr: RectF, a: f32) -> f32 {
        let s = self.s;
        let fonts = self.fonts.clone();
        match &row.ctl {
            Ctl::Toggle(on) => {
                let t = rect(rr.r() - 48.0 * s, rr.cy() - 13.0 * s, 48.0 * s, 26.0 * s);
                self.zones.push((t, SHot::Ctl(ri, 0)));
                let hot = self.hot == Some(SHot::Ctl(ri, 0));
                g.fill_rrect(t, 13.0 * s, if *on { if hot { ACCENT } else { ACCENT_DARK.mix(ACCENT, 0.6) } } else if hot { rgba(80, 80, 88, 255) } else { rgba(62, 62, 68, 255) }.alpha(a));
                let kx = if *on { t.r() - 13.0 * s } else { t.x + 13.0 * s };
                g.fill_circle(kx, t.cy(), 10.0 * s, TEXT.alpha(a));
                t.w
            }
            Ctl::Choice(opts, k) => {
                let size = 12.0 * s;
                let widths: Vec<f32> = opts.iter().map(|o| fonts.regular.measure(o, size) + 22.0 * s).collect();
                let total: f32 = widths.iter().sum::<f32>() + 8.0 * s;
                if total <= 360.0 * s && opts.len() <= 6 {
                    let bg = rect(rr.r() - total, rr.cy() - 16.0 * s, total, 32.0 * s);
                    g.fill_rrect(bg, 16.0 * s, rgba(255, 255, 255, (14.0 * a) as u8));
                    let mut x = bg.x + 4.0 * s;
                    for (j, o) in opts.iter().enumerate() {
                        let cr = rect(x, bg.y + 4.0 * s, widths[j], 24.0 * s);
                        self.zones.push((cr, SHot::Ctl(ri, j as i32)));
                        let sel = j == *k;
                        if sel {
                            g.fill_rrect(cr, 12.0 * s, ACCENT_DARK.alpha(a));
                        } else if self.hot == Some(SHot::Ctl(ri, j as i32)) {
                            g.fill_rrect(cr, 12.0 * s, CONTROL_HOVER.alpha(a));
                        }
                        g.text_in(&fonts.regular, o, size, cr, Align::Center, if sel { TEXT } else { TEXT2 }.alpha(a));
                        x += widths[j];
                    }
                    total
                } else {
                    // ‹ value ›
                    let w = 250.0 * s;
                    let bg = rect(rr.r() - w, rr.cy() - 16.0 * s, w, 32.0 * s);
                    g.fill_rrect(bg, 16.0 * s, rgba(255, 255, 255, (14.0 * a) as u8));
                    let l = rect(bg.x + 2.0 * s, bg.y + 2.0 * s, 28.0 * s, 28.0 * s);
                    let r = rect(bg.r() - 30.0 * s, bg.y + 2.0 * s, 28.0 * s, 28.0 * s);
                    for (rc, d, sub) in [(l, -1.0f32, -1), (r, 1.0, -2)] {
                        self.zones.push((rc, SHot::Ctl(ri, sub)));
                        if self.hot == Some(SHot::Ctl(ri, sub)) {
                            g.fill_circle(rc.cx(), rc.cy(), 14.0 * s, CONTROL_HOVER.alpha(a));
                        }
                        g.line(rc.cx() - d * 3.0 * s, rc.cy() - 5.0 * s, rc.cx() + d * 3.0 * s, rc.cy(), TEXT2.alpha(a), 1.6 * s);
                        g.line(rc.cx() + d * 3.0 * s, rc.cy(), rc.cx() - d * 3.0 * s, rc.cy() + 5.0 * s, TEXT2.alpha(a), 1.6 * s);
                    }
                    let v = fonts.regular.ellipsize(&opts[*k], 12.5 * s, w - 70.0 * s);
                    g.text_in(&fonts.regular, &v, 12.5 * s, bg, Align::Center, TEXT.alpha(a));
                    w
                }
            }
            Ctl::Text(v) => {
                let w = 230.0 * s;
                let f = rect(rr.r() - w, rr.cy() - 16.0 * s, w, 32.0 * s);
                self.zones.push((f, SHot::Ctl(ri, 0)));
                let editing = self.editing.as_ref().filter(|(id, _)| *id == row.id).map(|(_, t)| t.clone());
                g.fill_rrect(f, 10.0 * s, if editing.is_some() { rgba(255, 255, 255, 26) } else if self.hot == Some(SHot::Ctl(ri, 0)) { CONTROL_HOVER } else { CONTROL }.alpha(a));
                if editing.is_some() {
                    g.stroke_rrect(f, 10.0 * s, ACCENT.alpha(a), 1.4 * s);
                }
                let size = 12.5 * s;
                let base = f.cy() + fonts.regular.cap_height(size) / 2.0;
                match editing {
                    Some(t) => {
                        let txt = t.string();
                        g.text(&fonts.regular, &txt, size, f.x + 12.0 * s, base, TEXT.alpha(a));
                        if t.caret_visible(self.now) {
                            let cx = f.x + 12.0 * s + fonts.regular.measure(&t.before_caret(), size);
                            g.fill_rect(rect(cx, f.cy() - 8.0 * s, 1.4 * s, 16.0 * s), TEXT.alpha(a));
                        }
                    }
                    None => {
                        let t = fonts.regular.ellipsize(v, size, w - 24.0 * s);
                        g.text(&fonts.regular, &t, size, f.x + 12.0 * s, base, TEXT.alpha(a));
                    }
                }
                w
            }
            Ctl::Button(label) => {
                let tw = fonts.semibold.measure(label, 12.5 * s);
                let br = rect(rr.r() - tw - 30.0 * s, rr.cy() - 15.0 * s, tw + 30.0 * s, 30.0 * s);
                self.zones.push((br, SHot::Ctl(ri, 0)));
                chip(g, br, self.hot == Some(SHot::Ctl(ri, 0)), row.id == "reset", a);
                g.text_in(&fonts.semibold, label, 12.5 * s, br, Align::Center, TEXT.alpha(a));
                br.w
            }
            Ctl::Info(t) => {
                let tw = fonts.regular.measure(t, 12.5 * s);
                g.text(&fonts.regular, t, 12.5 * s, rr.r() - tw, rr.cy() + 4.0 * s, TEXT3.alpha(a));
                tw
            }
        }
    }

    fn update_hot(&mut self) {
        let (x, y) = self.cursor;
        let h = self.zones.iter().rev().find(|(r, _)| r.contains(x, y)).map(|(_, h)| *h);
        if h != self.hot {
            self.hot = h;
            self.need_render = true;
        }
    }

    fn hit(&self, x: f32, y: f32) -> Option<SHot> {
        self.zones.iter().rev().find(|(r, _)| r.contains(x, y)).map(|(_, h)| *h)
    }

    /// Coordinates are window-local.
    pub fn mouse_move(&mut self, x: f32, y: f32) {
        self.cursor = (x, y);
        self.update_hot();
    }

    pub fn mouse_down(&mut self, btn: Btn, x: f32, y: f32) {
        if btn != Btn::Left {
            return;
        }
        let h = self.hit(x, y);
        if let Some((_, _)) = &self.editing {
            if !matches!(h, Some(SHot::Ctl(_, 0))) {
                self.commit_edit();
            }
        }
        if h == Some(SHot::Header) {
            self.actions.push(SettingsAction::StartMove);
            return;
        }
        self.pressed = h;
    }

    pub fn mouse_up(&mut self, btn: Btn, x: f32, y: f32) {
        if btn != Btn::Left {
            return;
        }
        let h = self.hit(x, y);
        let p = self.pressed.take();
        if h.is_none() || h != p {
            return;
        }
        match h.unwrap() {
            SHot::Close => self.actions.push(SettingsAction::Close),
            SHot::Section(i) => {
                self.commit_edit();
                self.section = i;
                self.scroll = 0.0;
            }
            SHot::Ctl(ri, sub) => {
                let rows = self.rows();
                let Some(row) = rows.get(ri) else { return };
                match &row.ctl {
                    Ctl::Toggle(_) | Ctl::Button(_) => self.apply(row.id, 0),
                    Ctl::Choice(opts, k) => {
                        let v = match sub {
                            -1 => (*k as i32 - 1).rem_euclid(opts.len() as i32),
                            -2 => (*k as i32 + 1).rem_euclid(opts.len() as i32),
                            j => j,
                        };
                        self.apply(row.id, v);
                    }
                    Ctl::Text(v) => {
                        let mut t = TextField::new(v, true);
                        t.last_input = self.now;
                        self.editing = Some((row.id, t));
                        self.actions.push(SettingsAction::WantKeyboard(true));
                    }
                    Ctl::Info(_) => {}
                }
            }
            SHot::Header => {}
        }
        self.need_render = true;
    }

    pub fn wheel(&mut self, delta: f32) {
        self.scroll = (self.scroll - delta * 40.0 * self.s).clamp(0.0, self.max_scroll);
        self.need_render = true;
    }

    pub fn key(&mut self, k: Key) {
        let now = self.now;
        let Some((id, f)) = &mut self.editing else {
            if k == Key::Escape {
                self.actions.push(SettingsAction::Close);
            }
            return;
        };
        let id = *id;
        match f.key(k, now) {
            FieldResult::Submit(v) => {
                self.set_text(id, v);
                self.editing = None;
                self.actions.push(SettingsAction::WantKeyboard(false));
            }
            FieldResult::Cancel => {
                self.editing = None;
                self.actions.push(SettingsAction::WantKeyboard(false));
            }
            FieldResult::None => {}
        }
        self.need_render = true;
    }

    pub fn paste(&mut self, s: &str) {
        let now = self.now;
        if let Some((_, f)) = &mut self.editing {
            f.paste(s, now);
            self.need_render = true;
        }
    }

    fn commit_edit(&mut self) {
        if let Some((id, f)) = self.editing.take() {
            self.set_text(id, f.string());
            self.actions.push(SettingsAction::WantKeyboard(false));
        }
    }

    fn set_text(&mut self, id: &'static str, v: String) {
        let v = v.trim().to_string();
        match id {
            "cd_title" if !v.is_empty() => self.app.island.countdown_title = v,
            "cd_date" if time::parse_iso(&v).is_some() => self.app.island.countdown_date = v,
            _ => return,
        }
        self.actions.push(SettingsAction::Changed);
    }

    /// Demo hook: screen-local rect of a section or a control (row index within the current section).
    pub fn zone_of(&self, what: &str) -> Option<RectF> {
        self.zones
            .iter()
            .find(|(_, h)| match (h, what.split_once(':')) {
                (SHot::Section(i), Some(("section", n))) => n.parse::<usize>().ok() == Some(*i),
                (SHot::Ctl(r, sub), Some((rs, ss))) => rs.parse::<usize>().ok() == Some(*r) && ss.parse::<i32>().ok() == Some(*sub),
                (SHot::Close, None) => what == "close",
                _ => false,
            })
            .map(|(r, _)| *r)
    }
}
