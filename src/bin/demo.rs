//! Renders the DockX demo video with the real dock + island code (same drawing, same springs).
//! Usage: demo <out_dir> [stills]
//! Frames are written as raw RGBA to stdout-friendly files or piped to ffmpeg by the shell script.

use dockx::ui::dock::*;
use dockx::ui::gfx::*;
use dockx::ui::island::*;
use dockx::ui::lrc;
use dockx::ui::text_field::Key;
use dockx::ui::theme::*;
use dockx::ui::{Btn, Mods};
use dockx::ui::island::Hud;
use std::io::Write;
use std::rc::Rc;
use tiny_skia::{PathBuilder, Pixmap};

const W: u32 = 1920;
const H: u32 = 1080;
const FPS: f64 = 60.0;
const S: f32 = 1.25;

fn load_fonts() -> Fonts {
    let dir = "/usr/share/fonts/opentype/inter/";
    let f = |n: &str| Font::from_bytes(&std::fs::read(format!("{dir}{n}")).expect("font")).expect("font parse");
    Fonts { regular: f("Inter-Regular.otf"), semibold: f("Inter-SemiBold.otf"), bold: f("Inter-Bold.otf") }
}

// ───────────────────────── generated art (all original) ─────────────────────────

fn wallpaper() -> Pixmap {
    let mut c = Canvas::new(W, H);
    let full = rrect(rect(0.0, 0.0, W as f32, H as f32), 0.0).unwrap();
    c.gradient(&full, (0.0, 0.0), (W as f32, H as f32), &[(0.0, rgba(18, 8, 14, 255)), (0.55, rgba(10, 10, 16, 255)), (1.0, rgba(6, 6, 10, 255))]);
    c.radial(1450.0, 260.0, 900.0, rgba(150, 20, 34, 150), rgba(150, 20, 34, 0));
    c.radial(300.0, 900.0, 800.0, rgba(90, 16, 60, 120), rgba(90, 16, 60, 0));
    c.radial(980.0, 640.0, 520.0, rgba(226, 38, 50, 40), rgba(226, 38, 50, 0));
    // soft diagonal light streaks
    for i in 0..5 {
        let x = 300.0 + i as f32 * 340.0;
        let mut pb = PathBuilder::new();
        pb.move_to(x, 0.0);
        pb.line_to(x + 120.0, 0.0);
        pb.line_to(x - 380.0, H as f32);
        pb.line_to(x - 500.0, H as f32);
        pb.close();
        if let Some(p) = pb.finish() {
            c.fill_path(&p, rgba(255, 255, 255, 5));
        }
    }
    c.pm
}

#[derive(Clone, Copy)]
enum Glyph {
    Folder,
    Globe,
    Chat,
    Code,
    Game,
    Note,
    Terminal,
    Gear,
    Rocket,
    Doc,
    Image,
    Zip,
}

fn app_icon(fonts: &Fonts, glyph: Glyph, c1: Rgba, c2: Rgba) -> Pixmap {
    let n = 256.0;
    let mut c = Canvas::new(256, 256);
    let r = rect(14.0, 14.0, n - 28.0, n - 28.0);
    let p = rrect(r, 56.0).unwrap();
    c.gradient(&p, (r.x, r.y), (r.x, r.b()), &[(0.0, c1), (1.0, c2)]);
    c.stroke_path(&p, rgba(255, 255, 255, 40), 3.0);
    let w = rgba(255, 255, 255, 245);
    let (cx, cy) = (128.0, 128.0);
    match glyph {
        Glyph::Folder => {
            c.fill_rrect(rect(56.0, 78.0, 70.0, 40.0), 12.0, rgba(255, 255, 255, 200));
            c.fill_rrect(rect(56.0, 96.0, 144.0, 92.0), 16.0, w);
        }
        Glyph::Globe => {
            c.stroke_circle(cx, cy, 64.0, w, 10.0);
            c.line(cx - 64.0, cy, cx + 64.0, cy, w, 9.0);
            if let Some(p) = PathBuilder::from_oval(tiny_skia::Rect::from_xywh(cx - 28.0, cy - 64.0, 56.0, 128.0).unwrap()) {
                c.stroke_path(&p, w, 9.0);
            }
        }
        Glyph::Chat => {
            c.fill_rrect(rect(58.0, 66.0, 140.0, 100.0), 34.0, w);
            let mut pb = PathBuilder::new();
            pb.move_to(86.0, 150.0);
            pb.line_to(76.0, 194.0);
            pb.line_to(124.0, 160.0);
            pb.close();
            c.fill_path(&pb.finish().unwrap(), w);
            for k in 0..3 {
                c.fill_circle(96.0 + k as f32 * 32.0, 116.0, 9.0, c2);
            }
        }
        Glyph::Code => {
            let mut pb = PathBuilder::new();
            pb.move_to(102.0, 84.0);
            pb.line_to(62.0, 128.0);
            pb.line_to(102.0, 172.0);
            pb.move_to(154.0, 84.0);
            pb.line_to(194.0, 128.0);
            pb.line_to(154.0, 172.0);
            c.stroke_path_cap(&pb.finish().unwrap(), w, 16.0, tiny_skia::LineCap::Round);
            c.line(140.0, 74.0, 116.0, 182.0, rgba(255, 255, 255, 180), 12.0);
        }
        Glyph::Game => {
            c.fill_rrect(rect(52.0, 92.0, 152.0, 84.0), 42.0, w);
            c.line(88.0, 134.0, 116.0, 134.0, c2, 10.0);
            c.line(102.0, 120.0, 102.0, 148.0, c2, 10.0);
            c.fill_circle(156.0, 124.0, 9.0, c2);
            c.fill_circle(174.0, 144.0, 9.0, c2);
        }
        Glyph::Note => dockx::ui::icons::draw(&mut c, dockx::ui::icons::Icon::Music, cx, cy, 150.0, w),
        Glyph::Terminal => {
            c.fill_rrect(rect(50.0, 66.0, 156.0, 124.0), 18.0, rgba(0, 0, 0, 120));
            let mut pb = PathBuilder::new();
            pb.move_to(76.0, 104.0);
            pb.line_to(104.0, 128.0);
            pb.line_to(76.0, 152.0);
            c.stroke_path_cap(&pb.finish().unwrap(), w, 12.0, tiny_skia::LineCap::Round);
            c.line(120.0, 156.0, 168.0, 156.0, w, 12.0);
        }
        Glyph::Gear => {
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                c.line(cx + a.sin() * 40.0, cy - a.cos() * 40.0, cx + a.sin() * 74.0, cy - a.cos() * 74.0, w, 26.0);
            }
            c.fill_circle(cx, cy, 56.0, w);
            c.fill_circle(cx, cy, 22.0, c2);
        }
        Glyph::Rocket => {
            // speedometer: OptimaX
            c.arc(cx, cy + 16.0, 70.0, -2.2, 4.4, w, 14.0);
            c.line(cx, cy + 16.0, cx + 40.0, cy - 30.0, w, 12.0);
            c.fill_circle(cx, cy + 16.0, 14.0, w);
            c.text_in(&fonts.bold, "X", 40.0, rect(cx - 30.0, cy + 34.0, 60.0, 40.0), Align::Center, w);
        }
        Glyph::Doc | Glyph::Image | Glyph::Zip => {
            c.fill_rrect(rect(72.0, 50.0, 112.0, 156.0), 14.0, w);
            match glyph {
                Glyph::Doc => {
                    for k in 0..5 {
                        c.line(92.0, 88.0 + k as f32 * 22.0, if k == 4 { 136.0 } else { 164.0 }, 88.0 + k as f32 * 22.0, c2, 8.0);
                    }
                }
                Glyph::Image => {
                    c.fill_circle(108.0, 96.0, 14.0, c1);
                    let mut pb = PathBuilder::new();
                    pb.move_to(84.0, 180.0);
                    pb.line_to(120.0, 130.0);
                    pb.line_to(142.0, 156.0);
                    pb.line_to(156.0, 140.0);
                    pb.line_to(174.0, 180.0);
                    pb.close();
                    c.fill_path(&pb.finish().unwrap(), c2);
                }
                _ => {
                    for k in 0..6 {
                        c.fill_rect(rect(120.0 + (k % 2) as f32 * 14.0, 60.0 + k as f32 * 16.0, 14.0, 10.0), c2);
                    }
                    c.fill_rrect(rect(114.0, 156.0, 30.0, 30.0), 6.0, c2);
                }
            }
        }
    }
    c.pm
}

