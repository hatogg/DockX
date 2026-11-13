//! Software 2D canvas shared by the dock, the island and the demo video.
//! Everything is premultiplied RGBA (tiny-skia); the Windows layer swaps to BGRA when presenting.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder,
    Pattern, Pixmap, PixmapPaint, Point, RadialGradient, Shader, SpreadMode, Stroke, LineCap, LineJoin, Transform,
};

pub type Image = Rc<Pixmap>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba { r, g, b, a }
}

impl Rgba {
    pub fn alpha(self, k: f32) -> Rgba {
        let a = (self.a as f32 * k.clamp(0.0, 1.0)).round() as u8;
        Rgba { a, ..self }
    }
    pub fn with_a(self, a: u8) -> Rgba {
        Rgba { a, ..self }
    }
    pub fn mix(self, o: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba { r: l(self.r, o.r), g: l(self.g, o.g), b: l(self.b, o.b), a: l(self.a, o.a) }
    }
    pub fn sk(self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub const fn rect(x: f32, y: f32, w: f32, h: f32) -> RectF {
    RectF { x, y, w, h }
}

impl RectF {
    pub fn r(&self) -> f32 {
        self.x + self.w
    }
    pub fn b(&self) -> f32 {
        self.y + self.h
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.r() && py >= self.y && py < self.b()
    }
    pub fn inflate(&self, dx: f32, dy: f32) -> RectF {
        rect(self.x - dx, self.y - dy, self.w + 2.0 * dx, self.h + 2.0 * dy)
    }
    pub fn offset(&self, dx: f32, dy: f32) -> RectF {
        rect(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
}

const K: f32 = 0.552_284_8;

/// Rounded rectangle with individual corner radii (tl, tr, br, bl).
pub fn rrect4(r: RectF, tl: f32, tr: f32, br: f32, bl: f32) -> Option<Path> {
    if r.w <= 0.0 || r.h <= 0.0 {
        return None;
    }
    let m = r.w.min(r.h) / 2.0;
    let (tl, tr, br, bl) = (tl.clamp(0.0, m), tr.clamp(0.0, m), br.clamp(0.0, m), bl.clamp(0.0, m));
    let (x0, y0, x1, y1) = (r.x, r.y, r.r(), r.b());
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + tl, y0);
    pb.line_to(x1 - tr, y0);
    if tr > 0.0 {
        pb.cubic_to(x1 - tr + tr * K, y0, x1, y0 + tr - tr * K, x1, y0 + tr);
    }
    pb.line_to(x1, y1 - br);
    if br > 0.0 {
        pb.cubic_to(x1, y1 - br + br * K, x1 - br + br * K, y1, x1 - br, y1);
    }
    pb.line_to(x0 + bl, y1);
    if bl > 0.0 {
        pb.cubic_to(x0 + bl - bl * K, y1, x0, y1 - bl + bl * K, x0, y1 - bl);
    }
    pb.line_to(x0, y0 + tl);
    if tl > 0.0 {
        pb.cubic_to(x0, y0 + tl - tl * K, x0 + tl - tl * K, y0, x0 + tl, y0);
    }
    pb.close();
    pb.finish()
}

pub fn rrect(r: RectF, rad: f32) -> Option<Path> {
    rrect4(r, rad, rad, rad, rad)
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    if r <= 0.0 {
        return None;
    }
    PathBuilder::from_circle(cx, cy, r)
}

/// Notch shape: flat top glued to the screen edge with small concave "ears", rounded bottom.
pub fn notch_path(r: RectF, ear: f32, bottom: f32) -> Option<Path> {
    if r.w <= 2.0 * ear + 2.0 || r.h <= 1.0 {
        return rrect4(r, 0.0, 0.0, bottom, bottom);
    }
    let bottom = bottom.min(r.h - 0.5).min((r.w - 2.0 * ear) / 2.0).max(0.0);
    let ear = ear.min(r.h * 0.6);
    let (x0, y0, x1, y1) = (r.x, r.y, r.r(), r.b());
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y0);
    // right ear: concave curve from the top edge into the right side
    pb.cubic_to(x1 - ear * K, y0, x1 - ear, y0 + ear - ear * K, x1 - ear, y0 + ear);
    pb.line_to(x1 - ear, y1 - bottom);
    pb.cubic_to(x1 - ear, y1 - bottom + bottom * K, x1 - ear - bottom + bottom * K, y1, x1 - ear - bottom, y1);
    pb.line_to(x0 + ear + bottom, y1);
    pb.cubic_to(x0 + ear + bottom - bottom * K, y1, x0 + ear, y1 - bottom + bottom * K, x0 + ear, y1 - bottom);
    pb.line_to(x0 + ear, y0 + ear);
    pb.cubic_to(x0 + ear, y0 + ear - ear * K, x0 + ear * K, y0, x0, y0);
    pb.close();
    pb.finish()
}

pub struct Canvas {
    pub pm: Pixmap,
    clip: Option<Mask>,
    clip_rect: Option<RectF>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Canvas {
        Canvas { pm: Pixmap::new(w.max(1), h.max(1)).unwrap(), clip: None, clip_rect: None }
    }
    pub fn w(&self) -> u32 {
        self.pm.width()
    }
    pub fn h(&self) -> u32 {
        self.pm.height()
    }

    /// Makes the canvas exactly w×h and transparent.
    pub fn reset(&mut self, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if self.pm.width() != w || self.pm.height() != h {
            self.pm = Pixmap::new(w, h).unwrap();
        } else {
            self.pm.fill(Color::TRANSPARENT);
        }
        self.clip = None;
        self.clip_rect = None;
    }

    pub fn clear(&mut self) {
        self.pm.fill(Color::TRANSPARENT);
    }

    // ── clipping (anti-aliased path clip) ──

    pub fn set_clip(&mut self, path: Option<&Path>, bounds: Option<RectF>) {
        self.clip = None;
        self.clip_rect = bounds;
        if let Some(p) = path {
            if let Some(mut m) = Mask::new(self.pm.width(), self.pm.height()) {
                m.fill_path(p, FillRule::Winding, true, Transform::identity());
                self.clip = Some(m);
            }
        }
    }
    pub fn clip_rrect(&mut self, r: RectF, rad: f32) {
        let p = rrect(r, rad);
        self.set_clip(p.as_ref(), Some(r));
    }
    /// Rectangular clip for text and images only, keeping the current shape mask.
    pub fn set_clip_rect(&mut self, r: Option<RectF>) {
        self.clip_rect = r;
    }
    pub fn no_clip(&mut self) {
        self.clip = None;
        self.clip_rect = None;
    }

    // ── primitives ──

    pub fn fill_path(&mut self, path: &Path, c: Rgba) {
        if c.a == 0 {
            return;
        }
        let mut p = Paint::default();
        p.set_color(c.sk());
        p.anti_alias = true;
        self.pm.fill_path(path, &p, FillRule::Winding, Transform::identity(), self.clip.as_ref());
    }

    pub fn fill_path_shader(&mut self, path: &Path, shader: Shader) {
        let mut p = Paint::default();
        p.shader = shader;
        p.anti_alias = true;
        self.pm.fill_path(path, &p, FillRule::Winding, Transform::identity(), self.clip.as_ref());
    }

    pub fn fill_path_blend(&mut self, path: &Path, c: Rgba, mode: BlendMode) {
        let mut p = Paint::default();
        p.set_color(c.sk());
        p.anti_alias = true;
        p.blend_mode = mode;
        self.pm.fill_path(path, &p, FillRule::Winding, Transform::identity(), self.clip.as_ref());
    }

    pub fn stroke_path(&mut self, path: &Path, c: Rgba, width: f32) {
        self.stroke_path_cap(path, c, width, LineCap::Butt);
    }

    pub fn stroke_path_cap(&mut self, path: &Path, c: Rgba, width: f32, cap: LineCap) {
        if c.a == 0 || width <= 0.0 {
            return;
        }
        let mut p = Paint::default();
        p.set_color(c.sk());
        p.anti_alias = true;
        let st = Stroke { width, line_cap: cap, line_join: LineJoin::Round, ..Stroke::default() };
        self.pm.stroke_path(path, &p, &st, Transform::identity(), self.clip.as_ref());
    }

    pub fn fill_rrect(&mut self, r: RectF, rad: f32, c: Rgba) {
        if let Some(p) = rrect(r, rad) {
            self.fill_path(&p, c);
        }
    }

    pub fn stroke_rrect(&mut self, r: RectF, rad: f32, c: Rgba, w: f32) {
        if let Some(p) = rrect(r, rad) {
            self.stroke_path(&p, c, w);
        }
    }

    pub fn stroke_rrect_dash(&mut self, r: RectF, rad: f32, c: Rgba, w: f32, dash: f32, phase: f32) {
        let Some(path) = rrect(r, rad) else { return };
        let mut p = Paint::default();
        p.set_color(c.sk());
        p.anti_alias = true;
        let st = Stroke {
            width: w,
            line_cap: LineCap::Round,
            dash: tiny_skia::StrokeDash::new(vec![dash, dash * 0.8], phase),
            ..Stroke::default()
        };
        self.pm.stroke_path(&path, &p, &st, Transform::identity(), self.clip.as_ref());
    }

    pub fn fill_rect(&mut self, r: RectF, c: Rgba) {
        if let Some(rr) = tiny_skia::Rect::from_xywh(r.x, r.y, r.w, r.h) {
            let mut p = Paint::default();
            p.set_color(c.sk());
            self.pm.fill_rect(rr, &p, Transform::identity(), self.clip.as_ref());
        }
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if let Some(p) = circle(cx, cy, r) {
            self.fill_path(&p, c);
        }
    }

    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba, w: f32) {
        if let Some(p) = circle(cx, cy, r) {
            self.stroke_path(&p, c, w);
        }
    }

    /// Arc from angle a0 sweeping `sweep` radians (0 = 12 o'clock, clockwise), round caps.
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, a0: f32, sweep: f32, c: Rgba, w: f32) {
        if sweep.abs() < 0.001 || r <= 0.0 {
            return;
        }
        let n = ((sweep.abs() / (std::f32::consts::PI * 2.0)) * 64.0).ceil().max(2.0) as usize;
        let mut pb = PathBuilder::new();
        for i in 0..=n {
            let a = a0 + sweep * i as f32 / n as f32;
            let (x, y) = (cx + r * a.sin(), cy - r * a.cos());
            if i == 0 {
                pb.move_to(x, y)
            } else {
                pb.line_to(x, y)
            }
        }
        if let Some(p) = pb.finish() {
            self.stroke_path_cap(&p, c, w, LineCap::Round);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgba, w: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(p) = pb.finish() {
            self.stroke_path_cap(&p, c, w, LineCap::Round);
        }
    }

    pub fn vgradient(&mut self, path: &Path, y0: f32, y1: f32, top: Rgba, bottom: Rgba) {
        if let Some(sh) = LinearGradient::new(
            Point::from_xy(0.0, y0),
            Point::from_xy(0.0, y1),
            vec![GradientStop::new(0.0, top.sk()), GradientStop::new(1.0, bottom.sk())],
            SpreadMode::Pad,
            Transform::identity(),
        ) {
            self.fill_path_shader(path, sh);
        } else {
            self.fill_path(path, top);
        }
    }

    pub fn gradient(&mut self, path: &Path, p0: (f32, f32), p1: (f32, f32), stops: &[(f32, Rgba)]) {
        let st = stops.iter().map(|(t, c)| GradientStop::new(*t, c.sk())).collect();
        if let Some(sh) =
            LinearGradient::new(Point::from_xy(p0.0, p0.1), Point::from_xy(p1.0, p1.1), st, SpreadMode::Pad, Transform::identity())
        {
            self.fill_path_shader(path, sh);
        }
    }

    /// Soft round blob: `inner` at the centre fading to transparent `outer`.
    pub fn radial(&mut self, cx: f32, cy: f32, r: f32, inner: Rgba, outer: Rgba) {
        if r < 0.5 {
            return;
        }
        if let (Some(p), Some(sh)) = (
            circle(cx, cy, r),
            RadialGradient::new(
                Point::from_xy(cx, cy),
                Point::from_xy(cx, cy),
                r,
                vec![GradientStop::new(0.0, inner.sk()), GradientStop::new(1.0, outer.sk())],
                SpreadMode::Pad,
                Transform::identity(),
            ),
        ) {
            self.fill_path_shader(&p, sh);
        }
    }

    // ── images ──

    pub fn image(&mut self, img: &Pixmap, dst: RectF, opacity: f32) {
        self.image_q(img, dst, opacity, FilterQuality::Bicubic);
    }

    pub fn image_q(&mut self, img: &Pixmap, dst: RectF, opacity: f32, q: FilterQuality) {
        if dst.w < 0.5 || dst.h < 0.5 || opacity <= 0.003 {
            return;
        }
        let sx = dst.w / img.width() as f32;
        let sy = dst.h / img.height() as f32;
        let paint = PixmapPaint { opacity: opacity.min(1.0), blend_mode: BlendMode::SourceOver, quality: q };
        let t = Transform::from_row(sx, 0.0, 0.0, sy, dst.x, dst.y);
        self.pm.draw_pixmap(0, 0, img.as_ref(), &paint, t, self.clip.as_ref());
    }

    /// Image clipped to a rounded rectangle (album art, file thumbnails).
    pub fn image_rounded(&mut self, img: &Pixmap, dst: RectF, rad: f32, opacity: f32) {
        if dst.w < 0.5 || dst.h < 0.5 {
            return;
        }
        let Some(path) = rrect(dst, rad) else { return };
        let sx = dst.w / img.width() as f32;
        let sy = dst.h / img.height() as f32;
        let pat = Pattern::new(
            img.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Bicubic,
            opacity.min(1.0),
            Transform::from_row(sx, 0.0, 0.0, sy, dst.x, dst.y),
        );
        self.fill_path_shader(&path, pat);
    }

    /// Image darkened by `dim` (1 = unchanged) and faded by `opacity` — the pressed dock icon.
    pub fn image_dimmed(&mut self, img: &Pixmap, dst: RectF, opacity: f32, dim: f32) {
        if dim >= 0.999 {
            return self.image(img, dst, opacity);
        }
        let (w, h) = (dst.w.ceil() as u32 + 2, dst.h.ceil() as u32 + 2);
        let Some(mut tmp) = Pixmap::new(w, h) else { return };
        let t = Transform::from_row(dst.w / img.width() as f32, 0.0, 0.0, dst.h / img.height() as f32, 1.0, 1.0);
        tmp.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t, None);
        let mut p = Paint::default();
        p.set_color(Color::from_rgba(0.0, 0.0, 0.0, (1.0 - dim).clamp(0.0, 1.0)).unwrap());
        p.blend_mode = BlendMode::SourceAtop;
        tmp.fill_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32).unwrap(), &p, Transform::identity(), None);
        let paint = PixmapPaint { opacity: opacity.min(1.0), ..Default::default() };
        self.pm.draw_pixmap(0, 0, tmp.as_ref(), &paint, Transform::from_translate(dst.x - 1.0, dst.y - 1.0), self.clip.as_ref());
    }

    /// Copies another canvas at (x, y) with opacity (used to composite the demo frames).
    pub fn blit(&mut self, src: &Pixmap, x: f32, y: f32, opacity: f32) {
        let paint = PixmapPaint { opacity, quality: FilterQuality::Nearest, ..Default::default() };
        self.pm.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_translate(x.round(), y.round()), None);
    }

    // ── text ──

    /// Draws `s` with its baseline at `y`; returns the advance width.
    pub fn text(&mut self, f: &Font, s: &str, size: f32, x: f32, y: f32, c: Rgba) -> f32 {
        if c.a == 0 || s.is_empty() {
            return f.measure(s, size);
        }
        let (w, h) = (self.pm.width() as i32, self.pm.height() as i32);
        let clip_r = self.clip_rect;
        let mask = self.clip.as_ref().map(|m| m.data().to_vec());
        let data = self.pm.data_mut();
        let ca = c.a as u32;
        let mut pen = x;
        let mut prev: Option<char> = None;
        for ch in s.chars() {
            if let Some(p) = prev {
                pen += f.kern(p, ch, size);
            }
            prev = Some(ch);
            let g = f.glyph(ch, size);
            let gx = (pen + g.xmin as f32).round() as i32;
            let gy = (y - (g.height as f32 + g.ymin as f32)).round() as i32;
            pen += g.advance;
            if g.width == 0 {
                continue;
            }
            for row in 0..g.height as i32 {
                let py = gy + row;
                if py < 0 || py >= h {
                    continue;
                }
                if let Some(cr) = clip_r {
                    if (py as f32) < cr.y || (py as f32) >= cr.b() {
                        continue;
                    }
                }
                for col in 0..g.width as i32 {
                    let px = gx + col;
                    if px < 0 || px >= w {
                        continue;
                    }
                    let mut cov = g.bitmap[(row * g.width as i32 + col) as usize] as u32;
                    if cov == 0 {
                        continue;
                    }
                    if let Some(cr) = clip_r {
                        if (px as f32) < cr.x || (px as f32) >= cr.r() {
                            continue;
                        }
                    }
                    let idx = (py * w + px) as usize;
                    if let Some(m) = &mask {
                        cov = cov * m[idx] as u32 / 255;
                        if cov == 0 {
                            continue;
                        }
                    }
                    let a = cov * ca / 255; // 0..255
                    let o = idx * 4;
                    let inv = 255 - a;
                    data[o] = ((c.r as u32 * a + data[o] as u32 * inv) / 255) as u8;
                    data[o + 1] = ((c.g as u32 * a + data[o + 1] as u32 * inv) / 255) as u8;
                    data[o + 2] = ((c.b as u32 * a + data[o + 2] as u32 * inv) / 255) as u8;
                    data[o + 3] = (a + data[o + 3] as u32 * inv / 255) as u8;
                }
            }
        }
        pen - x
    }

    pub fn text_center(&mut self, f: &Font, s: &str, size: f32, cx: f32, y: f32, c: Rgba) {
        let w = f.measure(s, size);
        self.text(f, s, size, cx - w / 2.0, y, c);
    }

    /// Text vertically centred in a box (baseline from the font's cap height).
    pub fn text_in(&mut self, f: &Font, s: &str, size: f32, r: RectF, align: Align, c: Rgba) {
        let w = f.measure(s, size);
        let x = match align {
            Align::Left => r.x,
            Align::Center => r.cx() - w / 2.0,
            Align::Right => r.r() - w,
        };
        let base = r.cy() + f.cap_height(size) / 2.0;
        self.text(f, s, size, x, base, c);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

// ───────────────────────── fonts ─────────────────────────

pub struct Glyph {
    pub xmin: i32,
    pub ymin: i32,
    pub width: usize,
    pub height: usize,
    pub advance: f32,
    pub bitmap: Rc<Vec<u8>>,
}

pub struct Font {
    f: fontdue::Font,
    cache: RefCell<HashMap<(char, u32), Rc<Glyph>>>,
}

impl Font {
    pub fn from_bytes(bytes: &[u8]) -> Option<Font> {
        let f = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
        Some(Font { f, cache: RefCell::new(HashMap::new()) })
    }

    fn key(size: f32) -> u32 {
        (size * 4.0).round() as u32
    }

    pub fn glyph(&self, ch: char, size: f32) -> Rc<Glyph> {
        let k = (ch, Self::key(size));
        if let Some(g) = self.cache.borrow().get(&k) {
            return g.clone();
        }
        let px = k.1 as f32 / 4.0;
        let (m, bmp) = self.f.rasterize(ch, px);
        let g = Rc::new(Glyph {
            xmin: m.xmin,
            ymin: m.ymin,
            width: m.width,
            height: m.height,
            advance: m.advance_width,
            bitmap: Rc::new(bmp),
        });
        let mut c = self.cache.borrow_mut();
        if c.len() > 6000 {
            c.clear();
        }
        c.insert(k, g.clone());
        g
    }

    pub fn kern(&self, a: char, b: char, size: f32) -> f32 {
        self.f.horizontal_kern(a, b, size).unwrap_or(0.0)
    }

    pub fn measure(&self, s: &str, size: f32) -> f32 {
        let mut w = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            if let Some(p) = prev {
                w += self.kern(p, ch, size);
            }
            prev = Some(ch);
            w += self.f.metrics(ch, size).advance_width;
        }
        w
    }

    pub fn cap_height(&self, size: f32) -> f32 {
        let m = self.f.metrics('H', size);
        if m.height > 0 {
            m.height as f32
        } else {
            size * 0.7
        }
    }

    /// Cuts the string to `max_w`, adding "…".
    pub fn ellipsize(&self, s: &str, size: f32, max_w: f32) -> String {
        if self.measure(s, size) <= max_w {
            return s.to_string();
        }
        let ell = self.measure("…", size);
        let mut out = String::new();
        let mut w = 0.0;
        for ch in s.chars() {
            let cw = self.f.metrics(ch, size).advance_width;
            if w + cw + ell > max_w {
                break;
            }
            w += cw;
            out.push(ch);
        }
        out.trim_end().to_string() + "…"
    }
}

