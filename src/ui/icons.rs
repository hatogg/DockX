//! Small vector icons (drawn, no image files) so they stay sharp at any DPI.

use super::gfx::*;
use tiny_skia::{LineCap, PathBuilder};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Icon {
    Play,
    Pause,
    Next,
    Prev,
    Music,
    Shelf,
    Timer,
    Drop,
    List,
    Plus,
    Minus,
    Close,
    Check,
    Speaker,
    SpeakerMute,
    Trash,
    Select,
    Skip,
    Reset,
    Bell,
    Lyrics,
}

pub fn draw(c: &mut Canvas, icon: Icon, cx: f32, cy: f32, size: f32, col: Rgba) {
    let s = size;
    let w = (s * 0.11).max(1.2);
    match icon {
        Icon::Play => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.30, cy - s * 0.40);
            pb.line_to(cx + s * 0.42, cy);
            pb.line_to(cx - s * 0.30, cy + s * 0.40);
            pb.close();
            if let Some(p) = pb.finish() {
                c.fill_path(&p, col);
                c.stroke_path(&p, col, s * 0.08);
            }
        }
        Icon::Pause => {
            c.fill_rrect(rect(cx - s * 0.32, cy - s * 0.40, s * 0.22, s * 0.80), s * 0.06, col);
            c.fill_rrect(rect(cx + s * 0.10, cy - s * 0.40, s * 0.22, s * 0.80), s * 0.06, col);
        }
        Icon::Next | Icon::Prev => {
            let d = if icon == Icon::Next { 1.0 } else { -1.0 };
            for k in 0..2 {
                let ox = cx + d * (k as f32 * s * 0.36 - s * 0.36);
                let mut pb = PathBuilder::new();
                pb.move_to(ox, cy - s * 0.30);
                pb.line_to(ox + d * s * 0.36, cy);
                pb.line_to(ox, cy + s * 0.30);
                pb.close();
                if let Some(p) = pb.finish() {
                    c.fill_path(&p, col);
                    c.stroke_path(&p, col, s * 0.06);
                }
            }
            c.fill_rrect(rect(cx + d * s * 0.36 - s * 0.05, cy - s * 0.30, s * 0.10, s * 0.60), s * 0.03, col);
        }
        Icon::Music => {
            c.fill_circle(cx - s * 0.18, cy + s * 0.24, s * 0.16, col);
            c.line(cx - s * 0.04, cy + s * 0.24, cx - s * 0.04, cy - s * 0.36, col, w);
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.04, cy - s * 0.36);
            pb.quad_to(cx + s * 0.18, cy - s * 0.30, cx + s * 0.28, cy - s * 0.12);
            if let Some(p) = pb.finish() {
                c.stroke_path_cap(&p, col, w, LineCap::Round);
            }
        }
        Icon::Shelf => {
            // tray with an arrow dropping in
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.40, cy + s * 0.02);
            pb.line_to(cx - s * 0.40, cy + s * 0.34);
            pb.line_to(cx + s * 0.40, cy + s * 0.34);
            pb.line_to(cx + s * 0.40, cy + s * 0.02);
            pb.move_to(cx - s * 0.40, cy + s * 0.08);
            pb.line_to(cx - s * 0.14, cy + s * 0.08);
            pb.line_to(cx - s * 0.10, cy + s * 0.18);
            pb.line_to(cx + s * 0.10, cy + s * 0.18);
            pb.line_to(cx + s * 0.14, cy + s * 0.08);
            pb.line_to(cx + s * 0.40, cy + s * 0.08);
            pb.move_to(cx, cy - s * 0.40);
            pb.line_to(cx, cy - s * 0.02);
            pb.move_to(cx - s * 0.13, cy - s * 0.15);
            pb.line_to(cx, cy - s * 0.02);
            pb.line_to(cx + s * 0.13, cy - s * 0.15);
            if let Some(p) = pb.finish() {
                c.stroke_path_cap(&p, col, w, LineCap::Round);
            }
        }
        Icon::Timer => {
            c.stroke_circle(cx, cy + s * 0.05, s * 0.34, col, w);
            c.line(cx, cy + s * 0.05, cx, cy - s * 0.14, col, w);
            c.line(cx, cy + s * 0.05, cx + s * 0.13, cy + s * 0.12, col, w);
            c.line(cx - s * 0.10, cy - s * 0.42, cx + s * 0.10, cy - s * 0.42, col, w);
        }
        Icon::Drop => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy - s * 0.44);
            pb.cubic_to(cx + s * 0.12, cy - s * 0.24, cx + s * 0.34, cy - s * 0.02, cx + s * 0.34, cy + s * 0.14);
            pb.cubic_to(cx + s * 0.34, cy + s * 0.34, cx + s * 0.18, cy + s * 0.46, cx, cy + s * 0.46);
            pb.cubic_to(cx - s * 0.18, cy + s * 0.46, cx - s * 0.34, cy + s * 0.34, cx - s * 0.34, cy + s * 0.14);
            pb.cubic_to(cx - s * 0.34, cy - s * 0.02, cx - s * 0.12, cy - s * 0.24, cx, cy - s * 0.44);
            pb.close();
            if let Some(p) = pb.finish() {
                c.fill_path(&p, col);
            }
        }
        Icon::List => {
            for k in 0..3 {
                let y = cy - s * 0.28 + k as f32 * s * 0.28;
                c.fill_circle(cx - s * 0.30, y, s * 0.06, col);
                c.line(cx - s * 0.12, y, cx + s * 0.38, y, col, w);
            }
        }
        Icon::Plus | Icon::Minus => {
            c.line(cx - s * 0.32, cy, cx + s * 0.32, cy, col, w * 1.2);
            if icon == Icon::Plus {
                c.line(cx, cy - s * 0.32, cx, cy + s * 0.32, col, w * 1.2);
            }
        }
        Icon::Close => {
            c.line(cx - s * 0.26, cy - s * 0.26, cx + s * 0.26, cy + s * 0.26, col, w * 1.1);
            c.line(cx + s * 0.26, cy - s * 0.26, cx - s * 0.26, cy + s * 0.26, col, w * 1.1);
        }
        Icon::Check => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.30, cy + s * 0.02);
            pb.line_to(cx - s * 0.08, cy + s * 0.24);
            pb.line_to(cx + s * 0.32, cy - s * 0.24);
            if let Some(p) = pb.finish() {
                c.stroke_path_cap(&p, col, w * 1.3, LineCap::Round);
            }
        }
        Icon::Speaker | Icon::SpeakerMute => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.40, cy - s * 0.12);
            pb.line_to(cx - s * 0.22, cy - s * 0.12);
            pb.line_to(cx + s * 0.02, cy - s * 0.34);
            pb.line_to(cx + s * 0.02, cy + s * 0.34);
            pb.line_to(cx - s * 0.22, cy + s * 0.12);
            pb.line_to(cx - s * 0.40, cy + s * 0.12);
            pb.close();
            if let Some(p) = pb.finish() {
                c.fill_path(&p, col);
            }
            if icon == Icon::Speaker {
                c.arc(cx + s * 0.04, cy, s * 0.18, 0.75, 1.6, col, w);
                c.arc(cx + s * 0.04, cy, s * 0.34, 0.80, 1.5, col, w);
            } else {
                c.line(cx + s * 0.14, cy - s * 0.14, cx + s * 0.40, cy + s * 0.14, col, w);
                c.line(cx + s * 0.40, cy - s * 0.14, cx + s * 0.14, cy + s * 0.14, col, w);
            }
        }
        Icon::Trash => {
            c.line(cx - s * 0.34, cy - s * 0.26, cx + s * 0.34, cy - s * 0.26, col, w);
            c.line(cx - s * 0.10, cy - s * 0.38, cx + s * 0.10, cy - s * 0.38, col, w);
            if let Some(p) = rrect4(rect(cx - s * 0.25, cy - s * 0.18, s * 0.50, s * 0.58), 0.0, 0.0, s * 0.08, s * 0.08) {
                c.stroke_path(&p, col, w);
            }
        }
        Icon::Select => {
            c.stroke_rrect(rect(cx - s * 0.36, cy - s * 0.36, s * 0.72, s * 0.72), s * 0.16, col, w);
            draw(c, Icon::Check, cx, cy, s * 0.62, col);
        }
        Icon::Skip => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.30, cy - s * 0.32);
            pb.line_to(cx + s * 0.16, cy);
            pb.line_to(cx - s * 0.30, cy + s * 0.32);
            pb.close();
            if let Some(p) = pb.finish() {
                c.fill_path(&p, col);
            }
            c.fill_rrect(rect(cx + s * 0.20, cy - s * 0.32, s * 0.11, s * 0.64), s * 0.03, col);
        }
        Icon::Reset => {
            c.arc(cx, cy, s * 0.32, -2.4, 4.6, col, w);
            let a = -2.4f32;
            let (x, y) = (cx + s * 0.32 * a.sin(), cy - s * 0.32 * a.cos());
            let mut pb = PathBuilder::new();
            pb.move_to(x - s * 0.02, y - s * 0.18);
            pb.line_to(x + s * 0.02, y + s * 0.02);
            pb.line_to(x + s * 0.20, y - s * 0.06);
            if let Some(p) = pb.finish() {
                c.stroke_path_cap(&p, col, w, LineCap::Round);
            }
        }
        Icon::Bell => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx - s * 0.32, cy + s * 0.20);
            pb.cubic_to(cx - s * 0.22, cy + s * 0.08, cx - s * 0.26, cy - s * 0.36, cx, cy - s * 0.36);
            pb.cubic_to(cx + s * 0.26, cy - s * 0.36, cx + s * 0.22, cy + s * 0.08, cx + s * 0.32, cy + s * 0.20);
            pb.close();
            if let Some(p) = pb.finish() {
                c.fill_path(&p, col);
            }
            c.fill_circle(cx, cy + s * 0.30, s * 0.08, col);
        }
        Icon::Lyrics => {
            c.stroke_rrect(rect(cx - s * 0.40, cy - s * 0.32, s * 0.80, s * 0.58), s * 0.14, col, w);
            c.line(cx - s * 0.22, cy - s * 0.10, cx + s * 0.22, cy - s * 0.10, col, w);
            c.line(cx - s * 0.22, cy + s * 0.06, cx + s * 0.08, cy + s * 0.06, col, w);
        }
    }
}
