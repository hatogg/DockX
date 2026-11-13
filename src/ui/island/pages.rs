//! Island pages: Today, Music (with split karaoke lyrics), Shelf + Clipboard, Focus, Water, To-do,
//! Widgets, Prayer — plus the page dots and every click handler.

use super::*;

const GREEN: Rgba = rgba(110, 214, 150, 255);

impl Island {
    // ───────── Today ─────────

    pub(super) fn draw_today(&mut self, g: &mut Canvas, r: RectF, a: f32, _live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let pad = 20.0 * s;
        let x = r.x + pad;
        g.text(&fonts.bold, &self.lt.hm(), 40.0 * s, x, r.y + 56.0 * s, TEXT.alpha(a));
        g.text(&fonts.regular, &time::label(self.lt.days), 12.5 * s, x, r.y + 78.0 * s, TEXT2.alpha(a));
        // stats
        let mut y = r.y + 104.0 * s;
        let col_w = 200.0 * s;
        let water = self.water_ml() as f32 / self.cfg.goal_ml.max(1) as f32;
        let stats: Vec<(Icon, String, f32, Rgba)> = {
            let mut v = vec![
                (Icon::Drop, format!("{} / {} мл", self.water_ml(), self.cfg.goal_ml), water, WATER),
                (Icon::Timer, format!("Фокус: {} сес.", self.focus_done), (self.focus_done as f32 / 8.0).min(1.0), ACCENT),
            ];
            let undone = self.todos.iter().filter(|t| t.day == self.lt.days && !t.done && !t.removing).count();
            let total = self.todos.iter().filter(|t| t.day == self.lt.days && !t.removing).count();
            v.push((Icon::List, format!("Справи: {} / {}", total - undone, total), if total > 0 { (total - undone) as f32 / total as f32 } else { 0.0 }, GREEN));
            v
        };
        for (icon, text, frac, col) in stats {
            icons::draw(g, icon, x + 8.0 * s, y, 13.0 * s, col.alpha(a));
            g.text(&fonts.regular, &text, 12.5 * s, x + 24.0 * s, y + 4.5 * s, TEXT.alpha(a));
            let br = rect(x + 24.0 * s, y + 11.0 * s, col_w - 30.0 * s, 4.0 * s);
            g.fill_rrect(br, 2.0 * s, rgba(255, 255, 255, (30.0 * a) as u8));
            g.fill_rrect(rect(br.x, br.y, br.w * frac.clamp(0.0, 1.0), br.h), 2.0 * s, col.alpha(a));
            y += 34.0 * s;
        }
        if let Some(b) = self.battery {
            let t = format!("{}%{}", b.percent, if b.charging { " · заряджається" } else { "" });
            g.text(&fonts.regular, &t, 11.5 * s, x + 24.0 * s, y + 2.0 * s, TEXT3.alpha(a));
            bolt(g, x + 8.0 * s, y - 2.0 * s, 6.0 * s, if b.plugged { GREEN } else { TEXT3 }.alpha(a));
        }
        // right: next up + events
        let rx = r.x + 250.0 * s;
        let rw = r.r() - pad - rx;
        let card = rect(rx, r.y + 16.0 * s, rw, 64.0 * s);
        g.fill_rrect(card, 14.0 * s, rgba(255, 255, 255, (12.0 * a) as u8));
        g.text(&fonts.regular, "Найближче", 11.0 * s, card.x + 14.0 * s, card.y + 20.0 * s, TEXT3.alpha(a));
        let next = self.next_up();
        let nt = fonts.semibold.ellipsize(&next, 14.0 * s, card.w - 28.0 * s);
        g.text(&fonts.semibold, &nt, 14.0 * s, card.x + 14.0 * s, card.y + 44.0 * s, TEXT.alpha(a));
        g.text(&fonts.regular, "Події", 11.0 * s, rx + 2.0 * s, card.b() + 22.0 * s, TEXT3.alpha(a));
        let mut ey = card.b() + 42.0 * s;
        if self.events.is_empty() {
            g.text(&fonts.regular, "Поки тихо. Тут зʼявлятимуться події острова.", 12.0 * s, rx + 2.0 * s, ey, TEXT3.alpha(a));
        }
        for i in 0..self.events.len().min(4) {
            let (icon, text, unix) = (self.events[i].icon, self.events[i].text.clone(), self.events[i].unix);
            icons::draw(g, icon, rx + 8.0 * s, ey - 4.0 * s, 11.0 * s, TEXT2.alpha(a));
            let when = time::local(unix, self.tz_min).hm();
            let ww = fonts.regular.measure(&when, 11.0 * s);
            let t = fonts.regular.ellipsize(&text, 12.0 * s, rw - ww - 34.0 * s);
            g.text(&fonts.regular, &t, 12.0 * s, rx + 22.0 * s, ey, TEXT.alpha(a));
            g.text(&fonts.regular, &when, 11.0 * s, rx + rw - ww, ey, TEXT3.alpha(a));
            ey += 22.0 * s;
        }
    }

    fn next_up(&self) -> String {
        let now_min = self.lt.minutes();
        let mut best: Option<(i64, String)> = None;
        for t in &self.todos {
            if t.done || t.removing {
                continue;
            }
            if let Some(m) = t.remind {
                let k = (t.day - self.lt.days) * 1440 + m as i64 - now_min as i64;
                if k >= 0 && best.as_ref().map(|b| k < b.0).unwrap_or(true) {
                    best = Some((k, format!("{} · {:02}:{:02}", t.text, m / 60, m % 60)));
                }
            }
        }
        if let Some(pt) = self.prayer_times() {
            for i in [0usize, 2, 3, 4, 5] {
                let k = (pt[i] * 60.0) as i64 - now_min as i64;
                if k >= 0 && best.as_ref().map(|b| k < b.0).unwrap_or(true) {
                    best = Some((k, format!("{} · {}", prayer::NAMES[i], prayer::fmt(pt[i]))));
                }
            }
        }
        if let Some((_, s)) = best {
            return s;
        }
        if let Some(t) = self.todos.iter().find(|t| !t.done && !t.removing && t.day == self.lt.days) {
            return t.text.clone();
        }
        "Нічого запланованого — гарного дня!".into()
    }

    // ───────── Music ─────────

