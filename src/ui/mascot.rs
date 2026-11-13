//! «Іскра» — a little ember that lives in the island. Original character, drawn with paths.

use super::gfx::*;
use super::theme::*;
use tiny_skia::PathBuilder;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mood {
    Idle,
    Music,
    Focus,
    Happy,
    Sleep,
    Thirsty,
}

/// Draws the mascot centred at (cx, cy); `size` ≈ body height. `look` = -1..1 horizontal gaze.
pub fn draw(c: &mut Canvas, cx: f32, cy: f32, size: f32, mood: Mood, t: f64, look: f32, alpha: f32) {
    if alpha <= 0.01 || size < 4.0 {
        return;
    }
    let tt = t as f32;
    // body motion
    let (bob, squash) = match mood {
        Mood::Music => {
            let ph = (tt * std::f32::consts::TAU * 1.9).sin();
            (ph * size * 0.07, 1.0 + ph * 0.05)
        }
        Mood::Happy => {
            let ph = ((tt * 5.0).sin()).abs();
            (-ph * size * 0.16, 1.0 - ph * 0.04)
        }
        Mood::Sleep => (0.0, 1.0 + (tt * 1.4).sin() * 0.03),
        _ => (0.0, 1.0 + (tt * 2.1).sin() * 0.025),
    };
    let r = size * 0.42;
    let bx = cx;
    let by = cy + bob + size * 0.05;
    let rw = r * (2.0 - squash);
    let rh = r * squash;

    // flame tuft
    let sway = (tt * 3.3).sin() * size * 0.05;
    let mut pb = PathBuilder::new();
    pb.move_to(bx - rw * 0.55, by - rh * 0.55);
    pb.quad_to(bx - rw * 0.2 + sway, by - rh * 1.55, bx + sway * 1.5, by - rh * 1.75);
    pb.quad_to(bx + rw * 0.1 + sway, by - rh * 1.2, bx + rw * 0.25 + sway, by - rh * 1.35);
    pb.quad_to(bx + rw * 0.65, by - rh * 0.95, bx + rw * 0.55, by - rh * 0.5);
    pb.close();
    if let Some(p) = pb.finish() {
        c.gradient(&p, (bx, by - rh * 1.7), (bx, by - rh * 0.4), &[(0.0, rgba(255, 196, 70, (255.0 * alpha) as u8)), (1.0, ACCENT.alpha(alpha))]);
    }
    // body
    if let Some(p) = PathBuilder::from_oval(tiny_skia::Rect::from_xywh(bx - rw, by - rh, rw * 2.0, rh * 2.0).unwrap()) {
        c.gradient(
            &p,
            (bx - rw * 0.4, by - rh),
            (bx + rw * 0.3, by + rh),
            &[(0.0, rgba(255, 92, 82, (255.0 * alpha) as u8)), (1.0, rgba(176, 22, 34, (255.0 * alpha) as u8))],
        );
    }
    // cheek shine
    c.fill_circle(bx - rw * 0.45, by - rh * 0.45, r * 0.16, rgba(255, 255, 255, (70.0 * alpha) as u8));

    // eyes
    let ex = r * 0.36;
    let ey = by - rh * 0.08;
    let er = r * 0.2;
    let blink = {
        let cyc = (t % 4.3) as f32;
        cyc > 4.12
    };
    let look = look.clamp(-1.0, 1.0) * er * 0.45;
    let white = rgba(255, 255, 255, (255.0 * alpha) as u8);
    let dark = rgba(30, 10, 12, (255.0 * alpha) as u8);
    match mood {
        Mood::Sleep => {
            for sx in [-1.0, 1.0] {
                c.arc(bx + sx * ex, ey, er * 0.8, 1.9, 2.5, dark, r * 0.09);
            }
            let zt = (tt * 0.8).fract();
            let zs = size * (0.18 + zt * 0.12);
            let col = rgba(255, 255, 255, (200.0 * alpha * (1.0 - zt)) as u8);
            let zx = bx + rw * 0.9 + zt * size * 0.25;
            let zy = by - rh * 1.0 - zt * size * 0.45;
            c.line(zx, zy, zx + zs * 0.6, zy, col, size * 0.05);
            c.line(zx + zs * 0.6, zy, zx, zy + zs * 0.6, col, size * 0.05);
            c.line(zx, zy + zs * 0.6, zx + zs * 0.6, zy + zs * 0.6, col, size * 0.05);
        }
        Mood::Happy => {
            for sx in [-1.0, 1.0] {
                c.arc(bx + sx * ex, ey + er * 0.3, er * 0.75, -1.1, 2.2, dark, r * 0.1);
            }
        }
        _ => {
            for sx in [-1.0f32, 1.0] {
                if blink {
                    c.line(bx + sx * ex - er * 0.7, ey, bx + sx * ex + er * 0.7, ey, dark, r * 0.09);
                } else {
                    c.fill_circle(bx + sx * ex, ey, er, white);
                    c.fill_circle(bx + sx * ex + look, ey + er * 0.1, er * 0.55, dark);
                    c.fill_circle(bx + sx * ex + look + er * 0.2, ey - er * 0.2, er * 0.18, white);
                }
            }
            if mood == Mood::Focus {
                // headband
                c.line(bx - rw * 0.92, by - rh * 0.42, bx + rw * 0.92, by - rh * 0.42, rgba(255, 255, 255, (220.0 * alpha) as u8), r * 0.12);
            }
        }
    }
    // mouth
    let my = by + rh * 0.32;
    match mood {
        Mood::Happy | Mood::Music => c.arc(bx, my - r * 0.12, r * 0.18, 1.9, 2.5, dark, r * 0.08),
        Mood::Thirsty => c.fill_circle(bx, my, r * 0.08, dark),
        Mood::Sleep => {}
        _ => c.line(bx - r * 0.1, my, bx + r * 0.1, my, dark, r * 0.07),
    }
    // extras
    match mood {
        Mood::Music => {
            let k = (tt * 1.1).fract();
            let col = rgba(255, 255, 255, (210.0 * alpha * (1.0 - k)) as u8);
            super::icons::draw(c, super::icons::Icon::Music, bx + rw * 1.1 + k * size * 0.2, by - rh * 0.6 - k * size * 0.5, size * 0.42, col);
        }
        Mood::Happy => {
            for i in 0..3 {
                let a = tt * 2.0 + i as f32 * 2.1;
                let d = size * 0.75;
                let (sx, sy) = (bx + a.cos() * d, by - rh * 0.3 + a.sin() * d * 0.5);
                let tw = ((tt * 6.0 + i as f32).sin() * 0.5 + 0.5) * size * 0.08 + size * 0.03;
                c.line(sx - tw, sy, sx + tw, sy, rgba(255, 220, 120, (230.0 * alpha) as u8), size * 0.04);
                c.line(sx, sy - tw, sx, sy + tw, rgba(255, 220, 120, (230.0 * alpha) as u8), size * 0.04);
            }
        }
        Mood::Thirsty => {
            super::icons::draw(c, super::icons::Icon::Drop, bx + rw * 1.05, by - rh * 0.8, size * 0.35, WATER.alpha(alpha));
        }
        _ => {}
    }
}