fn album_art(seed: u32) -> Pixmap {
    let mut c = Canvas::new(256, 256);
    let p = rrect(rect(0.0, 0.0, 256.0, 256.0), 0.0).unwrap();
    let (a, b) = if seed == 0 { (rgba(255, 70, 60, 255), rgba(60, 8, 40, 255)) } else { (rgba(70, 160, 255, 255), rgba(20, 10, 60, 255)) };
    c.gradient(&p, (0.0, 0.0), (256.0, 256.0), &[(0.0, a), (1.0, b)]);
    // abstract skyline + sun
    c.fill_circle(176.0, 96.0, 46.0, rgba(255, 220, 160, 230));
    for i in 0..9 {
        let x = i as f32 * 30.0 - 6.0;
        let hgt = 60.0 + ((i * 37 + seed as usize * 11) % 90) as f32;
        c.fill_rect(rect(x, 256.0 - hgt, 26.0, hgt), rgba(10, 6, 16, 235));
        for k in 0..(hgt as i32 / 18) {
            if (i + k as usize) % 3 != 0 {
                c.fill_rect(rect(x + 6.0, 256.0 - hgt + 8.0 + k as f32 * 18.0, 5.0, 6.0), rgba(255, 200, 120, 200));
            }
        }
    }
    c.pm
}

const LYRICS: &str = "[00:00.00]♪
[00:04.00]Місто не спить, і я теж не сплю
[00:08.50]Неонові вогні малюють мою тінь
[00:13.00]Кожен світлофор — як новий рівень
[00:17.50]Я їду крізь ніч, поки грає цей біт
[00:22.00]Вікна горять, як пікселі екрану
[00:26.50]Я збираю цей вечір по кадру
[00:31.00]Ще одне коло, ще один поворот
[00:35.50]І вся ця ніч — мій власний код
[00:40.00]Нічне місто, світло на склі
[00:44.50]Ми на хвилі, ми на землі
[00:49.00]Нічне місто, тільки вперед
[00:53.50]Поки бас тримає мій серцевий ритм
[00:58.00]♪";

// ───────────────────────── cursor & timeline ─────────────────────────

fn draw_cursor(c: &mut Canvas, x: f32, y: f32, pressed: bool, grab: Option<&Pixmap>) {
    if let Some(img) = grab {
        c.image(img, rect(x + 10.0, y + 14.0, 56.0, 56.0), 0.85);
    }
    let k = if pressed { 0.9 } else { 1.0 };
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x, y + 26.0 * k);
    pb.line_to(x + 6.5 * k, y + 20.0 * k);
    pb.line_to(x + 11.0 * k, y + 30.0 * k);
    pb.line_to(x + 15.0 * k, y + 28.0 * k);
    pb.line_to(x + 10.5 * k, y + 18.5 * k);
    pb.line_to(x + 19.0 * k, y + 18.5 * k);
    pb.close();
    let p = pb.finish().unwrap();
    c.fill_path(&p, rgba(0, 0, 0, 255));
    c.stroke_path(&p, rgba(255, 255, 255, 255), 1.6);
    if pressed {
        c.stroke_circle(x, y, 16.0, rgba(255, 255, 255, 120), 2.0);
    }
}

// ───────────────────────── script ─────────────────────────

#[derive(Clone)]
enum Ev {
    Down(Btn),
    Up(Btn),
    DownMods(Btn, Mods),
    Wheel(f32),
    DragDock,
    DragIsland,
    DropDock(Vec<String>),
    DropIsland(Vec<String>),
    Type(String),
    Key(Key),
    Caption(String),
    Notice(NoticeKind, String),
    FocusEndsIn(f64),
    Media(bool),
    Copy(String),
    Hud(Hud),
    Battery(u8, bool),
    Wifi(Option<String>),
    Audio(String),
    IdleJump(f64),
    Cfg(fn(&mut IslandSettings)),
    DockCfg(bool),
    Grab(Option<usize>),
    GrabText(Option<String>),
    SetRunning(usize, bool),
    Fullscreen(bool),
    FocusReset,
}