    pub(super) fn draw_music(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let pad = 18.0 * s;
        let fonts = self.fonts.clone();
        // one island: player on the left, lyrics on the right (lyrics button opens/closes them)
        let left = rect(r.x, r.y, (472.0 * s - 2.0 * self.ear.v).min(r.w), r.h);
        let right = if self.lyrics_open && r.w - left.w > 80.0 * s { Some(rect(left.r(), r.y, r.r() - left.r(), r.h)) } else { None };
        if !self.media.has {
            let c = rect(r.x, r.y, r.w, r.h - 24.0 * s);
            mascot::draw(g, c.cx(), c.cy() - 22.0 * s, 46.0 * s, Mood::Sleep, self.now, 0.0, a);
            g.text_center(&fonts.semibold, "Нічого не грає", 15.0 * s, c.cx(), c.cy() + 30.0 * s, TEXT.alpha(a));
            g.text_center(&fonts.regular, "Увімкни музику: Spotify, YouTube у браузері, будь-який плеєр", 12.0 * s, c.cx(), c.cy() + 50.0 * s, TEXT3.alpha(a));
            return;
        }
        let art_r = rect(left.x + pad, left.y + pad, 112.0 * s, 112.0 * s);
        self.zone(art_r, Hot::Art, live);
        if let Some(art) = self.media.art.clone() {
            g.radial(art_r.cx(), art_r.cy() + 6.0 * s, art_r.w * 0.85, self.art_color.alpha(a * 0.35), self.art_color.with_a(0));
            g.image_rounded(&art, art_r, 16.0 * s, a);
        } else {
            if let Some(p) = rrect(art_r, 16.0 * s) {
                g.gradient(&p, (art_r.x, art_r.y), (art_r.r(), art_r.b()), &[(0.0, ACCENT.alpha(a)), (1.0, rgba(60, 10, 20, (255.0 * a) as u8))]);
            }
            icons::draw(g, Icon::Music, art_r.cx(), art_r.cy(), 44.0 * s, TEXT.alpha(a));
        }
        let np = self.media.players.len();
        if np > 1 {
            let t = format!("{}/{}", self.media.player_idx + 1, np);
            let tw = fonts.semibold.measure(&t, 10.5 * s);
            let br = rect(art_r.r() - tw - 14.0 * s, art_r.b() - 24.0 * s, tw + 10.0 * s, 18.0 * s);
            g.fill_rrect(br, 9.0 * s, rgba(0, 0, 0, (170.0 * a) as u8));
            g.text_in(&fonts.semibold, &t, 10.5 * s, br, Align::Center, TEXT.alpha(a));
            if self.is_hot(Hot::Art) {
                g.stroke_rrect(art_r, 16.0 * s, TEXT.alpha(a * 0.8), 1.5 * s);
            }
        }
        // info
        let ix = art_r.r() + 16.0 * s;
        let iw = left.r() - pad - ix;
        let lb = rect(left.r() - pad - 30.0 * s, left.y + pad - 4.0 * s, 30.0 * s, 30.0 * s);
        self.zone(lb, Hot::LyricsBtn, live);
        let lon = self.lyrics_open;
        g.fill_circle(lb.cx(), lb.cy(), 15.0 * s, if lon { ACCENT_DARK } else if self.is_hot(Hot::LyricsBtn) { CONTROL_HOVER } else { CONTROL }.alpha(a));
        icons::draw(g, Icon::Lyrics, lb.cx(), lb.cy(), 15.0 * s, TEXT.alpha(a));
        let tw_max = iw - 38.0 * s;
        let title = fonts.semibold.ellipsize(&self.media.title, 16.0 * s, tw_max);
        g.text(&fonts.semibold, &title, 16.0 * s, ix, left.y + pad + 16.0 * s, TEXT.alpha(a));
        let artist = fonts.regular.ellipsize(&self.media.artist, 13.0 * s, tw_max);
        g.text(&fonts.regular, &artist, 13.0 * s, ix, left.y + pad + 36.0 * s, TEXT2.alpha(a));
        if !self.media.app.is_empty() {
            let app = if np > 1 { format!("{}  ·  клік по обкладинці — інший плеєр", self.media.app) } else { self.media.app.clone() };
            let app = fonts.regular.ellipsize(&app, 11.0 * s, iw);
            g.text(&fonts.regular, &app, 11.0 * s, ix, left.y + pad + 54.0 * s, TEXT3.alpha(a));
        }
        // progress
        let pos = self.media.pos_at(self.now);
        let dur = self.media.duration.max(0.0);
        let frac = match self.scrub {
            Some(f) => f,
            None if dur > 0.0 => (pos / dur) as f32,
            _ => 0.0,
        };
        let bar = rect(ix, left.y + pad + 70.0 * s, iw, 18.0 * s);
        self.zone(bar, Hot::Seek, live);
        let hot = self.is_hot(Hot::Seek) || self.scrub.is_some();
        let bh = if hot { 6.0 * s } else { 4.0 * s };
        let track = rect(bar.x, bar.cy() - bh / 2.0, bar.w, bh);
        g.fill_rrect(track, bh / 2.0, rgba(255, 255, 255, (38.0 * a) as u8));
        g.fill_rrect(rect(track.x, track.y, track.w * frac, bh), bh / 2.0, self.art_color.mix(TEXT, 0.35).alpha(a));
        if hot {
            g.fill_circle(track.x + track.w * frac, track.cy(), 6.0 * s, TEXT.alpha(a));
        }
        let shown_pos = if self.scrub.is_some() { frac as f64 * dur } else { pos };
        g.text(&fonts.regular, &lrc::fmt_time(shown_pos), 10.5 * s, ix, bar.b() + 10.0 * s, TEXT3.alpha(a));
        let rem = format!("-{}", lrc::fmt_time((dur - shown_pos).max(0.0)));
        let rwid = fonts.regular.measure(&rem, 10.5 * s);
        g.text(&fonts.regular, &rem, 10.5 * s, ix + iw - rwid, bar.b() + 10.0 * s, TEXT3.alpha(a));
        // controls
        let cy = left.y + pad + 122.0 * s;
        let mid = ix + iw / 2.0;
        let pr = rect(mid - 21.0 * s, cy - 21.0 * s, 42.0 * s, 42.0 * s);
        self.zone(pr, Hot::PlayPause, live);
        let hp = self.is_hot(Hot::PlayPause);
        g.fill_circle(pr.cx(), pr.cy(), 21.0 * s, if hp { TEXT.alpha(a) } else { rgba(236, 236, 236, (235.0 * a) as u8) });
        let playing = self.media.playing;
        icons::draw(g, if playing { Icon::Pause } else { Icon::Play }, pr.cx() + if playing { 0.0 } else { 1.5 * s }, pr.cy(), 18.0 * s, rgba(12, 12, 14, (255.0 * a) as u8));
        for (hot, icon, dx, en) in [(Hot::Prev, Icon::Prev, -62.0, self.media.can_prev), (Hot::Next, Icon::Next, 62.0, self.media.can_next)] {
            let br = rect(mid + dx * s - 18.0 * s, cy - 18.0 * s, 36.0 * s, 36.0 * s);
            self.zone(br, hot, live && en);
            if self.is_hot(hot) {
                g.fill_circle(br.cx(), br.cy(), 18.0 * s, CONTROL_HOVER.alpha(a));
            }
            icons::draw(g, icon, br.cx(), br.cy(), 16.0 * s, if en { TEXT } else { TEXT3 }.alpha(a));
        }
        // volume
        let vy = left.y + pad + 160.0 * s;
        let vi = rect(ix - 4.0 * s, vy - 11.0 * s, 22.0 * s, 22.0 * s);
        self.zone(vi, Hot::VolIcon, live);
        let mute = self.muted || self.volume < 0.005;
        icons::draw(g, if mute { Icon::SpeakerMute } else { Icon::Speaker }, vi.cx(), vi.cy(), 14.0 * s, if self.is_hot(Hot::VolIcon) { TEXT } else { TEXT2 }.alpha(a));
        let vb = rect(ix + 24.0 * s, vy - 9.0 * s, iw - 24.0 * s, 18.0 * s);
        self.zone(vb, Hot::Volume, live);
        let vh = if self.is_hot(Hot::Volume) || self.vol_drag { 6.0 * s } else { 4.0 * s };
        let vt = rect(vb.x, vb.cy() - vh / 2.0, vb.w, vh);
        g.fill_rrect(vt, vh / 2.0, rgba(255, 255, 255, (38.0 * a) as u8));
        let v = if self.muted { 0.0 } else { self.volume };
        g.fill_rrect(rect(vt.x, vt.y, vt.w * v, vh), vh / 2.0, TEXT2.alpha(a));
        if self.is_hot(Hot::Volume) || self.vol_drag || self.now - self.vol_flash < 1.0 {
            let pct = format!("{}%", (v * 100.0).round());
            let tw = fonts.regular.measure(&pct, 10.5 * s);
            g.text(&fonts.regular, &pct, 10.5 * s, vt.r() - tw, vt.y - 6.0 * s, TEXT2.alpha(a));
        }
        // lyrics column inside the same island, fades in as the island widens
        if let Some(lr) = right {
            let la = a * smoothstep(120.0 * s, 300.0 * s, lr.w);
            if la > 0.01 {
                let panel = rect(lr.x + 2.0 * s, lr.y + 14.0 * s, lr.w - 18.0 * s, lr.h - 44.0 * s);
                g.fill_rrect(panel, 14.0 * s, rgba(255, 255, 255, (10.0 * la) as u8));
                self.draw_lyrics(g, panel.inflate(-4.0 * s, -2.0 * s), la, live);
            }
        }
    }

    fn draw_lyrics(&mut self, g: &mut Canvas, lr: RectF, a: f32, live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        match self.lyrics.clone() {
            Lyrics::Synced(lines) => {
                let center = lr.cy();
                let step = 28.0 * s;
                let sc = self.lyric_scroll.v;
                let pos = self.media.pos_at(self.now) + 0.15;
                let (cur, prog) = lrc::current(&lines, pos);
                let lo = (sc - 4.0).floor().max(0.0) as usize;
                let hi = ((sc + 5.0).ceil() as usize).min(lines.len());
                for i in lo..hi {
                    let d = i as f32 - sc;
                    let y = center + d * step;
                    if y < lr.y - step || y > lr.b() + step {
                        continue;
                    }
                    let k = (1.0 - d.abs()).max(0.0);
                    let mut size = lerp(12.5, 17.5, k) * s;
                    let f = if k > 0.5 { &fonts.semibold } else { &fonts.regular };
                    let edge = 1.0 - smoothstep(lr.h * 0.30, lr.h * 0.52, (y - center).abs());
                    let avail = lr.w - 24.0 * s;
                    let mw = f.measure(&lines[i].text, size);
                    if mw > avail {
                        size = (size * avail / mw).max(10.5 * s);
                    }
                    let text = f.ellipsize(&lines[i].text, size, avail);
                    let tw = f.measure(&text, size);
                    let x0 = lr.cx() - tw / 2.0;
                    let base = y + f.cap_height(size) / 2.0;
                    let zr = rect(lr.x, y - step / 2.0, lr.w, step);
                    if edge > 0.3 && zr.y >= lr.y - 4.0 && zr.b() <= lr.b() + 4.0 {
                        self.zone(zr, Hot::Line(i), live);
                    }
                    if self.is_hot(Hot::Line(i)) {
                        g.fill_rrect(zr.inflate(-4.0 * s, -2.0 * s), 10.0 * s, CONTROL.alpha(a * edge));
                    }
                    if i as i32 == cur {
                        // karaoke: the sung part of the line lights up
                        g.text(f, &text, size, x0, base, rgba(255, 255, 255, (110.0 * a * edge) as u8));
                        g.set_clip_rect(Some(rect(x0 - 2.0, y - step, tw * prog + 2.0, step * 2.0)));
                        g.text(f, &text, size, x0, base, self.art_color.mix(TEXT, 0.55).alpha(a * edge));
                        g.set_clip_rect(None);
                    } else {
                        let alpha = a * edge * lerp(0.38, 0.8, k);
                        g.text(f, &text, size, x0, base, rgba(255, 255, 255, (255.0 * alpha) as u8));
                    }
                }
            }
            Lyrics::Plain(lines) => {
                let mut y = lr.y + 24.0 * s;
                for l in lines.iter().take(7) {
                    let t = fonts.regular.ellipsize(l, 12.5 * s, lr.w - 20.0 * s);
                    g.text_center(&fonts.regular, &t, 12.5 * s, lr.cx(), y, TEXT2.alpha(a));
                    y += 22.0 * s;
                }
            }
            Lyrics::Loading => {
                g.text_center(&fonts.regular, "Шукаю текст пісні…", 12.5 * s, lr.cx(), lr.cy() + 4.0 * s, TEXT3.alpha(a));
            }
            Lyrics::NotFound | Lyrics::None => {
                icons::draw(g, Icon::Lyrics, lr.cx(), lr.cy() - 12.0 * s, 22.0 * s, TEXT3.alpha(a));
                let t = if self.cfg.lyrics_online { "Текст не знайдено" } else { "Пошук текстів вимкнено" };
                g.text_center(&fonts.regular, t, 12.5 * s, lr.cx(), lr.cy() + 20.0 * s, TEXT3.alpha(a));
            }
        }
    }

    // ───────── Shelf + Clipboard ─────────

