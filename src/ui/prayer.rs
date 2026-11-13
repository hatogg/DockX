//! Prayer times (the well-known PrayTimes algorithm), computed locally.

fn dsin(d: f64) -> f64 {
    d.to_radians().sin()
}
fn dcos(d: f64) -> f64 {
    d.to_radians().cos()
}
fn dtan(d: f64) -> f64 {
    d.to_radians().tan()
}
fn darcsin(x: f64) -> f64 {
    x.asin().to_degrees()
}
fn darccos(x: f64) -> f64 {
    x.acos().to_degrees()
}
fn darctan2(y: f64, x: f64) -> f64 {
    y.atan2(x).to_degrees()
}
fn darccot(x: f64) -> f64 {
    (1.0 / x).atan().to_degrees()
}
fn fix(a: f64, b: f64) -> f64 {
    a.rem_euclid(b)
}

pub struct Method {
    pub name: &'static str,
    pub fajr: f64,
    /// isha angle, or minutes after maghrib when `isha_min` is set
    pub isha: f64,
    pub isha_min: bool,
}

pub const METHODS: &[Method] = &[
    Method { name: "Мусульманська світова ліга", fajr: 18.0, isha: 17.0, isha_min: false },
    Method { name: "ISNA (Пн. Америка)", fajr: 15.0, isha: 15.0, isha_min: false },
    Method { name: "Єгипет", fajr: 19.5, isha: 17.5, isha_min: false },
    Method { name: "Умм аль-Кура (Мекка)", fajr: 18.5, isha: 90.0, isha_min: true },
    Method { name: "Diyanet (Туреччина)", fajr: 18.0, isha: 17.0, isha_min: false },
];

pub const NAMES: [&str; 6] = ["Фаджр", "Схід сонця", "Зухр", "Аср", "Магриб", "Іша"];

fn julian(y: i32, m: u32, d: u32) -> f64 {
    let (mut y, mut m) = (y as f64, m as f64);
    if m <= 2.0 {
        y -= 1.0;
        m += 12.0;
    }
    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + d as f64 + b - 1524.5
}

fn sun_position(jd: f64) -> (f64, f64) {
    let d = jd - 2451545.0;
    let g = fix(357.529 + 0.98560028 * d, 360.0);
    let q = fix(280.459 + 0.98564736 * d, 360.0);
    let l = fix(q + 1.915 * dsin(g) + 0.020 * dsin(2.0 * g), 360.0);
    let e = 23.439 - 0.00000036 * d;
    let ra = fix(darctan2(dcos(e) * dsin(l), dcos(l)) / 15.0, 24.0);
    let eqt = q / 15.0 - ra;
    let decl = darcsin(dsin(e) * dsin(l));
    (decl, eqt)
}

/// Times of the day in local hours: [fajr, sunrise, dhuhr, asr, maghrib, isha].
/// `tz` = UTC offset in hours; `hanafi` doubles the Asr shadow.
pub fn times(y: i32, m: u32, d: u32, lat: f64, lon: f64, tz: f64, method: &Method, hanafi: bool) -> [f64; 6] {
    let jd = julian(y, m, d) - lon / (15.0 * 24.0);
    let mid_day = |t: f64| {
        let (_, eqt) = sun_position(jd + t);
        fix(12.0 - eqt, 24.0)
    };
    let angle_time = |angle: f64, t: f64, ccw: bool| {
        let (decl, _) = sun_position(jd + t);
        let noon = mid_day(t);
        let v = (-dsin(angle) - dsin(decl) * dsin(lat)) / (dcos(decl) * dcos(lat));
        let tt = darccos(v.clamp(-1.0, 1.0)) / 15.0;
        let nan = !(-1.0..=1.0).contains(&v);
        (noon + if ccw { -tt } else { tt }, nan)
    };
    let asr_time = |factor: f64, t: f64| {
        let (decl, _) = sun_position(jd + t);
        let angle = -darccot(factor + dtan((lat - decl).abs()));
        angle_time(angle, t, false).0
    };
    // two passes from default guesses (day fractions)
    let mut tms = [5.0, 6.0, 12.0, 13.0, 18.0, 18.0];
    let mut nan = [false; 6];
    for _ in 0..2 {
        let p = tms.map(|h| h / 24.0);
        let (f, fnan) = angle_time(method.fajr, p[0], true);
        let (sr, _) = angle_time(0.833, p[1], true);
        let dh = mid_day(p[2]);
        let asr = asr_time(if hanafi { 2.0 } else { 1.0 }, p[3]);
        let (ss, _) = angle_time(0.833, p[4], false);
        let (isha, inan) = if method.isha_min { (ss + method.isha / 60.0, false) } else { angle_time(method.isha, p[5], false) };
        tms = [f, sr, dh, asr, ss, isha];
        nan = [fnan, false, false, false, false, inan && !method.isha_min];
    }
    // high latitudes: angle-based night portion
    let night = fix(tms[1] - tms[4], 24.0);
    let fajr_max = tms[1] - method.fajr / 60.0 * night;
    if nan[0] || tms[0] < fajr_max {
        tms[0] = fajr_max;
    }
    if !method.isha_min {
        let isha_max = tms[4] + method.isha / 60.0 * night;
        if nan[5] || tms[5] > isha_max {
            tms[5] = isha_max;
        }
    }
    let adj = tz - lon / 15.0;
    tms.map(|h| fix(h + adj, 24.0))
}

pub fn fmt(h: f64) -> String {
    let m = (h * 60.0).round() as i64;
    format!("{:02}:{:02}", (m / 60) % 24, m % 60)
}

#[cfg(test)]
mod calc_tests {
use crate::ui::{prayer, time};
#[test]
fn kyiv_prayers() {
    let t = prayer::times(2026, 11, 13, 50.4501, 30.5234, 2.0, &prayer::METHODS[0], false);
    let f: Vec<String> = t.iter().map(|h| prayer::fmt(*h)).collect();
    println!("Kyiv 2026-11-13: {:?}", f);
    let t = prayer::times(2026, 6, 21, 50.4501, 30.5234, 3.0, &prayer::METHODS[0], false);
    println!("Kyiv 2026-06-21: {:?}", t.iter().map(|h| prayer::fmt(*h)).collect::<Vec<_>>());
    let t = prayer::times(2026, 11, 13, 21.3891, 39.8579, 3.0, &prayer::METHODS[3], false);
    println!("Mecca 2026-11-13: {:?}", t.iter().map(|h| prayer::fmt(*h)).collect::<Vec<_>>());
}
#[test]
fn dates() {
    let d = time::days_from_civil(2026, 11, 13);
    assert_eq!(time::civil_from_days(d), (2026, 11, 13));
    assert_eq!(time::weekday(d), 5);
    println!("{}", time::label(d));
    let unix = d as f64 * 86400.0;
    println!("moon {:.3} {}", time::moon_phase(unix), time::moon_name(time::moon_phase(unix)));
    println!("eu dst nov {} jul {}", time::eu_dst(unix), time::eu_dst(unix - 120.0 * 86400.0));
    println!("{:?}", time::split_reminder("Стрім @20:00"));
    println!("{:?}", time::split_reminder("Купити хліб о 18.30"));
}
}
