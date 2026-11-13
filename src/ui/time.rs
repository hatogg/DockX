//! Calendar maths without any OS calls (works the same in the app and in the demo).

/// Days since 1970-01-01 for a civil date.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// 0 = Sunday … 6 = Saturday
pub fn weekday(days: i64) -> u32 {
    (((days % 7) + 7 + 4) % 7) as u32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local {
    pub days: i64,
    pub y: i32,
    pub m: u32,
    pub d: u32,
    pub weekday: u32,
    pub secs: f64,
}

impl Local {
    pub fn hour(&self) -> u32 {
        (self.secs / 3600.0) as u32
    }
    pub fn minute(&self) -> u32 {
        ((self.secs / 60.0) as u32) % 60
    }
    pub fn minutes(&self) -> u32 {
        (self.secs / 60.0) as u32
    }
    pub fn hm(&self) -> String {
        format!("{:02}:{:02}", self.hour(), self.minute())
    }
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

pub fn local(unix: f64, tz_min: i32) -> Local {
    let t = unix + tz_min as f64 * 60.0;
    let days = (t / 86400.0).floor() as i64;
    let (y, m, d) = civil_from_days(days);
    Local { days, y, m, d, weekday: weekday(days), secs: t - days as f64 * 86400.0 }
}

pub fn parse_iso(s: &str) -> Option<i64> {
    let mut it = s.trim().split('-');
    let y: i32 = it.next()?.trim().parse().ok()?;
    let m: u32 = it.next()?.trim().parse().ok()?;
    let d: u32 = it.next()?.trim().parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

pub fn iso_from_days(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

const MONTHS_GEN: [&str; 12] =
    ["січня", "лютого", "березня", "квітня", "травня", "червня", "липня", "серпня", "вересня", "жовтня", "листопада", "грудня"];
pub const MONTHS: [&str; 12] =
    ["Січень", "Лютий", "Березень", "Квітень", "Травень", "Червень", "Липень", "Серпень", "Вересень", "Жовтень", "Листопад", "Грудень"];
const DAYS: [&str; 7] = ["неділя", "понеділок", "вівторок", "середа", "четвер", "пʼятниця", "субота"];
pub const DAYS_SHORT: [&str; 7] = ["Нд", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"];

/// "пʼятниця, 13 листопада"
pub fn label(days: i64) -> String {
    let (_, m, d) = civil_from_days(days);
    format!("{}, {} {}", DAYS[weekday(days) as usize], d, MONTHS_GEN[(m - 1) as usize])
}

/// "13 листопада"
pub fn short_label(days: i64) -> String {
    let (_, m, d) = civil_from_days(days);
    format!("{} {}", d, MONTHS_GEN[(m - 1) as usize])
}

/// Monday of the week containing `days`.
pub fn week_start(days: i64) -> i64 {
    let wd = weekday(days) as i64; // 0 = Sun
    days - ((wd + 6) % 7)
}

fn last_sunday(y: i32, m: u32) -> i64 {
    let next = if m == 12 { days_from_civil(y + 1, 1, 1) } else { days_from_civil(y, m + 1, 1) };
    let last = next - 1;
    last - weekday(last) as i64
}

fn nth_sunday(y: i32, m: u32, n: i64) -> i64 {
    let first = days_from_civil(y, m, 1);
    let to_sun = (7 - weekday(first) as i64) % 7;
    first + to_sun + 7 * (n - 1)
}

/// EU summer time: last Sunday of March 01:00 UTC → last Sunday of October 01:00 UTC.
pub fn eu_dst(unix: f64) -> bool {
    let (y, _, _) = civil_from_days((unix / 86400.0).floor() as i64);
    let start = last_sunday(y, 3) as f64 * 86400.0 + 3600.0;
    let end = last_sunday(y, 10) as f64 * 86400.0 + 3600.0;
    unix >= start && unix < end
}

/// US daylight time: 2nd Sunday of March 02:00 local → 1st Sunday of November 02:00 local (approx. in UTC).
pub fn us_dst(unix: f64, std_offset_h: f64) -> bool {
    let (y, _, _) = civil_from_days((unix / 86400.0).floor() as i64);
    let start = nth_sunday(y, 3, 2) as f64 * 86400.0 + (2.0 - std_offset_h) * 3600.0;
    let end = nth_sunday(y, 11, 1) as f64 * 86400.0 + (1.0 - std_offset_h) * 3600.0;
    unix >= start && unix < end
}

pub struct City {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
    /// standard UTC offset, hours
    pub utc: f64,
    /// 0 none, 1 EU, 2 US
    pub dst: u8,
}

pub const CITIES: &[City] = &[
    City { name: "Київ", lat: 50.4501, lon: 30.5234, utc: 2.0, dst: 1 },
    City { name: "Ужгород", lat: 48.6208, lon: 22.2879, utc: 2.0, dst: 1 },
    City { name: "Мукачево", lat: 48.4393, lon: 22.7178, utc: 2.0, dst: 1 },
    City { name: "Львів", lat: 49.8397, lon: 24.0297, utc: 2.0, dst: 1 },
    City { name: "Одеса", lat: 46.4825, lon: 30.7233, utc: 2.0, dst: 1 },
    City { name: "Харків", lat: 49.9935, lon: 36.2304, utc: 2.0, dst: 1 },
    City { name: "Дніпро", lat: 48.4647, lon: 35.0462, utc: 2.0, dst: 1 },
    City { name: "Сімферополь", lat: 44.9521, lon: 34.1024, utc: 3.0, dst: 0 },
    City { name: "Варшава", lat: 52.2297, lon: 21.0122, utc: 1.0, dst: 1 },
    City { name: "Берлін", lat: 52.5200, lon: 13.4050, utc: 1.0, dst: 1 },
    City { name: "Лондон", lat: 51.5074, lon: -0.1278, utc: 0.0, dst: 1 },
    City { name: "Стамбул", lat: 41.0082, lon: 28.9784, utc: 3.0, dst: 0 },
    City { name: "Дубай", lat: 25.2048, lon: 55.2708, utc: 4.0, dst: 0 },
    City { name: "Мекка", lat: 21.3891, lon: 39.8579, utc: 3.0, dst: 0 },
    City { name: "Нью-Йорк", lat: 40.7128, lon: -74.0060, utc: -5.0, dst: 2 },
    City { name: "Лос-Анджелес", lat: 34.0522, lon: -118.2437, utc: -8.0, dst: 2 },
    City { name: "Токіо", lat: 35.6762, lon: 139.6503, utc: 9.0, dst: 0 },
];

/// Current UTC offset of a city, minutes.
pub fn city_offset_min(c: &City, unix: f64) -> i32 {
    let extra = match c.dst {
        1 if eu_dst(unix) => 1.0,
        2 if us_dst(unix, c.utc) => 1.0,
        _ => 0.0,
    };
    ((c.utc + extra) * 60.0) as i32
}

/// Moon age as a fraction of the synodic month (0 = new, 0.5 = full).
pub fn moon_phase(unix: f64) -> f64 {
    let synodic = 29.530588853 * 86400.0;
    let ref_new = 947182440.0; // 2000-01-06 18:14 UTC
    ((unix - ref_new) / synodic).rem_euclid(1.0)
}

pub fn moon_name(p: f64) -> &'static str {
    match (p * 8.0 + 0.5).floor() as i32 % 8 {
        0 => "Новий місяць",
        1 => "Молодий місяць",
        2 => "Перша чверть",
        3 => "Прибуваючий",
        4 => "Повня",
        5 => "Спадний",
        6 => "Остання чверть",
        _ => "Старий місяць",
    }
}

/// "1 год 12 хв", "45 хв", "3 дн"
pub fn span(secs: f64) -> String {
    let m = (secs / 60.0).ceil().max(0.0) as i64;
    if m >= 48 * 60 {
        format!("{} дн", m / (24 * 60))
    } else if m >= 60 {
        if m % 60 == 0 {
            format!("{} год", m / 60)
        } else {
            format!("{} год {} хв", m / 60, m % 60)
        }
    } else {
        format!("{} хв", m)
    }
}

/// Parses "18:30" / "9:05" / "18.30".
pub fn parse_hm(s: &str) -> Option<u32> {
    let s = s.trim().replace('.', ":");
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.trim().parse().ok()?;
    let m: u32 = m.trim().parse().ok()?;
    if h < 24 && m < 60 {
        Some(h * 60 + m)
    } else {
        None
    }
}

/// Pulls a trailing reminder out of a to-do text: "Стрім @20:00", "Стрім о 20:00".
pub fn split_reminder(text: &str) -> (String, Option<u32>) {
    let t = text.trim();
    for marker in ["@", " о ", " в ", " at "] {
        if let Some(i) = t.rfind(marker) {
            let tail = &t[i + marker.len()..];
            if let Some(min) = parse_hm(tail) {
                let head = t[..i].trim().to_string();
                if !head.is_empty() {
                    return (head, Some(min));
                }
            }
        }
    }
    (t.to_string(), None)
}