    pub(super) fn draw_shelf(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool, drop: bool) {
        let s = self.s;
        let pad = 18.0 * s;
        let fonts = self.fonts.clone();
        let hy = r.y + pad + 10.0 * s;
        let tab = if drop { 0 } else { self.shelf_tab };
        // tabs
        let mut tx = r.x + pad;
        for (i, label) in [format!("Файли  {}", self.shelf.len()), format!("Буфер  {}", self.clips.len())].iter().enumerate() {
            let tw = fonts.semibold.measure(label, 12.5 * s);
            let tr = rect(tx, hy - 13.0 * s, tw + 24.0 * s, 26.0 * s);
            self.zone(tr, Hot::Tab(i as u8), live && !drop);
            let active = tab == i as u8;
            if active {
                g.fill_rrect(tr, tr.h / 2.0, rgba(255, 255, 255, (34.0 * a) as u8));
            } else if self.is_hot(Hot::Tab(i as u8)) {
                g.fill_rrect(tr, tr.h / 2.0, CONTROL.alpha(a));
            }
            g.text_in(&fonts.semibold, label, 12.5 * s, tr, Align::Center, if active { TEXT } else { TEXT2 }.alpha(a));
            tx = tr.r() + 6.0 * s;
        }
        let area = rect(r.x + pad, r.y + 50.0 * s, r.w - 2.0 * pad, r.h - 50.0 * s - 30.0 * s);
        if tab == 1 {
            self.draw_clipboard(g, r, area, hy, a, live);
            return;
        }
        // header buttons (right to left)
        let mut bx = r.r() - pad;
        let sel = self.shelf.iter().filter(|x| x.selected).count();
        if !drop && !self.shelf.is_empty() {
            let mut buttons: Vec<(Hot, String, bool)> = vec![(Hot::ShelfClear, "Очистити".into(), false)];
            if sel > 0 {
                buttons.push((Hot::ShelfRemove, format!("Прибрати ({})", sel), true));
            }
            buttons.push((Hot::ShelfSelect, if self.shelf_select { "Готово".into() } else { "Вибрати".into() }, self.shelf_select));
            for (hot, label, active) in buttons {
                let tw = fonts.regular.measure(&label, 12.0 * s);
                let br = rect(bx - tw - 22.0 * s, hy - 12.0 * s, tw + 22.0 * s, 24.0 * s);
                self.zone(br, hot, live);
                chip(g, br, self.is_hot(hot), active, a);
                g.text_in(&fonts.regular, &label, 12.0 * s, br, Align::Center, TEXT.alpha(a));
                bx = br.x - 8.0 * s;
            }
        }
        if drop || self.shelf.is_empty() {
            let col = if drop { ACCENT } else { rgba(255, 255, 255, 70) };
            let phase = if drop { -(self.now as f32 * 18.0 * s) } else { 0.0 };
            if self.shelf.is_empty() || drop && self.shelf.is_empty() {
                g.stroke_rrect_dash(area, 16.0 * s, col.alpha(a), 1.6 * s, 7.0 * s, phase);
                if drop {
                    g.fill_rrect(area, 16.0 * s, ACCENT.alpha(a * 0.08));
                }
                icons::draw(g, Icon::Shelf, area.cx(), area.cy() - 12.0 * s, 26.0 * s, if drop { ACCENT } else { TEXT3 }.alpha(a));
                let t = if drop { "Відпусти, щоб покласти на полицю" } else { "Перетягни файли сюди — тут зберігаються лише посилання" };
                g.text_center(&fonts.regular, t, 12.5 * s, area.cx(), area.cy() + 22.0 * s, if drop { TEXT } else { TEXT3 }.alpha(a));
                return;
            }
        }
        let tile_w = 86.0 * s;
        let scroll = self.shelf_scroll.v;
        g.set_clip_rect(Some(area));
        let n = self.shelf.len();
        for i in 0..n {
            let x = area.x + i as f32 * tile_w - scroll;
            if x + tile_w < area.x || x > area.r() {
                continue;
            }
            let tr = rect(x, area.y, tile_w, area.h);
            let it_sel = self.shelf[i].selected;
            let ap = self.shelf[i].appear.v;
            self.zone(tr, Hot::Tile(i), live && !drop);
            let hot = self.is_hot(Hot::Tile(i)) || self.is_hot(Hot::TileX(i));
            if it_sel || hot {
                let bg = if it_sel { ACCENT_DARK.alpha(a * 0.55) } else { CONTROL.alpha(a) };
                g.fill_rrect(tr.inflate(-3.0 * s, -2.0 * s), 12.0 * s, bg);
                if it_sel {
                    g.stroke_rrect(tr.inflate(-3.0 * s, -2.0 * s), 12.0 * s, ACCENT.alpha(a), 1.4 * s);
                }
            }
            let isz = 52.0 * s * ease_out_cubic(ap);
            let ir = rect(tr.cx() - isz / 2.0, tr.y + 10.0 * s + (52.0 * s - isz) / 2.0, isz, isz);
            if let Some(icon) = self.shelf[i].icon.clone() {
                g.image(&icon, ir, a * ap);
            } else {
                g.fill_rrect(ir, 10.0 * s, CONTROL.alpha(a));
            }
            let name = fonts.regular.ellipsize(&self.shelf[i].name, 11.0 * s, tile_w - 10.0 * s);
            g.text_center(&fonts.regular, &name, 11.0 * s, tr.cx(), tr.y + 80.0 * s, TEXT2.alpha(a * ap));
            if hot && !self.shelf_select && !drop {
                let xr = rect(tr.r() - 24.0 * s, tr.y + 2.0 * s, 20.0 * s, 20.0 * s);
                self.zone(xr, Hot::TileX(i), live);
                g.fill_circle(xr.cx(), xr.cy(), 9.0 * s, if self.is_hot(Hot::TileX(i)) { ACCENT } else { rgba(70, 70, 76, 255) }.alpha(a));
                icons::draw(g, Icon::Close, xr.cx(), xr.cy(), 10.0 * s, TEXT.alpha(a));
            }
            if self.shelf_select {
                let cr = (tr.x + 14.0 * s, tr.y + 14.0 * s);
                g.fill_circle(cr.0, cr.1, 8.0 * s, if it_sel { ACCENT } else { rgba(0, 0, 0, 160) }.alpha(a));
                g.stroke_circle(cr.0, cr.1, 8.0 * s, TEXT.alpha(a), 1.3 * s);
                if it_sel {
                    icons::draw(g, Icon::Check, cr.0, cr.1, 11.0 * s, TEXT.alpha(a));
                }
            }
        }
        g.set_clip_rect(None);
        if drop {
            g.fill_rrect(area, 16.0 * s, rgba(0, 0, 0, (150.0 * a) as u8));
            g.stroke_rrect_dash(area, 16.0 * s, ACCENT.alpha(a), 1.6 * s, 7.0 * s, -(self.now as f32 * 18.0 * s));
            g.text_center(&fonts.semibold, "Відпусти, щоб покласти на полицю", 13.0 * s, area.cx(), area.cy() + 5.0 * s, TEXT.alpha(a));
        }
    }

    fn draw_clipboard(&mut self, g: &mut Canvas, r: RectF, area: RectF, hy: f32, a: f32, live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let pad = 18.0 * s;
        if !self.clips.is_empty() {
            let label = "Очистити";
            let tw = fonts.regular.measure(label, 12.0 * s);
            let br = rect(r.r() - pad - tw - 22.0 * s, hy - 12.0 * s, tw + 22.0 * s, 24.0 * s);
            self.zone(br, Hot::ShelfClear, live);
            chip(g, br, self.is_hot(Hot::ShelfClear), false, a);
            g.text_in(&fonts.regular, label, 12.0 * s, br, Align::Center, TEXT.alpha(a));
        }
        if !self.cfg.clip_on || self.clips.is_empty() {
            let t = if self.cfg.clip_on { "Скопіюй текст чи посилання — він зʼявиться тут" } else { "Історію буфера вимкнено в налаштуваннях" };
            icons::draw(g, Icon::List, area.cx(), area.cy() - 12.0 * s, 22.0 * s, TEXT3.alpha(a));
            g.text_center(&fonts.regular, t, 12.5 * s, area.cx(), area.cy() + 20.0 * s, TEXT3.alpha(a));
            return;
        }
        let row_h = 30.0 * s;
        let max = ((self.clips.len() as f32 * row_h) - area.h).max(0.0);
        self.clip_scroll = self.clip_scroll.min(max);
        g.set_clip_rect(Some(area));
        let mut y = area.y - self.clip_scroll;
        for i in 0..self.clips.len() {
            let row = rect(area.x - 6.0 * s, y, area.w + 12.0 * s, row_h);
            y += row_h;
            if row.b() < area.y || row.y > area.b() {
                continue;
            }
            let ap = self.clips[i].appear.v;
            self.zone(row, Hot::Clip(i), live);
            let hot = self.is_hot(Hot::Clip(i)) || self.is_hot(Hot::ClipX(i));
            if hot {
                g.fill_rrect(row.inflate(0.0, -1.0), 9.0 * s, CONTROL.alpha(a));
            }
            let text: String = self.clips[i].text.split_whitespace().collect::<Vec<_>>().join(" ");
            let is_link = text.starts_with("http://") || text.starts_with("https://");
            icons::draw(g, if is_link { Icon::Next } else { Icon::List }, row.x + 16.0 * s, row.cy(), 11.0 * s, if is_link { WATER } else { TEXT3 }.alpha(a * ap));
            let ago = time::span((self.unix - self.clips[i].unix).max(60.0));
            let flash = self.copied_flash.0 == i && self.now - self.copied_flash.1 < 1.2;
            let right = if flash { "Скопійовано".to_string() } else { ago };
            let rw = fonts.regular.measure(&right, 11.0 * s);
            let t = fonts.regular.ellipsize(&text, 12.5 * s, row.w - rw - 74.0 * s);
            g.text(&fonts.regular, &t, 12.5 * s, row.x + 32.0 * s, row.cy() + 4.5 * s, TEXT.alpha(a * ap));
            let rx = if hot { row.r() - 36.0 * s - rw } else { row.r() - 10.0 * s - rw };
            g.text(&fonts.regular, &right, 11.0 * s, rx, row.cy() + 4.0 * s, if flash { GREEN } else { TEXT3 }.alpha(a));
            if hot {
                let xr = rect(row.r() - 30.0 * s, row.cy() - 11.0 * s, 22.0 * s, 22.0 * s);
                self.zone(xr, Hot::ClipX(i), live);
                if self.is_hot(Hot::ClipX(i)) {
                    g.fill_circle(xr.cx(), xr.cy(), 11.0 * s, ACCENT_DARK.alpha(a));
                }
                icons::draw(g, Icon::Close, xr.cx(), xr.cy(), 10.0 * s, TEXT2.alpha(a));
            }
        }
        g.set_clip_rect(None);
    }

    // ───────── Focus ─────────