#[derive(Clone, Copy)]
enum Tg {
    Same,
    At(f32, f32),
    Isl(&'static str, f32, f32),
    Dock(usize, f32, f32),
    Set(&'static str, f32, f32),
    Menu(usize),
    LyricRel(i32),
}

struct Script {
    path: Vec<(f64, Tg)>,
    resolved: Vec<Option<(f32, f32)>>,
    events: Vec<(f64, Ev)>,
    t: f64,
}

impl Script {
    fn new() -> Script {
        Script { path: vec![(0.0, Tg::At(1300.0, 620.0))], resolved: vec![Some((1300.0, 620.0))], events: Vec::new(), t: 0.0 }
    }
    fn go(&mut self, tg: Tg, dur: f64) -> &mut Self {
        self.t += dur;
        self.path.push((self.t, tg));
        self.resolved.push(None);
        self
    }
    fn mv(&mut self, x: f32, y: f32, d: f64) -> &mut Self {
        self.go(Tg::At(x, y), d)
    }
    fn isl(&mut self, n: &'static str, d: f64) -> &mut Self {
        self.go(Tg::Isl(n, 0.5, 0.5), d)
    }
    fn isl_at(&mut self, n: &'static str, fx: f32, fy: f32, d: f64) -> &mut Self {
        self.go(Tg::Isl(n, fx, fy), d)
    }
    fn dock(&mut self, i: usize, fx: f32, dy: f32, d: f64) -> &mut Self {
        self.go(Tg::Dock(i, fx, dy), d)
    }
    fn set(&mut self, n: &'static str, d: f64) -> &mut Self {
        self.go(Tg::Set(n, 0.5, 0.5), d)
    }
    fn menu(&mut self, i: usize, d: f64) -> &mut Self {
        self.go(Tg::Menu(i), d)
    }
    fn wait(&mut self, d: f64) -> &mut Self {
        self.go(Tg::Same, d)
    }
    fn ev(&mut self, e: Ev) -> &mut Self {
        self.events.push((self.t, e));
        self
    }
    fn click(&mut self) -> &mut Self {
        self.ev(Ev::Down(Btn::Left)).wait(0.09).ev(Ev::Up(Btn::Left)).wait(0.06)
    }
    fn rclick(&mut self) -> &mut Self {
        self.ev(Ev::Down(Btn::Right)).wait(0.08).ev(Ev::Up(Btn::Right)).wait(0.05)
    }
    fn cap(&mut self, s: &str) -> &mut Self {
        self.ev(Ev::Caption(s.into()))
    }
}

/// Demo stand-in for the native context menu (same colours as the owner-drawn one in the app).
struct FakeMenu {
    x: f32,
    y: f32,
    items: Vec<(String, u32)>,
    t: f64,
}

impl FakeMenu {
    fn item_h() -> f32 {
        30.0 * S
    }
    fn width(&self, f: &Fonts) -> f32 {
        self.items.iter().map(|(t, _)| f.regular.measure(t, 13.0 * S)).fold(0.0, f32::max) + 64.0 * S
    }
    fn rect_of(&self, f: &Fonts, i: usize) -> RectF {
        let w = self.width(f);
        let mut y = self.y + 4.0 * S;
        for (k, (t, _)) in self.items.iter().enumerate() {
            let h = if t.is_empty() { 9.0 * S } else { Self::item_h() };
            if k == i {
                return rect(self.x + 2.0, y, w - 4.0, h);
            }
            y += h;
        }
        rect(self.x, y, w, 0.0)
    }
    fn total_h(&self) -> f32 {
        self.items.iter().map(|(t, _)| if t.is_empty() { 9.0 * S } else { Self::item_h() }).sum::<f32>() + 8.0 * S
    }
    fn hit(&self, f: &Fonts, x: f32, y: f32) -> Option<usize> {
        (0..self.items.len()).find(|&i| !self.items[i].0.is_empty() && self.rect_of(f, i).contains(x, y))
    }
    fn draw(&self, c: &mut Canvas, f: &Fonts, cur: (f32, f32), now: f64) {
        let a = ((now - self.t) / 0.12).min(1.0) as f32;
        let r = rect(self.x, self.y, self.width(f), self.total_h());
        shadow(c, r, 6.0 * S, S, a, false);
        c.fill_rect(r, rgba(22, 22, 25, (255.0 * a) as u8));
        c.stroke_rrect(r, 0.0, rgba(58, 58, 64, (255.0 * a) as u8), 1.0);
        let hot = self.hit(f, cur.0, cur.1);
        for (i, (t, _)) in self.items.iter().enumerate() {
            let ir = self.rect_of(f, i);
            if t.is_empty() {
                c.line(ir.x + 10.0 * S, ir.cy(), ir.r() - 10.0 * S, ir.cy(), rgba(52, 52, 58, (255.0 * a) as u8), 1.0);
                continue;
            }
            if hot == Some(i) {
                c.fill_rrect(ir.inflate(-4.0 * S, -1.0), 5.0 * S, rgba(150, 26, 34, (255.0 * a) as u8));
            }
            let txt = t.trim_end_matches('▸');
            c.text_in(&f.regular, txt, 13.0 * S, rect(ir.x + 30.0 * S, ir.y, ir.w, ir.h), Align::Left, rgba(236, 236, 236, (255.0 * a) as u8));
            if t.ends_with('▸') {
                c.line(ir.r() - 18.0 * S, ir.cy() - 4.0 * S, ir.r() - 14.0 * S, ir.cy(), TEXT2.alpha(a), 1.6 * S);
                c.line(ir.r() - 14.0 * S, ir.cy(), ir.r() - 18.0 * S, ir.cy() + 4.0 * S, TEXT2.alpha(a), 1.6 * S);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args.get(1).cloned().unwrap_or_else(|| "/tmp/demo".into());
    let stills = args.get(2).map(|s| s == "stills").unwrap_or(false);
    std::fs::create_dir_all(&out_dir).ok();
    let fonts = Rc::new(load_fonts());
    let wall = wallpaper();

    let defs: Vec<(&str, &str, Glyph, Rgba, Rgba)> = vec![
        ("C:\\Windows\\explorer.exe", "Провідник", Glyph::Folder, rgba(255, 196, 64, 255), rgba(214, 120, 20, 255)),
        ("C:\\Apps\\browser.exe", "Браузер", Glyph::Globe, rgba(80, 160, 255, 255), rgba(30, 70, 200, 255)),
        ("C:\\Apps\\chat.exe", "Месенджер", Glyph::Chat, rgba(90, 210, 255, 255), rgba(20, 120, 220, 255)),
        ("C:\\Apps\\code.exe", "Редактор коду", Glyph::Code, rgba(70, 140, 255, 255), rgba(40, 40, 140, 255)),
        ("C:\\Apps\\games.exe", "Ігри", Glyph::Game, rgba(120, 120, 140, 255), rgba(30, 30, 44, 255)),
        ("C:\\Apps\\player.exe", "Плеєр", Glyph::Note, rgba(255, 80, 120, 255), rgba(180, 20, 70, 255)),
        ("C:\\Apps\\OptimaX.exe", "OptimaX", Glyph::Rocket, rgba(240, 60, 60, 255), rgba(110, 10, 20, 255)),
        ("C:\\Windows\\System32\\cmd.exe", "Термінал", Glyph::Terminal, rgba(70, 70, 80, 255), rgba(14, 14, 18, 255)),
    ];
    let items: Vec<DockItem> = defs.iter().map(|(p, n, g, a, b)| DockItem::new(p, n, mips(app_icon(&fonts, *g, *a, *b)))).collect();
    let extra_icon = mips(app_icon(&fonts, Glyph::Gear, rgba(160, 160, 170, 255), rgba(60, 60, 70, 255)));
    let mut dock = Dock::new(DockSettings::default(), items, fonts.clone());
    for i in [0, 1, 3] {
        dock.items[i].running = true;
    }

    let mut isl = Island::new(IslandSettings::default(), fonts.clone());
    let file_icons = [
        ("D:\\Projects\\StandKnife\\banner_final.png", Glyph::Image, rgba(255, 120, 90, 255), rgba(150, 30, 60, 255)),
        ("D:\\Docs\\Звіт_листопад.docx", Glyph::Doc, rgba(80, 140, 255, 255), rgba(30, 60, 170, 255)),
        ("D:\\Downloads\\OptimaX-2.4.zip", Glyph::Zip, rgba(255, 200, 80, 255), rgba(200, 120, 20, 255)),
    ];
    let file_imgs: Vec<Image> = file_icons.iter().map(|(_, g, a, b)| Rc::new(app_icon(&fonts, *g, *a, *b))).collect();
    let lyrics = Rc::new(lrc::parse(LYRICS));
    let arts = [Rc::new(album_art(0)), Rc::new(album_art(1))];

    // ── script ──
    let mut sc = Script::new();
    sc.cap("DockX 2.0 — док і Dynamic Island в одному exe, на Rust").wait(2.4);
    sc.cap("Док: пружинне збільшення іконок, як у macOS").dock(0, 0.0, 0.0, 0.9).dock(7, 1.0, -4.0, 2.4).dock(3, 0.5, 0.0, 0.9).wait(0.3);
    sc.cap("Запуск: іконка підстрибує, поки програма відкривається").dock(6, 0.5, 0.0, 0.7).click().wait(1.9).ev(Ev::SetRunning(6, true)).wait(0.7);
    sc.cap("Перетягни програму на док — для неї відкривається місце")
        .mv(560.0, 640.0, 0.6)
        .ev(Ev::Grab(Some(0)))
        .dock(2, -0.2, -30.0, 1.1)
        .ev(Ev::DragDock)
        .dock(2, 0.2, -8.0, 0.5)
        .wait(0.5)
        .ev(Ev::DropDock(vec!["C:\\Apps\\settings.exe".into()]))
        .ev(Ev::Grab(None))
        .wait(1.0);
    sc.cap("Зміна порядку: сусіди плавно переїжджають").dock(8, 0.5, 0.0, 0.7).ev(Ev::Down(Btn::Left)).dock(5, 0.5, -4.0, 0.6).dock(1, 0.5, 0.0, 1.0).ev(Ev::Up(Btn::Left)).wait(0.8);
    sc.cap("Потягни вгору й відпусти — іконка зникає з «пуфом»").dock(5, 0.5, 0.0, 0.6).ev(Ev::Down(Btn::Left)).dock(5, 0.6, -150.0, 0.8).wait(0.4).ev(Ev::Up(Btn::Left)).wait(1.0);
    sc.cap("ПКМ → «Перейменувати…» прямо в підписі").dock(2, 0.5, 0.0, 0.7).rclick().wait(0.5).menu(4, 0.6).wait(0.25).click().wait(0.3);
    sc.ev(Ev::Type("Мій браузер".into())).wait(1.2).ev(Ev::Key(Key::Enter)).wait(1.2);
    sc.cap("Dynamic Island: годинник і кільце заряду батареї").mv(1240.0, 420.0, 0.8).wait(2.2);
    sc.cap("Музика: обкладинка, візуалізатор і рядок пісні під островом").ev(Ev::Media(true)).wait(3.6);
    sc.cap("Наведи — острів розгортається: плеєр і караоке-текст в одному острові").isl("island", 0.8).wait(3.6);
    sc.cap("Кнопка тексту ховає й показує текст пісні").isl("lyrics_btn", 0.6).click().wait(1.4).isl("lyrics_btn", 0.4).click().wait(1.4);
    sc.cap("Клік по рядку пісні перемотує на цей момент").go(Tg::LyricRel(2), 0.7).wait(0.2).click().wait(2.0);
    sc.cap("Колесо миші над островом — гучність")
        .isl_at("island", 0.36, 0.55, 0.5)
        .ev(Ev::Wheel(1.0))
        .wait(0.22)
        .ev(Ev::Wheel(1.0))
        .wait(0.22)
        .ev(Ev::Wheel(1.0))
        .wait(0.8)
        .ev(Ev::Wheel(-1.0))
        .ev(Ev::Wheel(-1.0))
        .wait(0.6);
    sc.cap("Перемотування смугою прогресу і наступний трек").isl_at("seek", 0.7, 0.5, 0.6).click().wait(1.0).isl("next", 0.5).click().wait(1.6);
    sc.cap("Клік по обкладинці — перемикання між плеєрами (Spotify ⇄ YouTube у браузері)").isl("art", 0.6).wait(0.2).click().wait(2.4);
    sc.cap("Наведи на крапки — смуга сторінок з іконками").isl("dots", 0.7).wait(1.4);
    sc.cap("Сторінка «Сьогодні»: зведення дня і стрічка подій").isl("page0", 0.4).click().wait(0.3).isl("island", 0.4).wait(2.6);
    // shelf
    sc.cap("Полиця: кидай файли на острів").mv(620.0, 600.0, 0.8).wait(0.6).ev(Ev::Grab(Some(1))).isl("island", 1.0).ev(Ev::DragIsland).isl_at("island", 0.5, 0.6, 0.6).wait(0.5);
    sc.ev(Ev::DropIsland(file_icons.iter().map(|f| f.0.to_string()).collect())).ev(Ev::Grab(None)).wait(1.3);
    sc.cap("Виділи кілька (Ctrl / Shift / «Вибрати») і витягни разом")
        .isl("tile0", 0.6)
        .click()
        .isl("tile1", 0.5)
        .ev(Ev::DownMods(Btn::Left, Mods { ctrl: true, shift: false }))
        .wait(0.08)
        .ev(Ev::Up(Btn::Left))
        .wait(0.8)
        .ev(Ev::Down(Btn::Left))
        .isl_at("tile1", 0.5, 1.2, 0.3)
        .ev(Ev::Grab(Some(2)))
        .mv(520.0, 560.0, 1.0)
        .ev(Ev::Up(Btn::Left))
        .ev(Ev::Grab(None))
        .wait(1.0);
    // clipboard
    sc.cap("Історія буфера: усе скопійоване поруч із полицею")
        .ev(Ev::Copy("https://github.com/hatogg/OptimaX-by-X44TO".into()))
        .wait(0.5)
        .ev(Ev::Copy("Турнір StandKnife — субота, 18:00, реєстрація до пʼятниці".into()))
        .wait(0.4)
        .isl("island", 0.9)
        .wait(0.5)
        .isl("tab1", 0.5)
        .click()
        .wait(1.2);
    sc.cap("Клік — скопіювати знову, потягни — вставити в чат").isl("clip1", 0.5).click().wait(1.2).ev(Ev::Down(Btn::Left)).isl_at("clip0", 0.5, 1.3, 0.3).ev(Ev::GrabText(Some("Турнір StandKnife — субота…".into()))).mv(560.0, 600.0, 1.0).ev(Ev::Up(Btn::Left)).ev(Ev::GrabText(None)).wait(0.9);
    // focus
    sc.cap("Фокус-таймер: 4 раунди по 25 хвилин").isl("island", 0.8).wait(0.6).isl("dots", 0.4).wait(0.6).isl("page3", 0.3).click().wait(0.6).isl("focus_start", 0.5).click().wait(1.6);
    sc.cap("Таймер живе в згорнутому острові").mv(1260.0, 520.0, 0.8).wait(2.2).ev(Ev::FocusEndsIn(1.0)).wait(1.1);
    sc.cap("Сесію завершено — острів сповіщає, маскот радіє").wait(2.6).isl("noticeA", 0.7).click().wait(0.8);
    // water
    sc.cap("Нагадування попити води").ev(Ev::Notice(NoticeKind::Water, String::new())).wait(1.8).isl("noticeA", 0.8).wait(0.2).click().wait(1.4);
    sc.cap("Вода: обʼєм чашки, норма в мл і графік по годинах").isl("island", 0.8).wait(0.6).isl("dots", 0.4).wait(0.6).isl("page4", 0.3).click().wait(0.8).isl("drink", 0.5).click().wait(2.0);
    // todo
    sc.cap("Справи: календар тижня і нагадування на час").isl("dots", 0.5).wait(0.5).isl("page5", 0.3).click().wait(0.9);
    sc.isl_at("todo_input", 0.3, 0.5, 0.5).click().ev(Ev::Type("Стрім на TikTok @14:24".into())).wait(1.9).ev(Ev::Key(Key::Enter)).wait(0.6).ev(Ev::Key(Key::Escape)).wait(0.4);
    sc.cap("Перетягни справу, щоб змінити порядок").isl_at("todo2", 0.5, 0.5, 0.6).ev(Ev::Down(Btn::Left)).isl_at("todo0", 0.5, 0.3, 0.9).ev(Ev::Up(Btn::Left)).wait(0.8);
    sc.cap("Дзвіночок ставить нагадування, клік — виконано").isl_at("todo1", 0.5, 0.5, 0.5).wait(0.3).isl("todobell1", 0.3).click().wait(0.6).isl_at("todo1", 0.1, 0.5, 0.4).click().wait(1.0);
    sc.cap("Плануй наперед: вибери інший день").isl("day6", 0.6).click().wait(1.4).isl("day4", 0.5).click().wait(0.8);
    let t_fire = sc.t + 1.5;
    sc.cap("Час настав — нагадування вискакує з острова").mv(1250.0, 560.0, 0.8).wait(4.6).isl("noticeA", 0.7).wait(0.2).click().wait(0.8);
    // widgets
    sc.cap("Віджети: календар, світовий час, фаза місяця, відлік, батарея, вода, фокус").isl("island", 0.8).wait(0.6).isl("dots", 0.4).wait(0.6).isl("page6", 0.3).click().wait(0.3).isl("island", 0.4).wait(1.2);
    sc.cap("Клік по віджету — інший").isl("widget0", 0.5).click().wait(1.6).isl("widget1", 0.5).click().wait(1.2).click().wait(1.6);
    // settings
    sc.cap("ПКМ по острову → Налаштування…").isl_at("island", 0.5, 0.2, 0.6).rclick().wait(0.5).menu(8, 0.6).wait(0.2).click().wait(1.0);
    sc.cap("Вікно налаштувань у тому ж стилі").set("section:2", 0.7).click().wait(0.9);
    sc.cap("Вигляд: плаваюча пігулка, ефект скла").set("1:1", 0.6).click().wait(1.1).set("3:0", 0.6).click().wait(1.4);
    sc.cap("Час молитви: окрема вкладка (можна вимкнути)").set("section:8", 0.7).click().wait(0.6).set("0:0", 0.6).click().wait(1.0);
    sc.set("section:9", 0.6).click().wait(1.0).set("section:10", 0.6).click().wait(1.2).set("close", 0.7).click().wait(0.6);
    sc.cap("Вкладка «Молитва»: дуга сонця і час для твого міста").isl("island", 0.8).wait(0.6).isl("dots", 0.4).wait(0.6).isl("page7", 0.3).click().wait(0.3).isl("island", 0.4).wait(2.6);
    sc.ev(Ev::Cfg(|c| {
        c.floating = false;
        c.glass = false;
    }));
    // system indicators
    sc.cap("Системні індикатори: гучність, яскравість, Caps Lock, мова").mv(1300.0, 600.0, 0.8).wait(0.8);
    sc.ev(Ev::Hud(Hud::Volume(0.72, false))).wait(1.8).ev(Ev::Hud(Hud::Brightness(0.55))).wait(1.8).ev(Ev::Hud(Hud::Caps(true))).wait(1.8).ev(Ev::Hud(Hud::Layout("Українська".into()))).wait(2.0);
    sc.cap("Картки пристроїв: зарядка, Wi-Fi, навушники").ev(Ev::Battery(64, true)).wait(3.6).ev(Ev::Wifi(Some("X44TO_5G".into()))).wait(3.6).ev(Ev::Audio("AirBuds Pro".into())).wait(3.6);
    sc.cap("Низький заряд — попередження").ev(Ev::Battery(21, false)).wait(0.3).ev(Ev::Battery(19, false)).wait(3.0).isl("noticeA", 0.7).click().wait(0.6);
    // minimal + mascot
    sc.cap("Мінімальний виріз: у спокої — тонка лінія").ev(Ev::FocusReset).ev(Ev::Media(false)).mv(1300.0, 640.0, 0.5).ev(Ev::Cfg(|c| c.minimal = true)).wait(2.6).isl("island", 0.8).wait(1.8).mv(1300.0, 640.0, 0.7).wait(1.2).ev(Ev::Cfg(|c| c.minimal = false)).wait(0.6);
    sc.cap("Коли за ПК нікого — зʼявляється маскот, згодом засинає").ev(Ev::IdleJump(60.0)).wait(2.6).ev(Ev::IdleJump(300.0)).wait(2.6);
    sc.cap("Наведи курсор — маскот вітається").isl("island", 1.0).wait(2.2).mv(1300.0, 640.0, 0.8).wait(0.8);
    sc.cap("Повноекранні ігри й відео: док та острів ховаються").ev(Ev::Fullscreen(true)).wait(2.0).ev(Ev::Fullscreen(false)).wait(1.6);
    sc.cap("Автоприховування дока: виїжджає від нижнього краю")
        .ev(Ev::DockCfg(true))
        .mv(960.0, 760.0, 0.6)
        .wait(1.2)
        .mv(960.0, 1079.0, 0.7)
        .wait(1.4)
        .mv(960.0, 700.0, 0.7)
        .wait(1.3)
        .ev(Ev::DockCfg(false))
        .wait(0.8);
    sc.cap("DockX 2.0 by X44TO").mv(960.0, 600.0, 0.6).wait(3.0);
    let total = sc.t;

    // local clock: the reminder above fires exactly at t_fire (14:24:00)
    let tz = 120;
    let day0 = dockx::ui::time::days_from_civil(2026, 11, 13);
    let base_local = 14.0 * 3600.0 + 24.0 * 60.0 - t_fire;
    let unix_at = |t: f64| day0 as f64 * 86400.0 + base_local - tz as f64 * 60.0 + t;
    isl.set_today_for_demo(unix_at(0.0), tz);
    isl.volume = 0.62;
    isl.set_battery(Some(Battery { percent: 64, charging: false, plugged: false }));
    isl.todo_add("Залити реліз OptimaX 2.5", 0, None);
    isl.todo_add("Банер для турніру StandKnife", 0, Some(19 * 60));
    isl.todo_add("Купити термопасту", 0, None);
    isl.todos[2].done = false;
    isl.todo_add("Перевірити GoldHEN на PS4", 2, None);
    isl.drinks = vec![(8.6, 250), (10.1, 250), (11.8, 330), (13.3, 250)];
    let _ = isl.data_text();

    let track = |n: u32, now: f64, pos: f64, playing: bool| -> MediaInfo {
        let (title, art, dur, app, pidx) = match n {
            0 => ("Нічне місто", arts[0].clone(), 214.0, "Spotify", 0),
            1 => ("Хвилі", arts[1].clone(), 188.0, "Spotify", 0),
            _ => ("X44TO - Неон (Live)", arts[1].clone(), 241.0, "Chrome", 1),
        };
        MediaInfo {
            has: true,
            title: title.into(),
            artist: if n == 2 { String::new() } else { "X44TO".into() },
            album: "Неон".into(),
            app: app.into(),
            art: Some(art),
            playing,
            position: pos,
            duration: dur,
            sampled_at: now,
            can_prev: true,
            can_next: true,
            players: vec!["Spotify".into(), "Chrome".into()],
            player_idx: pidx,
        }
    };
    let mut cur_track = 0u32;

    let mut ev_i = 0;
    let mut caption = String::new();
    let mut caption_t = 0.0;
    let mut prev_caption = String::new();
    let mut pressed = false;
    let mut grab: Option<Pixmap> = None;
    let mut grab_text: Option<String> = None;
    let mut fs = false;
    let mut over_island_drag = false;
    let mut idle_start = 0.0f64;
    let mut last_cur = (0.0f32, 0.0f32);
    let mut menu: Option<(FakeMenu, Option<usize>, bool)> = None; // (menu, dock idx, island)
    let mut settings: Option<(dockx::ui::settings::SettingsPanel, (f32, f32))> = None;
    let mut typing: Vec<(f64, char)> = Vec::new();
    let mut dragout: Option<(f64, Vec<String>, bool)> = None;
    let frames = (total * FPS).ceil() as usize;
    let screen = rect(0.0, 0.0, W as f32, H as f32);
    let mut out = std::io::BufWriter::with_capacity(1 << 22, std::io::stdout().lock());
    let mut frame = Canvas::new(W, H);
    let still_times: Vec<f64> = if stills { args.get(3).map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default() } else { vec![] };
    let mut still_i = 0;
    eprintln!("total {:.1}s, {} frames, fire at {:.1}", total, frames, t_fire);

    for fi in 0..frames {
        let now = fi as f64 / FPS;
        // resolve cursor
        let (cx, cy) = {
            let n = sc.path.len();
            let mut j = n - 1;
            for k in 1..n {
                if now <= sc.path[k].0 {
                    j = k;
                    break;
                }
            }
            for k in 1..=j {
                if sc.resolved[k].is_none() {
                    let prev = sc.resolved[k - 1].unwrap();
                    let p = match sc.path[k].1 {
                        Tg::Same => prev,
                        Tg::At(x, y) => (x, y),
                        Tg::Isl(name, fx, fy) => isl.zone_rect(name).map(|r| (r.x + r.w * fx, r.y + r.h * fy)).unwrap_or_else(|| {
                            eprintln!("missing zone {name} at {now:.2}");
                            prev
                        }),
                        Tg::Dock(i, fx, dy) => dock.item_rect(i).map(|r| (r.x + r.w * fx, r.cy() + dy)).unwrap_or(prev),
                        Tg::Set(name, fx, fy) => settings
                            .as_ref()
                            .and_then(|(p, o)| p.zone_of(name).map(|r| (o.0 + r.x + r.w * fx, o.1 + r.y + r.h * fy)))
                            .unwrap_or_else(|| {
                                eprintln!("missing settings zone {name} at {now:.2}");
                                prev
                            }),
                        Tg::LyricRel(off) => isl
                            .zone_rect(&format!("line{}", isl.current_lyric() + off))
                            .map(|r| (r.x + r.w * 0.5, r.cy()))
                            .unwrap_or_else(|| {
                                eprintln!("missing lyric line at {now:.2}");
                                prev
                            }),
                        Tg::Menu(i) => menu.as_ref().map(|(m, _, _)| {
                            let r = m.rect_of(&fonts, i);
                            (r.x + r.w * 0.4, r.cy())
                        }).unwrap_or(prev),
                    };
                    sc.resolved[k] = Some(p);
                }
            }
            if j == 0 {
                sc.resolved[0].unwrap()
            } else {
                let (a, b) = (sc.resolved[j - 1].unwrap(), sc.resolved[j].unwrap());
                let (t0, t1) = (sc.path[j - 1].0, sc.path[j].0);
                let k = if t1 > t0 { ((now - t0) / (t1 - t0)).clamp(0.0, 1.0) as f32 } else { 1.0 };
                let e = dockx::ui::anim::ease_in_out_cubic(k);
                (a.0 + (b.0 - a.0) * e, a.1 + (b.1 - a.1) * e)
            }
        };
        if (cx - last_cur.0).abs() + (cy - last_cur.1).abs() > 0.5 {
            idle_start = now;
        }
        last_cur = (cx, cy);

        // events
        while ev_i < sc.events.len() && sc.events[ev_i].0 <= now {
            let e = sc.events[ev_i].1.clone();
            ev_i += 1;
            match e {
                Ev::Down(b) | Ev::DownMods(b, _) => {
                    pressed = true;
                    idle_start = now;
                    let mods = if let Ev::DownMods(_, m) = e { m } else { Mods::default() };
                    if menu.is_some() {
                        // menus act on button-up
                    } else if let Some((p, o)) = &mut settings {
                        let r = rect(o.0, o.1, p.size().0 as f32, p.size().1 as f32);
                        if r.contains(cx, cy) {
                            p.mouse_down(b, cx - o.0, cy - o.1);
                        }
                    } else if isl.contains_screen(cx, cy) {
                        isl.mouse_down(b, cx, cy, mods, false);
                    } else {
                        dock.mouse_down(b, cx, cy);
                    }
                }
                Ev::Up(b) => {
                    pressed = false;
                    if let Some((m, idx, island_menu)) = menu.take() {
                        let pick = m.hit(&fonts, cx, cy).map(|i| m.items[i].1);
                        dock.menu_closed(now);
                        match pick {
                            Some(5) => {
                                if let Some(i) = idx {
                                    dock.begin_rename(i);
                                }
                            }
                            Some(50) => {
                                let mut app = dockx::ui::settings::AppSettings::default();
                                app.island = isl.cfg.clone();
                                app.dock = dock.cfg.clone();
                                let p = dockx::ui::settings::SettingsPanel::new(app, fonts.clone(), S);
                                let (w, h) = p.size();
                                settings = Some((p, ((W as f32 - w as f32) / 2.0, (H as f32 - h as f32) / 2.0 + 60.0)));
                            }
                            _ => {}
                        }
                        let _ = island_menu;
                    } else if let Some((p, o)) = &mut settings {
                        p.mouse_up(b, cx - o.0, cy - o.1);
                    } else {
                        isl.mouse_up(b, cx, cy);
                        dock.mouse_up(b, cx, cy);
                    }
                }
                Ev::Wheel(d) => isl.wheel(d, cx, cy),
                Ev::DragDock => {
                    dock.drag_over(cx, cy, true, true);
                }
                Ev::DragIsland => {
                    over_island_drag = true;
                    isl.drag_over(cx, cy);
                }
                Ev::DropDock(files) => dock.drop_files(files),
                Ev::DropIsland(files) => {
                    over_island_drag = false;
                    isl.drop_files(files);
                }
                Ev::Type(s) => {
                    for (k, ch) in s.chars().enumerate() {
                        typing.push((now + k as f64 * 0.07, ch));
                    }
                }
                Ev::Key(k) => {
                    if dock.editing.is_some() {
                        dock.key(k)
                    } else {
                        isl.key(k)
                    }
                }
                Ev::Caption(s) => {
                    prev_caption = std::mem::replace(&mut caption, s);
                    caption_t = now;
                }
                Ev::Notice(k, t) => isl.debug_notice(k, &t),
                Ev::FocusEndsIn(sec) => isl.debug_focus_ends_in(sec),
                Ev::Media(p) => {
                    cur_track = 0;
                    if p {
                        isl.set_media(track(0, now, 31.0, true));
                        let key = isl.media.key();
                        isl.set_lyrics(&key, Lyrics::Synced(lyrics.clone()));
                    } else {
                        let mut m = isl.media.clone();
                        m.position = m.pos_at(now);
                        m.sampled_at = now - 1000.0;
                        m.playing = false;
                        m.has = false;
                        isl.set_media(m);
                    }
                }
                Ev::Copy(t) => isl.clip_push(&t),
                Ev::Hud(h) => isl.show_hud(h),
                Ev::Battery(p, plugged) => isl.set_battery(Some(Battery { percent: p, charging: plugged, plugged })),
                Ev::Wifi(s) => isl.wifi_changed(s),
                Ev::Audio(n) => isl.audio_device(&n),
                Ev::IdleJump(sec) => idle_start = now - sec,
                Ev::Cfg(f) => {
                    f(&mut isl.cfg);
                    isl.kick();
                }
                Ev::DockCfg(v) => dock.cfg.auto_hide = v,
                Ev::Grab(g) => grab = g.map(|i| if i == 0 { (*extra_icon[0]).clone() } else { (*file_imgs[(i - 1).min(2)]).clone() }),
                Ev::GrabText(t) => grab_text = t,
                Ev::SetRunning(i, v) => {
                    if let Some(it) = dock.items.get_mut(i) {
                        it.running = v;
                    }
                }
                Ev::Fullscreen(v) => fs = v,
                Ev::FocusReset => isl.focus_reset(),
            }
        }
        while let Some(&(tt, ch)) = typing.first() {
            if tt > now {
                break;
            }
            if dock.editing.is_some() {
                dock.key(Key::Char(ch));
            } else {
                isl.key(Key::Char(ch));
            }
            typing.remove(0);
            idle_start = now;
        }
        if menu.is_none() {
            if let Some((p, o)) = &mut settings {
                p.mouse_move(cx - o.0, cy - o.1);
            } else {
                dock.mouse_move(cx, cy);
                isl.mouse_move(cx, cy);
            }
        }
        if over_island_drag {
            isl.drag_over(cx, cy);
        }
        // host side
        for a in std::mem::take(&mut dock.actions) {
            match a {
                DockAction::AddPaths { paths, index, into_gap } => {
                    let its = paths.iter().map(|p| DockItem::new(p, "Налаштування", extra_icon.clone())).collect();
                    dock.insert_items(its, index, into_gap);
                }
                DockAction::Activate { idx, new_instance, .. } => dock.start_bounce(idx, new_instance),
                DockAction::Menu { idx, x, y } => {
                    let items: Vec<(String, u32)> = vec![
                        ("Перейти до вікна".into(), 1),
                        ("Нове вікно".into(), 2),
                        ("Запустити від імені адміністратора".into(), 3),
                        ("Показати у Провіднику".into(), 4),
                        ("Перейменувати…".into(), 5),
                        (String::new(), 0),
                        ("Прибрати з Dock".into(), 6),
                        (String::new(), 0),
                        ("Налаштування Dock▸".into(), 7),
                    ];
                    let mut m = FakeMenu { x, y, items, t: now };
                    m.y -= m.total_h();
                    menu = Some((m, idx, false));
                }
                _ => {}
            }
        }
        let mut more = Vec::new();
        for a in std::mem::take(&mut isl.actions) {
            match a {
                IslandAction::ShelfAdd(paths) => {
                    let items = paths
                        .iter()
                        .map(|p| {
                            let i = file_icons.iter().position(|f| f.0 == p).unwrap_or(0);
                            ShelfItem::new(p, Some(file_imgs[i].clone()))
                        })
                        .collect();
                    more.push(items);
                }
                IslandAction::DragOut(paths) => dragout = Some((now + 1.25, paths, true)),
                IslandAction::DragText(_) => dragout = Some((now + 1.25, vec![], false)),
                IslandAction::MediaNext => {
                    cur_track = 1;
                    isl.set_media(track(1, now, 0.0, true));
                    let key = isl.media.key();
                    isl.set_lyrics(&key, Lyrics::NotFound);
                }
                IslandAction::NextPlayer => {
                    cur_track = if cur_track == 2 { 1 } else { 2 };
                    isl.set_media(track(cur_track, now, 74.0, true));
                    let key = isl.media.key();
                    isl.set_lyrics(&key, Lyrics::NotFound);
                }
                IslandAction::MediaSeek(t) => {
                    let m = MediaInfo { position: t, sampled_at: now, ..isl.media.clone() };
                    isl.set_media(m);
                }
                IslandAction::Menu { x, y } => {
                    let items: Vec<(String, u32)> = vec![
                        ("Відкрити сторінку▸".into(), 0),
                        (String::new(), 0),
                        ("Плаваюча пігулка".into(), 60),
                        ("Мінімальний виріз".into(), 61),
                        ("Ефект скла".into(), 62),
                        ("Маскот".into(), 63),
                        ("Рядок пісні під островом".into(), 65),
                        (String::new(), 0),
                        ("Налаштування…".into(), 50),
                        ("Сховати острів".into(), 64),
                        ("Вийти з DockX".into(), 99),
                    ];
                    menu = Some((FakeMenu { x, y, items, t: now }, None, true));
                }
                _ => {}
            }
        }
        for items in more {
            isl.add_shelf_items(items);
        }
        isl.actions.retain(|a| !matches!(a, IslandAction::SaveData));
        if let Some((t, p, files)) = dragout.clone() {
            if now >= t {
                if files {
                    isl.drag_out_finished(&p, true);
                } else {
                    isl.drag_text_finished();
                }
                dragout = None;
            }
        }
        if let Some((p, _)) = &mut settings {
            p.step(now);
            let acts = std::mem::take(&mut p.actions);
            for a in acts {
                match a {
                    dockx::ui::settings::SettingsAction::Changed => {
                        isl.cfg = p.app.island.clone();
                        dock.cfg = p.app.dock.clone();
                        isl.kick();
                    }
                    dockx::ui::settings::SettingsAction::Close => {
                        settings_close(&mut settings_closing());
                    }
                    _ => {}
                }
            }
        }
        if CLOSE.with(|c| c.replace(false)) {
            settings = None;
        }

        let denv = DockEnv { now, cursor: (cx, cy), work: screen, screen, s: S, fullscreen: fs, menu_open: menu.is_some() };
        dock.step(&denv);
        let ienv = IslandEnv { now, cursor: (cx, cy), screen, s: S, fullscreen: fs, menu_open: menu.is_some(), unix: unix_at(now), tz_min: tz, idle_secs: now - idle_start };
        isl.step(&ienv);

        // compose
        frame.pm.data_mut().copy_from_slice(wall.data());
        if fs {
            let full = rrect(rect(0.0, 0.0, W as f32, H as f32), 0.0).unwrap();
            frame.gradient(&full, (0.0, 0.0), (0.0, H as f32), &[(0.0, rgba(30, 60, 90, 255)), (1.0, rgba(10, 20, 30, 255))]);
            frame.text_center(&fonts.bold, "ГРА НА ВЕСЬ ЕКРАН", 64.0, 960.0, 560.0, rgba(255, 255, 255, 60));
        }
        if dock.win_visible {
            frame.blit(&dock.canvas.pm, dock.win_x as f32, dock.win_y as f32, dock.win_alpha as f32 / 255.0);
        }
        if let Some((p, o)) = &settings {
            frame.blit(&p.canvas.pm, o.0, o.1, 1.0);
        }
        if isl.win_visible {
            frame.blit(&isl.canvas.pm, isl.win_x as f32, isl.win_y as f32, 1.0);
        }
        if let Some((m, _, _)) = &menu {
            m.draw(&mut frame, &fonts, (cx, cy), now);
        }
        // caption chip
        let k = ((now - caption_t) / 0.35).min(1.0) as f32;
        let draw_cap = |frame: &mut Canvas, text: &str, a: f32, dy: f32| {
            if text.is_empty() || a <= 0.01 {
                return;
            }
            let size = 22.0;
            let tw = fonts.semibold.measure(text, size);
            let r = rect(60.0, 120.0 + dy, tw + 44.0, 50.0);
            frame.fill_rrect(r, 25.0, rgba(14, 14, 16, (225.0 * a) as u8));
            frame.stroke_rrect(r, 25.0, rgba(255, 255, 255, (50.0 * a) as u8), 1.2);
            frame.fill_circle(r.x + 22.0, r.cy(), 5.0, ACCENT.alpha(a));
            frame.text_in(&fonts.semibold, text, size, rect(r.x + 36.0, r.y, tw, r.h), Align::Left, rgba(255, 255, 255, (245.0 * a) as u8));
        };
        let clean = std::env::var_os("DEMO_CLEAN").is_some();
        if clean {
        } else if k < 0.5 {
            let e = 1.0 - k * 2.0;
            draw_cap(&mut frame, &prev_caption, e, -10.0 * (1.0 - e));
        } else {
            let e = k * 2.0 - 1.0;
            draw_cap(&mut frame, &caption, e, 10.0 * (1.0 - e));
        }
        if let Some(t) = &grab_text {
            let tw = fonts.regular.measure(t, 15.0);
            let r = rect(cx + 14.0, cy + 18.0, tw + 24.0, 32.0);
            frame.fill_rrect(r, 8.0, rgba(250, 250, 250, 230));
            frame.text_in(&fonts.regular, t, 15.0, r, Align::Center, rgba(20, 20, 24, 255));
        }
        if !clean {
            draw_cursor(&mut frame, cx, cy, pressed, grab.as_ref());
        }

        if stills {
            if still_i < still_times.len() && now >= still_times[still_i] {
                std::fs::write(format!("{out_dir}/still_{:02}.rgba", still_i), frame.pm.data()).ok();
                still_i += 1;
            }
        } else {
            out.write_all(frame.pm.data()).unwrap();
        }
    }
    out.flush().ok();
    eprintln!("done");
}

thread_local! {
    static CLOSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
struct Closing;
fn settings_closing() -> Closing {
    Closing
}
fn settings_close(_: &mut Closing) {
    CLOSE.with(|c| c.set(true));
}