pub struct Fonts {
    pub regular: Font,
    pub semibold: Font,
    pub bold: Font,
}

/// Downscales an image to `size`×`size` (high quality), used for icon mip chains.
pub fn resize(src: &Pixmap, size: u32) -> Pixmap {
    let mut dst = Pixmap::new(size, size).unwrap();
    let t = Transform::from_scale(size as f32 / src.width() as f32, size as f32 / src.height() as f32);
    dst.draw_pixmap(0, 0, src.as_ref(), &PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() }, t, None);
    dst
}

/// 256 → 128 → 64 → 32 px chain so every frame scales from the nearest larger copy.
pub fn mips(src: Pixmap) -> Vec<Image> {
    let mut v = Vec::new();
    let mut cur = if src.width() != 256 || src.height() != 256 { resize(&src, 256) } else { src };
    let mut sz = 128;
    v.push(Rc::new(cur.clone()));
    while sz >= 32 {
        cur = resize(&cur, sz);
        v.push(Rc::new(cur.clone()));
        sz /= 2;
    }
    v
}

pub fn mip_for(m: &[Image], size: f32) -> Option<&Image> {
    for i in (0..m.len()).rev() {
        if m[i].width() as f32 >= size {
            return Some(&m[i]);
        }
    }
    m.first()
}

pub fn hsv(h: f32, s: f32, v: f32) -> Rgba {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    rgba(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8, 255)
}

/// Generated tile for items without a usable icon (same look as DockX 1.1).
pub fn fallback_icon(name: &str, fonts: &Fonts, size: u32) -> Pixmap {
    let mut c = Canvas::new(size, size);
    let mut hash: i32 = 17;
    for ch in name.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(ch as i32);
    }
    let hue = ((hash & 0x7fffffff) % 360) as f32;
    let (c1, c2) = (hsv(hue, 0.55, 0.80), hsv((hue + 25.0) % 360.0, 0.75, 0.42));
    let s = size as f32;
    let pad = s * 0.06;
    let r = rect(pad, pad, s - 2.0 * pad, s - 2.0 * pad);
    if let Some(p) = rrect(r, s * 0.22) {
        c.vgradient(&p, r.y, r.b(), c1, c2);
    }
    let letter: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
    c.text_in(&fonts.bold, &letter, s * 0.46, r, Align::Center, rgba(255, 255, 255, 255));
    c.pm
}