    pub(super) fn draw_focus(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let (cx, cy, rr) = (r.x + 86.0 * s, r.y + 84.0 * s, 56.0 * s);
        let left = self.focus_left();
        let frac = (1.0 - left / self.mode_len()).clamp(0.0, 1.0) as f32;
        let col = if self.focus_mode == FocusMode::Focus { ACCENT } else { GREEN };
        g.stroke_circle(cx, cy, rr, rgba(255, 255, 255, (30.0 * a) as u8), 7.0 * s);
        g.arc(cx, cy, rr, 0.0, frac.max(0.0005) * std::f32::consts::TAU, col.alpha(a), 7.0 * s);
        if self.focus_running {
            let ang = frac * std::f32::consts::TAU;
            g.fill_circle(cx + rr * ang.sin(), cy - rr * ang.cos(), 5.5 * s, TEXT.alpha(a));
        }
        let t = lrc::fmt_time(left);
        let tw = fonts.bold.measure(&t, 26.0 * s);
        g.text(&fonts.bold, &t, 26.0 * s, cx - tw / 2.0, cy + 9.0 * s, TEXT.alpha(a));
        let x = r.x + 168.0 * s;
        let title = match self.focus_mode {
            FocusMode::Focus => "Фокус",
            FocusMode::Break => "Перерва",
            FocusMode::LongBreak => "Довга перерва",
        };
        g.text(&fonts.semibold, title, 17.0 * s, x, r.y + 42.0 * s, TEXT.alpha(a));
        let sub = format!("Раунд {} з 4  ·  сьогодні {}", self.focus_round % 4 + 1, self.focus_done);
        g.text(&fonts.regular, &sub, 12.0 * s, x, r.y + 62.0 * s, TEXT2.alpha(a));
        for i in 0..4 {
            let done = (i as u32) < self.focus_round % 4;
            let dx = x + 5.0 * s + i as f32 * 16.0 * s;
            g.fill_circle(dx, r.y + 80.0 * s, 4.5 * s, if done { col } else { rgba(255, 255, 255, 40) }.alpha(a));
        }
        let by = r.y + 104.0 * s;
        let main = rect(x, by, 124.0 * s, 36.0 * s);
        self.zone(main, Hot::FocusStart, live);
        chip(g, main, self.is_hot(Hot::FocusStart), true, a);
        let (ic, label) = if self.focus_running { (Icon::Pause, "Пауза") } else { (Icon::Play, if frac > 0.0 { "Далі" } else { "Почати" }) };
        icons::draw(g, ic, main.x + 22.0 * s, main.cy(), 13.0 * s, TEXT.alpha(a));
        g.text_in(&fonts.semibold, label, 13.5 * s, rect(main.x + 36.0 * s, main.y, main.w - 44.0 * s, main.h), Align::Left, TEXT.alpha(a));
        for (i, (hot, icon)) in [(Hot::FocusReset, Icon::Reset), (Hot::FocusSkip, Icon::Skip)].into_iter().enumerate() {
            let br = rect(main.r() + 10.0 * s + i as f32 * 46.0 * s, by, 36.0 * s, 36.0 * s);
            self.zone(br, hot, live);
            chip(g, br, self.is_hot(hot), false, a);
            icons::draw(g, icon, br.cx(), br.cy(), 15.0 * s, TEXT.alpha(a));
        }
        let hint = format!("{} хв фокус · {} хв перерва · {} хв довга", self.cfg.focus_min, self.cfg.break_min, self.cfg.long_break_min);
        g.text(&fonts.regular, &hint, 11.0 * s, x, by + 56.0 * s, TEXT3.alpha(a));
    }

    // ───────── Water ─────────

