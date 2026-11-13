//! One style for the dock and the island: black glass, thin light rim, X44TO red accent.

use super::gfx::*;

pub const ACCENT: Rgba = rgba(226, 38, 50, 255);
pub const ACCENT_DARK: Rgba = rgba(150, 26, 34, 255);
pub const PANEL_TOP: Rgba = rgba(30, 30, 33, 236);
pub const PANEL_BOTTOM: Rgba = rgba(10, 10, 12, 244);
pub const RIM: Rgba = rgba(255, 255, 255, 46);
pub const SHEEN: Rgba = rgba(255, 255, 255, 26);
pub const PILL_BG: Rgba = rgba(20, 20, 23, 240);
pub const TEXT: Rgba = rgba(242, 242, 242, 255);
pub const TEXT2: Rgba = rgba(255, 255, 255, 150);
pub const TEXT3: Rgba = rgba(255, 255, 255, 92);
pub const CONTROL: Rgba = rgba(255, 255, 255, 22);
pub const CONTROL_HOVER: Rgba = rgba(255, 255, 255, 40);
pub const WATER: Rgba = rgba(64, 156, 255, 255);

/// Soft layered shadow under a panel shape (8 expanding rounded rects, like DockX 1.1).
pub fn shadow(c: &mut Canvas, r: RectF, rad: f32, s: f32, alpha: f32, top_attached: bool) {
    for k in (1..=8).rev() {
        let e = k as f32 * 1.6 * s;
        let sr = if top_attached {
            rect(r.x - e, r.y - 4.0 * e, r.w + 2.0 * e, r.h + 5.0 * e + 3.0 * s)
        } else {
            rect(r.x - e, r.y - e + 3.0 * s, r.w + 2.0 * e, r.h + 2.0 * e)
        };
        let a = ((10.0 * (1.0 - k as f32 / 9.0) + 2.0) * alpha) as u8;
        c.fill_rrect(sr, rad + e, rgba(0, 0, 0, a));
    }
}

/// Fills a panel path with the dock's gradient, rim and top sheen.
pub fn panel(c: &mut Canvas, path: &tiny_skia::Path, bounds: RectF, s: f32, alpha: f32, sheen: bool) {
    c.vgradient(path, bounds.y - 1.0, bounds.b() + 1.0, PANEL_TOP.alpha(alpha), PANEL_BOTTOM.alpha(alpha));
    c.stroke_path(path, RIM.alpha(alpha), s.max(1.0));
    if sheen {
        let rad = (bounds.h / 2.0).min(18.0 * s);
        c.line(bounds.x + rad, bounds.y + 1.8 * s, bounds.r() - rad, bounds.y + 1.8 * s, SHEEN.alpha(alpha), s.max(1.0));
    }
}

/// Rounded button chip used across the island.
pub fn chip(c: &mut Canvas, r: RectF, hover: bool, active: bool, alpha: f32) {
    let col = if active {
        ACCENT_DARK.mix(ACCENT, if hover { 0.35 } else { 0.0 })
    } else if hover {
        CONTROL_HOVER
    } else {
        CONTROL
    };
    c.fill_rrect(r, r.h / 2.0, col.alpha(alpha));
}
