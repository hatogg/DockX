//! Synced lyrics (LRC) parsing and lookup.

#[derive(Clone, Debug)]
pub struct LrcLine {
    pub t: f64,
    pub text: String,
}

/// Parses "[mm:ss.xx] text" lines (several time tags per line allowed). Empty lines become "♪".
pub fn parse(src: &str) -> Vec<LrcLine> {
    let mut out = Vec::new();
    let mut offset = 0.0f64;
    for raw in src.lines() {
        let mut line = raw.trim();
        let mut times = Vec::new();
        while line.starts_with('[') {
            let Some(end) = line.find(']') else { break };
            let tag = &line[1..end];
            line = line[end + 1..].trim_start();
            if let Some(v) = tag.strip_prefix("offset:") {
                if let Ok(ms) = v.trim().parse::<f64>() {
                    offset = ms / 1000.0;
                }
                continue;
            }
            if let Some(t) = parse_time(tag) {
                times.push(t);
            }
        }
        let text = if line.is_empty() { "♪".to_string() } else { line.to_string() };
        for t in times {
            out.push(LrcLine { t: (t - offset).max(0.0), text: text.clone() });
        }
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn parse_time(tag: &str) -> Option<f64> {
    let (m, rest) = tag.split_once(':')?;
    let m: f64 = m.trim().parse().ok()?;
    let s: f64 = rest.trim().replace(',', ".").parse().ok()?;
    Some(m * 60.0 + s)
}

/// Index of the line being sung at `t` (or -1 before the first line), plus progress 0..1 to the next line.
pub fn current(lines: &[LrcLine], t: f64) -> (i32, f32) {
    if lines.is_empty() || t < lines[0].t {
        return (-1, 0.0);
    }
    let mut lo = 0usize;
    let mut hi = lines.len();
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if lines[mid].t <= t {
            lo = mid
        } else {
            hi = mid
        }
    }
    let next_t = lines.get(lo + 1).map(|l| l.t).unwrap_or(lines[lo].t + 4.0);
    let p = ((t - lines[lo].t) / (next_t - lines[lo].t).max(0.01)).clamp(0.0, 1.0) as f32;
    (lo as i32, p)
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0).round() as i64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Percent-encodes a query value (UTF-8).
pub fn url_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            b' ' => o.push_str("%20"),
            _ => o.push_str(&format!("%{:02X}", b)),
        }
    }
    o
}

/// Strips " - Remastered 2011", "(Official Video)" etc. for better lyrics matches.
pub fn clean_title(t: &str) -> String {
    let mut s = t.to_string();
    for open in ['(', '['] {
        if let Some(i) = s.find(open) {
            let low = s[i..].to_lowercase();
            if ["official", "video", "audio", "lyric", "remaster", "live", "hd", "4k", "mv", "visualizer"]
                .iter()
                .any(|k| low.contains(k))
            {
                s.truncate(i);
            }
        }
    }
    if let Some(i) = s.to_lowercase().find(" - remaster") {
        s.truncate(i);
    }
    s.trim().to_string()
}

/// "Artist - Title" from YouTube-style titles when the artist is a channel name.
pub fn split_artist_title(title: &str) -> Option<(String, String)> {
    for sep in [" - ", " – ", " — "] {
        if let Some((a, t)) = title.split_once(sep) {
            if !a.trim().is_empty() && !t.trim().is_empty() {
                return Some((a.trim().to_string(), clean_title(t)));
            }
        }
    }
    None
}