    pub(super) fn draw_water(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let gr = rect(r.x + 32.0 * s, r.y + 24.0 * s, 74.0 * s, 110.0 * s);
        let total = self.water_ml();
        let lvl = (total as f32 / self.cfg.goal_ml.max(1) as f32).min(1.0);
        let glass = {
            let mut pb = tiny_skia::PathBuilder::new();
            pb.move_to(gr.x, gr.y);
            pb.line_to(gr.r(), gr.y);
            pb.line_to(gr.r() - 9.0 * s, gr.b() - 8.0 * s);
            pb.quad_to(gr.r() - 10.0 * s, gr.b(), gr.r() - 18.0 * s, gr.b());
            pb.line_to(gr.x + 18.0 * s, gr.b());
            pb.quad_to(gr.x + 10.0 * s, gr.b(), gr.x + 9.0 * s, gr.b() - 8.0 * s);
            pb.close();
            pb.finish()
        };
        if let Some(gp) = &glass {
            g.fill_path(gp, rgba(255, 255, 255, (14.0 * a) as u8));
            if lvl > 0.0 {
                let wy = gr.b() - (gr.h - 6.0 * s) * lvl;
                let t = self.now as f32;
                let mut pb = tiny_skia::PathBuilder::new();
                pb.move_to(gr.x - 2.0, gr.b() + 2.0);
                let n = 24;
                for i in 0..=n {
                    let xx = gr.x - 2.0 + (gr.w + 4.0) * i as f32 / n as f32;
                    let yy = wy + (t * 2.4 + i as f32 * 0.55).sin() * 2.2 * s;
                    pb.line_to(xx, yy);
                }
                pb.line_to(gr.r() + 2.0, gr.b() + 2.0);
                pb.close();
                if let Some(wp) = pb.finish() {
                    // clip water to the glass via a mask
                    let mut mask = tiny_skia::Mask::new(g.w(), g.h()).unwrap();
                    mask.fill_path(gp, tiny_skia::FillRule::Winding, true, tiny_skia::Transform::identity());
                    let mut paint = tiny_skia::Paint::default();
                    if let Some(sh) = tiny_skia::LinearGradient::new(
                        tiny_skia::Point::from_xy(0.0, wy),
                        tiny_skia::Point::from_xy(0.0, gr.b()),
                        vec![
                            tiny_skia::GradientStop::new(0.0, rgba(110, 190, 255, (230.0 * a) as u8).sk()),
                            tiny_skia::GradientStop::new(1.0, rgba(40, 110, 230, (240.0 * a) as u8).sk()),
                        ],
                        tiny_skia::SpreadMode::Pad,
                        tiny_skia::Transform::identity(),
                    ) {
                        paint.shader = sh;
                    }
                    paint.anti_alias = true;
                    g.pm.fill_path(&wp, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), Some(&mask));
                }
            }
            g.stroke_path(gp, rgba(255, 255, 255, (90.0 * a) as u8), 1.6 * s);
        }
        let x = r.x + 134.0 * s;
        let big = format!("{}", total);
        g.text(&fonts.bold, &big, 26.0 * s, x, r.y + 50.0 * s, TEXT.alpha(a));
        let bw = fonts.bold.measure(&big, 26.0 * s);
        g.text(&fonts.regular, &format!("/ {} мл", self.cfg.goal_ml), 13.0 * s, x + bw + 6.0 * s, r.y + 50.0 * s, TEXT2.alpha(a));
        let sub = if !self.cfg.water_on {
            "Нагадування вимкнено".to_string()
        } else if total >= self.cfg.goal_ml {
            "Норму виконано — так тримати!".to_string()
        } else {
            format!("Нагадаю через {}", time::span((self.water_due - self.now).max(0.0)))
        };
        g.text(&fonts.regular, &sub, 12.0 * s, x, r.y + 72.0 * s, TEXT2.alpha(a));
        let by = r.y + 92.0 * s;
        let drink = rect(x, by, 136.0 * s, 36.0 * s);
        let en = self.cfg.water_on;
        self.zone(drink, Hot::WaterDrink, live && en);
        if en {
            g.fill_rrect(drink, drink.h / 2.0, if self.is_hot(Hot::WaterDrink) { rgba(84, 170, 255, 255) } else { WATER }.alpha(a));
        } else {
            g.fill_rrect(drink, drink.h / 2.0, CONTROL.alpha(a));
        }
        icons::draw(g, Icon::Drop, drink.x + 20.0 * s, drink.cy(), 14.0 * s, if en { TEXT } else { TEXT3 }.alpha(a));
        let dl = format!("+{} мл", self.cfg.cup_ml);
        g.text_in(&fonts.semibold, &dl, 13.5 * s, rect(drink.x + 34.0 * s, drink.y, 96.0 * s, drink.h), Align::Left, if en { TEXT } else { TEXT3 }.alpha(a));
        let undo = rect(drink.r() + 8.0 * s, by, 36.0 * s, 36.0 * s);
        self.zone(undo, Hot::WaterUndo, live && !self.drinks.is_empty());
        chip(g, undo, self.is_hot(Hot::WaterUndo), false, a);
        icons::draw(g, Icon::Minus, undo.cx(), undo.cy(), 13.0 * s, if self.drinks.is_empty() { TEXT3 } else { TEXT }.alpha(a));
        let tl = if en { format!("Кожні {} хв", self.cfg.water_interval) } else { "Нагадування вимк.".to_string() };
        let tw = fonts.regular.measure(&tl, 11.5 * s);
        let tg = rect(x, by + 46.0 * s, tw + 42.0 * s, 26.0 * s);
        self.zone(tg, Hot::WaterToggle, live);
        chip(g, tg, self.is_hot(Hot::WaterToggle), false, a);
        let sw = rect(tg.x + 8.0 * s, tg.cy() - 6.0 * s, 20.0 * s, 12.0 * s);
        g.fill_rrect(sw, 6.0 * s, if en { WATER } else { rgba(90, 90, 96, 255) }.alpha(a));
        g.fill_circle(if en { sw.r() - 6.0 * s } else { sw.x + 6.0 * s }, sw.cy(), 4.5 * s, TEXT.alpha(a));
        g.text_in(&fonts.regular, &tl, 11.5 * s, rect(tg.x + 34.0 * s, tg.y, tw, tg.h), Align::Left, TEXT2.alpha(a));
        // hourly chart
        let cr = rect(r.x + 336.0 * s, r.y + 34.0 * s, r.r() - 18.0 * s - (r.x + 336.0 * s), r.h - 34.0 * s - 64.0 * s);
        g.text(&fonts.regular, "По годинах", 11.0 * s, cr.x, cr.y - 10.0 * s, TEXT3.alpha(a));
        let hours: Vec<u32> = (6..24).collect();
        let mut per = vec![0u32; 24];
        for (h, ml) in &self.drinks {
            per[(*h as usize).min(23)] += ml;
        }
        let maxv = per.iter().copied().max().unwrap_or(0).max(self.cfg.cup_ml * 2) as f32;
        let n = hours.len() as f32;
        let bw = cr.w / n;
        let cur_h = self.lt.hour();
        for (k, h) in hours.iter().enumerate() {
            let v = per[*h as usize] as f32 / maxv;
            let bx = cr.x + k as f32 * bw;
            let bh = (cr.h * v).max(2.0 * s);
            let col = if *h == cur_h { WATER } else if v > 0.0 { rgba(80, 150, 230, 255) } else { rgba(255, 255, 255, 30) };
            g.fill_rrect(rect(bx + bw * 0.18, cr.b() - bh, bw * 0.64, bh), bw * 0.25, col.alpha(a));
            if *h % 6 == 0 || *h == 23 {
                g.text_center(&fonts.regular, &format!("{}", h), 9.5 * s, bx + bw / 2.0, cr.b() + 13.0 * s, TEXT3.alpha(a));
            }
        }
        let cups = self.drinks.len();
        g.text(&fonts.regular, &format!("{} чашок сьогодні", cups), 11.0 * s, cr.x, cr.b() + 32.0 * s, TEXT2.alpha(a));
    }

    // ───────── To-do (with week calendar and reminders) ─────────

    pub(super) fn draw_todo(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let pad = 18.0 * s;
        let day = self.todo_day;
        let title = if day == self.lt.days {
            "Справи на сьогодні".to_string()
        } else if day == self.lt.days + 1 {
            "Справи на завтра".to_string()
        } else {
            format!("Справи · {}", time::short_label(day))
        };
        g.text(&fonts.semibold, &title, 15.0 * s, r.x + pad, r.y + pad + 12.0 * s, TEXT.alpha(a));
        let list_idx = self.day_todos();
        let done = list_idx.iter().filter(|&&i| self.todos[i].done).count();
        let total = list_idx.len();
        if total > 0 {
            let c = format!("{}/{}", done, total);
            let cw = fonts.semibold.measure(&c, 12.5 * s);
            g.text(&fonts.semibold, &c, 12.5 * s, r.r() - pad - cw, r.y + pad + 12.0 * s, if done == total { GREEN } else { TEXT2 }.alpha(a));
        }
        // week strip
        let ws = time::week_start(day);
        let sy = r.y + 46.0 * s;
        let arrow_w = 22.0 * s;
        let strip = rect(r.x + pad + arrow_w, sy, r.w - 2.0 * pad - 2.0 * arrow_w, 34.0 * s);
        let pr = rect(r.x + pad - 4.0 * s, sy + 5.0 * s, 24.0 * s, 24.0 * s);
        let nr = rect(strip.r() + 2.0 * s, sy + 5.0 * s, 24.0 * s, 24.0 * s);
        self.zone(pr, Hot::WeekPrev, live);
        self.zone(nr, Hot::WeekNext, live);
        for (rr, hot, flip) in [(pr, Hot::WeekPrev, true), (nr, Hot::WeekNext, false)] {
            if self.is_hot(hot) {
                g.fill_circle(rr.cx(), rr.cy(), 12.0 * s, CONTROL_HOVER.alpha(a));
            }
            let d = if flip { -1.0 } else { 1.0 };
            g.line(rr.cx() - d * 3.0 * s, rr.cy() - 5.0 * s, rr.cx() + d * 3.0 * s, rr.cy(), TEXT2.alpha(a), 1.6 * s);
            g.line(rr.cx() + d * 3.0 * s, rr.cy(), rr.cx() - d * 3.0 * s, rr.cy() + 5.0 * s, TEXT2.alpha(a), 1.6 * s);
        }
        let cw = strip.w / 7.0;
        for k in 0..7i64 {
            let dday = ws + k;
            let cr = rect(strip.x + k as f32 * cw + 2.0 * s, strip.y, cw - 4.0 * s, strip.h);
            self.zone(cr, Hot::Day(k), live);
            let sel = dday == day;
            let today = dday == self.lt.days;
            if sel {
                g.fill_rrect(cr, 10.0 * s, ACCENT_DARK.alpha(a));
            } else if self.is_hot(Hot::Day(k)) {
                g.fill_rrect(cr, 10.0 * s, CONTROL_HOVER.alpha(a));
            } else if today {
                g.stroke_rrect(cr, 10.0 * s, ACCENT.alpha(a * 0.8), 1.2 * s);
            }
            let (_, _, dn) = time::civil_from_days(dday);
            let wd = time::DAYS_SHORT[time::weekday(dday) as usize];
            g.text_center(&fonts.regular, wd, 9.5 * s, cr.cx(), cr.y + 13.0 * s, if sel { TEXT } else { TEXT3 }.alpha(a));
            g.text_center(&fonts.semibold, &format!("{}", dn), 12.0 * s, cr.cx(), cr.y + 27.0 * s, TEXT.alpha(a));
            if self.todos.iter().any(|t| t.day == dday && !t.done && !t.removing) && !sel {
                g.fill_circle(cr.r() - 6.0 * s, cr.y + 6.0 * s, 2.5 * s, ACCENT.alpha(a));
            }
        }
        // list
        let list = rect(r.x + pad - 6.0 * s, r.y + 88.0 * s, r.w - 2.0 * pad + 12.0 * s, r.h - 88.0 * s - 72.0 * s);
        g.set_clip_rect(Some(list));
        let mut y = list.y - self.todo_scroll.v;
        if total == 0 {
            g.text(&fonts.regular, "Порожньо. Додай справу ↓  (порада: «Стрім @20:00» — з нагадуванням)", 12.0 * s, list.x + 6.0 * s, list.y + 20.0 * s, TEXT3.alpha(a));
        }
        let dragging = self.reorder.map(|(i, _)| i);
        for &i in &list_idx {
            let (ap, ck, dn, off) = (self.todos[i].appear.v, self.todos[i].check.v, self.todos[i].done, self.todos[i].offset.v);
            let rh = 30.0 * s * ap;
            let mut row = rect(list.x, y + off, list.w, rh);
            y += rh;
            if dragging == Some(i) {
                let (_, sy0) = self.reorder.unwrap();
                let cur_y = self.cursor_local.1 + self.win_y as f32;
                row.y = (y - rh) + (cur_y - sy0);
            }
            if rh < 2.0 || row.b() < list.y || row.y > list.b() {
                continue;
            }
            let ra = a * ap;
            let hot_row = matches!(self.hot, Some(Hot::TodoRow(j)) | Some(Hot::TodoDel(j)) | Some(Hot::TodoBell(j)) if j == i);
            if dragging == Some(i) {
                g.fill_rrect(row.inflate(0.0, -1.0), 9.0 * s, rgba(40, 40, 46, (240.0 * a) as u8));
                g.stroke_rrect(row.inflate(0.0, -1.0), 9.0 * s, ACCENT.alpha(a), 1.2 * s);
            } else if hot_row {
                g.fill_rrect(row.inflate(0.0, -1.0), 9.0 * s, CONTROL.alpha(ra));
            }
            self.zone(row, Hot::TodoRow(i), live);
            let cb = (row.x + 16.0 * s, row.cy());
            g.stroke_circle(cb.0, cb.1, 8.0 * s, rgba(255, 255, 255, 110).mix(GREEN, ck).alpha(ra), 1.5 * s);
            if ck > 0.01 {
                g.fill_circle(cb.0, cb.1, 8.0 * s * ck, GREEN.alpha(ra));
                icons::draw(g, Icon::Check, cb.0, cb.1, 11.0 * s, TEXT.alpha(ra * ck));
            }
            let remind = self.todos[i].remind;
            let chip_w = if remind.is_some() { 56.0 * s } else { 0.0 };
            let txt = fonts.regular.ellipsize(&self.todos[i].text, 13.0 * s, row.w - 104.0 * s - chip_w);
            let tx = row.x + 34.0 * s;
            let base = row.cy() + fonts.regular.cap_height(13.0 * s) / 2.0;
            let tw = g.text(&fonts.regular, &txt, 13.0 * s, tx, base, if dn { TEXT3 } else { TEXT }.alpha(ra));
            if ck > 0.01 {
                g.line(tx, row.cy(), tx + tw * ck, row.cy(), TEXT3.alpha(ra), 1.2 * s);
            }
            let mut rx = row.r() - 8.0 * s;
            if hot_row && dragging.is_none() {
                for (hot, icon) in [(Hot::TodoDel(i), Icon::Trash), (Hot::TodoBell(i), Icon::Bell)] {
                    let xr = rect(rx - 22.0 * s, row.cy() - 11.0 * s, 22.0 * s, 22.0 * s);
                    self.zone(xr, hot, live);
                    if self.is_hot(hot) {
                        g.fill_circle(xr.cx(), xr.cy(), 11.0 * s, if matches!(hot, Hot::TodoDel(_)) { ACCENT_DARK } else { CONTROL_HOVER }.alpha(ra));
                    }
                    icons::draw(g, icon, xr.cx(), xr.cy(), 12.0 * s, TEXT2.alpha(ra));
                    rx = xr.x - 4.0 * s;
                }
            }
            if let Some(m) = remind {
                let label = format!("{:02}:{:02}", m / 60, m % 60);
                let lw = fonts.semibold.measure(&label, 11.0 * s);
                let cr = rect(rx - lw - 24.0 * s, row.cy() - 10.0 * s, lw + 22.0 * s, 20.0 * s);
                let fired = self.todos[i].fired;
                g.fill_rrect(cr, 10.0 * s, if fired { rgba(255, 255, 255, 18) } else { ACCENT_DARK.with_a(200) }.alpha(ra));
                icons::draw(g, Icon::Bell, cr.x + 10.0 * s, cr.cy(), 9.0 * s, TEXT.alpha(ra));
                g.text(&fonts.semibold, &label, 11.0 * s, cr.x + 18.0 * s, cr.cy() + 4.0 * s, TEXT.alpha(ra));
            }
        }
        g.set_clip_rect(None);
        // input
        let ir = rect(r.x + pad, r.b() - 66.0 * s, r.w - 2.0 * pad, 34.0 * s);
        self.zone(ir, Hot::TodoInput, live);
        let focused = self.todo_field.is_some();
        g.fill_rrect(ir, 11.0 * s, if focused { rgba(255, 255, 255, 26) } else if self.is_hot(Hot::TodoInput) { CONTROL_HOVER } else { CONTROL }.alpha(a));
        if focused {
            g.stroke_rrect(ir, 11.0 * s, ACCENT.alpha(a), 1.4 * s);
        }
        icons::draw(g, Icon::Plus, ir.x + 18.0 * s, ir.cy(), 12.0 * s, if focused { ACCENT } else { TEXT2 }.alpha(a));
        let tx = ir.x + 34.0 * s;
        let size = 13.0 * s;
        let base = ir.cy() + fonts.regular.cap_height(size) / 2.0;
        match &self.todo_field {
            Some(f) => {
                let txt = f.string();
                g.text(&fonts.regular, &txt, size, tx, base, TEXT.alpha(a));
                if f.caret_visible(self.now) {
                    let cx = tx + fonts.regular.measure(&f.before_caret(), size);
                    g.fill_rect(rect(cx, ir.cy() - 8.0 * s, 1.4 * s, 16.0 * s), TEXT.alpha(a));
                }
                if txt.is_empty() {
                    g.text(&fonts.regular, "Що треба зробити?  @18:30 — нагадати", size, tx + 4.0 * s, base, TEXT3.alpha(a));
                }
            }
            None => {
                let t = if self.todo_draft.is_empty() { "Додати справу…".to_string() } else { self.todo_draft.clone() };
                g.text(&fonts.regular, &t, size, tx, base, TEXT3.alpha(a));
            }
        }
    }

    fn cycle_reminder(&mut self, i: usize) {
        let now_min = self.lt.minutes();
        let today = self.lt.days;
        let Some(t) = self.todos.get_mut(i) else { return };
        let presets: Vec<u32> = if t.day == today {
            let r5 = |m: u32| ((m + 4) / 5 * 5).min(23 * 60 + 55);
            vec![r5(now_min + 30), r5(now_min + 60), r5(now_min + 180)]
        } else {
            vec![9 * 60, 12 * 60, 18 * 60, 20 * 60]
        };
        let next = match t.remind {
            None => Some(presets[0]),
            Some(cur) => presets.iter().position(|&p| p == cur).and_then(|k| presets.get(k + 1).copied()),
        };
        t.remind = next;
        t.fired = false;
        self.actions.push(IslandAction::SaveData);
    }

    // ───────── Widgets ─────────

    pub(super) fn draw_widgets(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let pad = 18.0 * s;
        let gap = 12.0 * s;
        let cw = (r.w - 2.0 * pad - gap) / 2.0;
        let ch = r.h - pad - 34.0 * s;
        for k in 0..2u8 {
            let cr = rect(r.x + pad + k as f32 * (cw + gap), r.y + pad, cw, ch);
            self.zone(cr, Hot::Widget(k), live);
            let hot = self.is_hot(Hot::Widget(k));
            g.fill_rrect(cr, 18.0 * s, rgba(255, 255, 255, if hot { 20 } else { 12 }).alpha(a));
            let kind = if k == 0 { self.cfg.widget_a } else { self.cfg.widget_b } as usize % WIDGETS.len();
            self.draw_widget(g, kind, cr, a, live, k);
            if hot {
                let t = "клік — інший віджет";
                g.text_in(&self.fonts.regular, t, 10.0 * s, rect(cr.x, cr.b() - 18.0 * s, cr.w - 12.0 * s, 14.0 * s), Align::Right, TEXT3.alpha(a));
            }
        }
    }

    fn draw_widget(&mut self, g: &mut Canvas, kind: usize, cr: RectF, a: f32, live: bool, slot: u8) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let p = 14.0 * s;
        g.text(&fonts.regular, WIDGETS[kind], 11.0 * s, cr.x + p, cr.y + p + 6.0 * s, TEXT3.alpha(a));
        let body = rect(cr.x + p, cr.y + p + 16.0 * s, cr.w - 2.0 * p, cr.h - 2.0 * p - 16.0 * s);
        match kind {
            0 => {
                // week calendar
                let ws = time::week_start(self.lt.days);
                let (_, m, _) = time::civil_from_days(self.lt.days);
                g.text(&fonts.semibold, time::MONTHS[(m - 1) as usize], 15.0 * s, body.x, body.y + 16.0 * s, TEXT.alpha(a));
                let cw = body.w / 7.0;
                for k in 0..7i64 {
                    let d = ws + k;
                    let cx = body.x + cw * (k as f32 + 0.5);
                    let (_, _, dn) = time::civil_from_days(d);
                    g.text_center(&fonts.regular, time::DAYS_SHORT[time::weekday(d) as usize], 10.5 * s, cx, body.y + 46.0 * s, TEXT3.alpha(a));
                    if d == self.lt.days {
                        g.fill_circle(cx, body.y + 66.0 * s, 13.0 * s, ACCENT.alpha(a));
                    }
                    g.text_center(&fonts.semibold, &format!("{}", dn), 13.0 * s, cx, body.y + 71.0 * s, TEXT.alpha(a));
                    let n = self.todos.iter().filter(|t| t.day == d && !t.done && !t.removing).count();
                    for j in 0..n.min(3) {
                        g.fill_circle(cx - 5.0 * s + j as f32 * 5.0 * s, body.y + 90.0 * s, 1.8 * s, TEXT2.alpha(a));
                    }
                }
            }
            1 => {
                // world clock
                let c = &time::CITIES[self.cfg.world_city as usize % time::CITIES.len()];
                let off = time::city_offset_min(c, self.unix);
                let lt = time::local(self.unix, off);
                let (cx, cy, rr) = (body.x + 46.0 * s, body.cy() + 4.0 * s, 40.0 * s);
                let night = !(7..19).contains(&lt.hour());
                g.fill_circle(cx, cy, rr, if night { rgba(30, 32, 44, 255) } else { rgba(236, 236, 236, 255) }.alpha(a));
                let fg = if night { TEXT } else { rgba(20, 20, 22, 255) };
                for k in 0..12 {
                    let an = k as f32 * std::f32::consts::TAU / 12.0;
                    g.line(cx + an.sin() * rr * 0.82, cy - an.cos() * rr * 0.82, cx + an.sin() * rr * 0.92, cy - an.cos() * rr * 0.92, fg.alpha(a * 0.6), 1.4 * s);
                }
                let hm = lt.secs as f32 / 3600.0;
                let ha = (hm % 12.0) / 12.0 * std::f32::consts::TAU;
                let ma = (lt.secs as f32 / 60.0 % 60.0) / 60.0 * std::f32::consts::TAU;
                g.line(cx, cy, cx + ha.sin() * rr * 0.5, cy - ha.cos() * rr * 0.5, fg.alpha(a), 3.0 * s);
                g.line(cx, cy, cx + ma.sin() * rr * 0.74, cy - ma.cos() * rr * 0.74, fg.alpha(a), 2.0 * s);
                g.fill_circle(cx, cy, 3.0 * s, ACCENT.alpha(a));
                let tx = cx + rr + 18.0 * s;
                g.text(&fonts.semibold, c.name, 14.0 * s, tx, cy - 14.0 * s, TEXT.alpha(a));
                g.text(&fonts.bold, &lt.hm(), 24.0 * s, tx, cy + 14.0 * s, TEXT.alpha(a));
                let diff = (off - self.tz_min) as f32 / 60.0;
                let d = if diff == 0.0 { "як у тебе".to_string() } else { format!("{:+} год", diff) };
                g.text(&fonts.regular, &d, 11.5 * s, tx, cy + 32.0 * s, TEXT2.alpha(a));
            }
            2 => {
                // moon phase
                let ph = time::moon_phase(self.unix);
                let (cx, cy, rr) = (body.x + 46.0 * s, body.cy() + 4.0 * s, 38.0 * s);
                g.fill_circle(cx, cy, rr, rgba(50, 52, 60, 255).alpha(a));
                if let Some(path) = moon_lit(cx, cy, rr, ph as f32) {
                    g.fill_path(&path, rgba(244, 238, 220, 255).alpha(a));
                }
                g.stroke_circle(cx, cy, rr, rgba(255, 255, 255, 40).alpha(a), 1.0 * s);
                let illum = (1.0 - (ph * std::f64::consts::TAU).cos()) / 2.0;
                let tx = cx + rr + 18.0 * s;
                g.text(&fonts.semibold, time::moon_name(ph), 14.0 * s, tx, cy - 6.0 * s, TEXT.alpha(a));
                g.text(&fonts.regular, &format!("Освітлено {:.0}%", illum * 100.0), 12.0 * s, tx, cy + 14.0 * s, TEXT2.alpha(a));
                let age = ph * 29.53;
                g.text(&fonts.regular, &format!("{:.0}-й день циклу", age.floor() + 1.0), 11.0 * s, tx, cy + 32.0 * s, TEXT3.alpha(a));
            }
            3 => {
                // countdown
                let target = time::parse_iso(&self.cfg.countdown_date).unwrap_or(self.lt.days);
                let days = target - self.lt.days;
                let title = fonts.semibold.ellipsize(&self.cfg.countdown_title, 14.0 * s, body.w);
                g.text(&fonts.semibold, &title, 14.0 * s, body.x, body.y + 16.0 * s, TEXT.alpha(a));
                let big = format!("{}", days.max(0));
                g.text(&fonts.bold, &big, 44.0 * s, body.x, body.y + 70.0 * s, ACCENT.mix(TEXT, 0.2).alpha(a));
                let bw = fonts.bold.measure(&big, 44.0 * s);
                let word = if days == 1 { "день" } else if (2..=4).contains(&(days % 10)) && !(12..=14).contains(&(days % 100)) { "дні" } else { "днів" };
                g.text(&fonts.regular, word, 14.0 * s, body.x + bw + 8.0 * s, body.y + 70.0 * s, TEXT2.alpha(a));
                g.text(&fonts.regular, &time::short_label(target), 11.5 * s, body.x, body.y + 92.0 * s, TEXT3.alpha(a));
            }
            4 | 5 | 6 => {
                let (frac, col, big, sub) = match kind {
                    4 => match self.battery {
                        Some(b) => (
                            b.percent as f32 / 100.0,
                            if b.plugged { GREEN } else if b.percent <= 20 { ACCENT } else { TEXT },
                            format!("{}%", b.percent),
                            if b.charging { "Заряджається".to_string() } else if b.plugged { "Від мережі".into() } else { "Від батареї".into() },
                        ),
                        None => (1.0, TEXT3, "—".into(), "Немає батареї".into()),
                    },
                    5 => (
                        (self.water_ml() as f32 / self.cfg.goal_ml.max(1) as f32).min(1.0),
                        WATER,
                        format!("{} мл", self.water_ml()),
                        format!("з {} мл", self.cfg.goal_ml),
                    ),
                    _ => (
                        (1.0 - self.focus_left() / self.mode_len()).clamp(0.0, 1.0) as f32,
                        if self.focus_mode == FocusMode::Focus { ACCENT } else { GREEN },
                        lrc::fmt_time(self.focus_left()),
                        if self.focus_running { "Іде фокус".to_string() } else { "Таймер на паузі".into() },
                    ),
                };
                let (cx, cy, rr) = (body.x + 44.0 * s, body.cy() + 4.0 * s, 36.0 * s);
                g.stroke_circle(cx, cy, rr, rgba(255, 255, 255, (30.0 * a) as u8), 7.0 * s);
                g.arc(cx, cy, rr, 0.0, frac.max(0.001) * std::f32::consts::TAU, col.alpha(a), 7.0 * s);
                match kind {
                    4 => bolt(g, cx, cy, 12.0 * s, col.alpha(a)),
                    5 => icons::draw(g, Icon::Drop, cx, cy, 22.0 * s, col.alpha(a)),
                    _ => icons::draw(g, Icon::Timer, cx, cy, 22.0 * s, col.alpha(a)),
                }
                let tx = cx + rr + 20.0 * s;
                g.text(&fonts.bold, &big, 22.0 * s, tx, cy + 2.0 * s, TEXT.alpha(a));
                g.text(&fonts.regular, &sub, 12.0 * s, tx, cy + 22.0 * s, TEXT2.alpha(a));
                if kind == 6 {
                    let br = rect(tx, cy + 32.0 * s, 92.0 * s, 26.0 * s);
                    self.zone(br, Hot::WidgetGo(slot), live);
                    chip(g, br, self.is_hot(Hot::WidgetGo(slot)), true, a);
                    let lbl = if self.focus_running { "Пауза" } else { "Почати" };
                    g.text_in(&fonts.semibold, lbl, 12.0 * s, br, Align::Center, TEXT.alpha(a));
                } else if kind == 5 && self.cfg.water_on {
                    let br = rect(tx, cy + 32.0 * s, 92.0 * s, 26.0 * s);
                    self.zone(br, Hot::WidgetGo(slot), live);
                    g.fill_rrect(br, br.h / 2.0, if self.is_hot(Hot::WidgetGo(slot)) { rgba(84, 170, 255, 255) } else { WATER }.alpha(a));
                    g.text_in(&fonts.semibold, &format!("+{} мл", self.cfg.cup_ml), 12.0 * s, br, Align::Center, TEXT.alpha(a));
                }
            }
            _ => {}
        }
    }

    // ───────── Prayer times ─────────

    pub(super) fn draw_prayer(&mut self, g: &mut Canvas, r: RectF, a: f32, _live: bool) {
        let s = self.s;
        let fonts = self.fonts.clone();
        let pad = 18.0 * s;
        let Some(t) = self.prayer_times() else { return };
        let now_h = self.lt.secs / 3600.0;
        // sun arc
        let ar = rect(r.x + pad + 6.0 * s, r.y + 34.0 * s, 280.0 * s, 120.0 * s);
        let base_y = ar.b();
        let sx = |h: f64| -> f32 {
            let k = ((h - t[1]) / (t[4] - t[1])).clamp(-0.08, 1.08) as f32;
            ar.x + ar.w * k
        };
        let arc_y = |x: f32| -> f32 {
            let u = ((x - ar.x) / ar.w) * 2.0 - 1.0;
            base_y - ar.h * (1.0 - u * u).max(0.0).sqrt()
        };
        // horizon
        g.line(ar.x - 10.0 * s, base_y, ar.r() + 10.0 * s, base_y, rgba(255, 255, 255, (50.0 * a) as u8), 1.0 * s);
        let mut pb = tiny_skia::PathBuilder::new();
        let n = 48;
        for i in 0..=n {
            let x = ar.x + ar.w * i as f32 / n as f32;
            if i == 0 {
                pb.move_to(x, arc_y(x))
            } else {
                pb.line_to(x, arc_y(x))
            }
        }
        if let Some(p) = pb.finish() {
            g.stroke_path(&p, rgba(255, 200, 90, (110.0 * a) as u8), 1.6 * s);
        }
        // travelled part
        if now_h > t[1] {
            let xe = sx(now_h.min(t[4]));
            let mut pb = tiny_skia::PathBuilder::new();
            let steps = 40;
            for i in 0..=steps {
                let x = ar.x + (xe - ar.x) * i as f32 / steps as f32;
                if i == 0 {
                    pb.move_to(x, arc_y(x))
                } else {
                    pb.line_to(x, arc_y(x))
                }
            }
            if let Some(p) = pb.finish() {
                g.stroke_path_cap(&p, rgba(255, 196, 70, (255.0 * a) as u8), 3.0 * s, tiny_skia::LineCap::Round);
            }
        }
        for (i, name) in prayer::NAMES.iter().enumerate() {
            if i == 0 || i == 5 {
                continue;
            }
            let x = sx(t[i]);
            let y = arc_y(x.clamp(ar.x, ar.r()));
            g.fill_circle(x, y, 3.5 * s, rgba(255, 255, 255, (200.0 * a) as u8));
            let lbl = fonts.regular.ellipsize(name, 10.0 * s, 60.0 * s);
            g.text_center(&fonts.regular, &lbl, 10.0 * s, x, y - 9.0 * s, TEXT3.alpha(a));
        }
        if now_h >= t[1] && now_h <= t[4] {
            let x = sx(now_h);
            let y = arc_y(x);
            g.radial(x, y, 20.0 * s, rgba(255, 196, 70, (120.0 * a) as u8), rgba(255, 196, 70, 0));
            g.fill_circle(x, y, 8.0 * s, rgba(255, 210, 90, (255.0 * a) as u8));
        } else {
            let x = if now_h < t[1] { ar.x - 2.0 * s } else { ar.r() + 2.0 * s };
            if let Some(p) = moon_lit(x, base_y - 16.0 * s, 8.0 * s, 0.3) {
                g.fill_path(&p, rgba(230, 230, 240, (230.0 * a) as u8));
            }
        }
        let city = self.prayer_city_name();
        let method = prayer::METHODS.get(self.cfg.prayer_method as usize).map(|m| m.name).unwrap_or("");
        g.text(&fonts.semibold, &city, 13.0 * s, r.x + pad, r.y + pad + 4.0 * s, TEXT.alpha(a));
        let cw = fonts.semibold.measure(&city, 13.0 * s);
        g.text(&fonts.regular, &format!("· {}", method), 11.0 * s, r.x + pad + cw + 6.0 * s, r.y + pad + 4.0 * s, TEXT3.alpha(a));
        // list
        let lx = r.x + 330.0 * s;
        let lw = r.r() - pad - lx;
        let next = [0usize, 2, 3, 4, 5].into_iter().find(|&i| t[i] > now_h);
        let head = match next {
            Some(i) => format!("{} через {}", prayer::NAMES[i], time::span((t[i] - now_h) * 3600.0)),
            None => "Фаджр — завтра".to_string(),
        };
        g.text(&fonts.semibold, &head, 13.0 * s, lx, r.y + pad + 4.0 * s, ACCENT.mix(TEXT, 0.3).alpha(a));
        let mut y = r.y + 34.0 * s;
        for i in 0..6 {
            let row = rect(lx - 8.0 * s, y, lw + 16.0 * s, 25.0 * s);
            if Some(i) == next {
                g.fill_rrect(row, 10.0 * s, ACCENT_DARK.alpha(a));
            }
            let past = t[i] < now_h && Some(i) != next;
            let col = if past { TEXT3 } else { TEXT };
            g.text(&fonts.regular, prayer::NAMES[i], 12.5 * s, lx, row.cy() + 4.5 * s, col.alpha(a));
            let tt = prayer::fmt(t[i]);
            let tw = fonts.semibold.measure(&tt, 12.5 * s);
            g.text(&fonts.semibold, &tt, 12.5 * s, lx + lw - tw, row.cy() + 4.5 * s, col.alpha(a));
            y += 26.0 * s;
        }
    }

    // ───────── page dots / icon bar ─────────

    pub(super) fn draw_dots(&mut self, g: &mut Canvas, r: RectF, a: f32, live: bool) {
        let s = self.s;
        let k = self.dots_open.v;
        let pages = self.pages();
        let n = pages.len() as f32;
        let cur = pages.iter().position(|p| *p == self.page).unwrap_or(0);
        let cy = r.b() - 15.0 * s;
        let dw = lerp(14.0, 32.0, k) * s;
        let total = dw * n;
        let x0 = r.cx() - total / 2.0;
        let bar = rect(x0 - 6.0 * s * k, cy - lerp(5.0, 15.0, k) * s, total + 12.0 * s * k, lerp(10.0, 30.0, k) * s);
        self.zone(bar.inflate(10.0 * s, 4.0 * s), Hot::Dots, live);
        if k > 0.01 {
            g.fill_rrect(bar, bar.h / 2.0, rgba(255, 255, 255, (22.0 * a * k) as u8));
        }
        for (i, p) in pages.iter().enumerate() {
            let cx = x0 + dw * (i as f32 + 0.5);
            let zr = rect(cx - dw / 2.0, bar.y, dw, bar.h);
            self.zone(zr, Hot::Page(i), live);
            let active = i == cur;
            let hot = self.is_hot(Hot::Page(i));
            if k < 0.99 {
                let da = a * (1.0 - k);
                let w = if active { 16.0 * s } else { 6.0 * s };
                g.fill_rrect(rect(cx - w / 2.0, cy - 3.0 * s, w, 6.0 * s), 3.0 * s, if active { TEXT } else { rgba(255, 255, 255, 80) }.alpha(da));
            }
            if k > 0.01 {
                let ia = a * k;
                if active {
                    g.fill_rrect(rect(cx - 14.0 * s, cy - 12.0 * s, 28.0 * s, 24.0 * s), 12.0 * s, ACCENT_DARK.alpha(ia));
                } else if hot {
                    g.fill_rrect(rect(cx - 14.0 * s, cy - 12.0 * s, 28.0 * s, 24.0 * s), 12.0 * s, CONTROL_HOVER.alpha(ia));
                }
                if *p == Page::Prayer {
                    if let Some(path) = moon_lit(cx, cy, 6.5 * s, 0.85) {
                        g.fill_path(&path, if active || hot { TEXT } else { TEXT2 }.alpha(ia));
                    }
                } else if *p == Page::Widgets {
                    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                        g.fill_rrect(rect(cx + dx * 4.0 * s - 3.0 * s, cy + dy * 4.0 * s - 3.0 * s, 6.0 * s, 6.0 * s), 1.5 * s, if active || hot { TEXT } else { TEXT2 }.alpha(ia));
                    }
                } else {
                    icons::draw(g, p.icon(), cx, cy, 14.0 * s, if active || hot { TEXT } else { TEXT2 }.alpha(ia));
                }
                if hot {
                    let label = p.title();
                    let tw = self.fonts.regular.measure(label, 11.0 * s);
                    let lr = rect(cx - tw / 2.0 - 8.0 * s, bar.y - 24.0 * s, tw + 16.0 * s, 19.0 * s);
                    g.fill_rrect(lr, lr.h / 2.0, PILL_BG.alpha(ia));
                    g.stroke_rrect(lr, lr.h / 2.0, RIM.alpha(ia), s.max(1.0));
                    g.text_in(&self.fonts.regular, label, 11.0 * s, lr, Align::Center, TEXT.alpha(ia));
                }
            }
        }
    }

    // ───────── clicks ─────────

    pub(super) fn click(&mut self, h: Hot) {
        match h {
            Hot::PlayPause => {
                self.media.position = self.media.pos_at(self.now);
                self.media.sampled_at = self.now;
                self.media.playing = !self.media.playing;
                self.actions.push(IslandAction::MediaToggle);
            }
            Hot::Next => self.actions.push(IslandAction::MediaNext),
            Hot::Prev => self.actions.push(IslandAction::MediaPrev),
            Hot::Art => {
                if self.media.players.len() > 1 {
                    self.actions.push(IslandAction::NextPlayer);
                }
            }
            Hot::LyricsBtn => self.lyrics_open = !self.lyrics_open,
            Hot::Line(i) => {
                if let Lyrics::Synced(lines) = &self.lyrics {
                    if let Some(l) = lines.get(i) {
                        let t = l.t;
                        self.media.position = t;
                        self.media.sampled_at = self.now;
                        self.actions.push(IslandAction::MediaSeek(t));
                    }
                }
            }
            Hot::VolIcon => {
                self.muted = !self.muted;
                self.vol_flash = self.now;
                self.actions.push(IslandAction::ToggleMute);
            }
            Hot::Page(i) => {
                let pages = self.pages();
                if let Some(p) = pages.get(i) {
                    self.page = *p;
                }
                // the new page may be shorter than where the cursor is: don't collapse under it
                self.hold_until = self.now + 1.2;
            }
            Hot::Tab(t) => self.shelf_tab = t,
            Hot::TileX(i) => {
                if i < self.shelf.len() {
                    self.shelf.remove(i);
                    self.actions.push(IslandAction::SaveData);
                }
            }
            Hot::ShelfSelect => {
                self.shelf_select = !self.shelf_select;
                if !self.shelf_select {
                    for it in &mut self.shelf {
                        it.selected = false;
                    }
                }
            }
            Hot::ShelfRemove => {
                self.shelf.retain(|x| !x.selected);
                self.shelf_select = false;
                self.actions.push(IslandAction::SaveData);
            }
            Hot::ShelfClear => {
                if self.shelf_tab == 1 {
                    self.clips.clear();
                } else {
                    self.shelf.clear();
                    self.shelf_select = false;
                    self.actions.push(IslandAction::SaveData);
                }
            }
            Hot::Clip(i) => {
                if let Some(c) = self.clips.get(i) {
                    self.actions.push(IslandAction::CopyText(c.text.clone()));
                    self.copied_flash = (i, self.now);
                }
            }
            Hot::ClipX(i) => {
                if i < self.clips.len() {
                    self.clips.remove(i);
                }
            }
            Hot::FocusStart => self.focus_toggle(),
            Hot::WidgetGo(_) => {
                if self.widget_is(h, 6) {
                    self.focus_toggle()
                } else {
                    self.drink()
                }
            }
            Hot::FocusReset => {
                self.focus_running = false;
                self.focus_remaining = self.mode_len();
            }
            Hot::FocusSkip => {
                self.focus_running = true;
                self.focus_ends = self.now;
                self.tick_timers(self.now);
                self.notices.retain(|n| !matches!(n.kind, NoticeKind::FocusDone | NoticeKind::BreakDone));
            }
            Hot::WaterDrink => self.drink(),
            Hot::WaterUndo => {
                self.drinks.pop();
                self.actions.push(IslandAction::SaveData);
            }
            Hot::WaterToggle => {
                self.cfg.water_on = !self.cfg.water_on;
                self.water_due = self.now + self.cfg.water_interval as f64 * 60.0;
                self.actions.push(IslandAction::SaveSettings);
            }
            Hot::TodoRow(i) => {
                if let Some(t) = self.todos.get_mut(i) {
                    t.done = !t.done;
                    if t.done {
                        self.happy_until = self.now + 1.5;
                        let txt = t.text.clone();
                        self.log(Icon::Check, format!("Зроблено: {}", txt));
                    }
                    self.actions.push(IslandAction::SaveData);
                }
            }
            Hot::TodoDel(i) => {
                if let Some(t) = self.todos.get_mut(i) {
                    t.removing = true;
                    self.actions.push(IslandAction::SaveData);
                }
            }
            Hot::TodoBell(i) => self.cycle_reminder(i),
            Hot::TodoInput => {
                if self.todo_field.is_none() {
                    let mut f = TextField::new(&self.todo_draft, false);
                    f.last_input = self.now;
                    self.todo_field = Some(f);
                    self.actions.push(IslandAction::WantKeyboard(true));
                }
            }
            Hot::Day(k) => {
                self.todo_day = time::week_start(self.todo_day) + k;
                self.todo_scroll_raw = 0.0;
            }
            Hot::WeekPrev => self.todo_day -= 7,
            Hot::WeekNext => self.todo_day += 7,
            Hot::Widget(k) => {
                let n = WIDGETS.len() as u8;
                if k == 0 {
                    self.cfg.widget_a = (self.cfg.widget_a + 1) % n;
                } else {
                    self.cfg.widget_b = (self.cfg.widget_b + 1) % n;
                }
                self.actions.push(IslandAction::SaveSettings);
            }
            Hot::NoticeA => {
                if let Some(n) = self.notices.first() {
                    match n.kind {
                        NoticeKind::Water => self.drink(),
                        NoticeKind::BreakDone => {
                            self.focus_mode = FocusMode::Focus;
                            self.focus_remaining = self.mode_len();
                            self.focus_toggle();
                        }
                        NoticeKind::Todo => {
                            let txt = n.text.clone();
                            if let Some(t) = self.todos.iter_mut().find(|t| t.text == txt && !t.done) {
                                t.done = true;
                            }
                            self.actions.push(IslandAction::SaveData);
                        }
                        _ => {}
                    }
                }
                if !self.notices.is_empty() {
                    self.notices.remove(0);
                }
            }
            Hot::NoticeB => {
                if let Some(n) = self.notices.first() {
                    if n.kind == NoticeKind::Water {
                        self.water_due = self.now + 10.0 * 60.0;
                    }
                }
                if !self.notices.is_empty() {
                    self.notices.remove(0);
                }
            }
            Hot::Seek | Hot::Volume | Hot::Dots | Hot::Tile(_) | Hot::Panel => {}
        }
    }

    fn widget_is(&self, h: Hot, kind: u8) -> bool {
        match h {
            Hot::WidgetGo(0) => self.cfg.widget_a % WIDGETS.len() as u8 == kind,
            Hot::WidgetGo(_) => self.cfg.widget_b % WIDGETS.len() as u8 == kind,
            _ => false,
        }
    }
}

