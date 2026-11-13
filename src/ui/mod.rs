//! Platform-independent UI: drawing, the dock, the island. Shared by the Windows app and the demo renderer.

pub mod anim;
pub mod dock;
pub mod gfx;
pub mod icons;
pub mod island;
pub mod lrc;
pub mod mascot;
pub mod prayer;
pub mod settings;
pub mod text_field;
pub mod theme;
pub mod time;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Btn {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
}

const MONTHS: [&str; 12] = [
    "січня", "лютого", "березня", "квітня", "травня", "червня", "липня", "серпня", "вересня", "жовтня", "листопада", "грудня",
];
const DAYS: [&str; 7] = ["неділя", "понеділок", "вівторок", "середа", "четвер", "пʼятниця", "субота"];

/// "пʼятниця, 13 листопада"; `weekday` 0 = Sunday.
pub fn uk_date(month: u32, day: u32, weekday: u32) -> String {
    format!("{}, {} {}", DAYS[(weekday % 7) as usize], day, MONTHS[((month + 11) % 12) as usize])
}

/// "2026-11-13"
pub fn iso_date(year: u32, month: u32, day: u32) -> String {
    format!("{:04}-{:02}-{:02}", year, month, day)
}