/// Lit part of the moon for phase p (0 new → 0.5 full → 1 new).
pub(crate) fn moon_lit(cx: f32, cy: f32, r: f32, p: f32) -> Option<tiny_skia::Path> {
    const K: f32 = 0.552_284_8;
    let c = (p * std::f32::consts::TAU).cos();
    let mut pb = tiny_skia::PathBuilder::new();
    if p < 0.5 {
        // right half lit + terminator
        pb.move_to(cx, cy - r);
        pb.cubic_to(cx + r * K, cy - r, cx + r, cy - r * K, cx + r, cy);
        pb.cubic_to(cx + r, cy + r * K, cx + r * K, cy + r, cx, cy + r);
        let e = r * c;
        pb.cubic_to(cx + e * K, cy + r, cx + e, cy + r * K, cx + e, cy);
        pb.cubic_to(cx + e, cy - r * K, cx + e * K, cy - r, cx, cy - r);
    } else {
        pb.move_to(cx, cy - r);
        pb.cubic_to(cx - r * K, cy - r, cx - r, cy - r * K, cx - r, cy);
        pb.cubic_to(cx - r, cy + r * K, cx - r * K, cy + r, cx, cy + r);
        let e = -r * c;
        pb.cubic_to(cx + e * K, cy + r, cx + e, cy + r * K, cx + e, cy);
        pb.cubic_to(cx + e, cy - r * K, cx + e * K, cy - r, cx, cy - r);
    }
    pb.close();
    pb.finish()
}
