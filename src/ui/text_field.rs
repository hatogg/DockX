//! Minimal single-line text input (to-do entries, renaming dock icons).

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
    /// Ctrl+A
    SelectAll,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldResult {
    None,
    Submit(String),
    Cancel,
}

#[derive(Clone, Debug, Default)]
pub struct TextField {
    pub text: Vec<char>,
    pub caret: usize,
    /// whole text selected (typing replaces it) — enough for rename
    pub all_selected: bool,
    pub last_input: f64,
    pub max_len: usize,
}

impl TextField {
    pub fn new(initial: &str, select_all: bool) -> TextField {
        let text: Vec<char> = initial.chars().collect();
        TextField { caret: text.len(), text, all_selected: select_all && !initial.is_empty(), last_input: 0.0, max_len: 120 }
    }

    pub fn string(&self) -> String {
        self.text.iter().collect()
    }

    pub fn before_caret(&self) -> String {
        self.text[..self.caret].iter().collect()
    }

    pub fn paste(&mut self, s: &str, now: f64) {
        for ch in s.chars() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            self.key(Key::Char(ch), now);
        }
    }

    pub fn key(&mut self, k: Key, now: f64) -> FieldResult {
        self.last_input = now;
        if self.all_selected {
            match k {
                Key::Char(_) | Key::Backspace | Key::Delete => {
                    self.text.clear();
                    self.caret = 0;
                    self.all_selected = false;
                    if !matches!(k, Key::Char(_)) {
                        return FieldResult::None;
                    }
                }
                Key::Left | Key::Home => {
                    self.all_selected = false;
                    self.caret = 0;
                    return FieldResult::None;
                }
                Key::Right | Key::End => {
                    self.all_selected = false;
                    self.caret = self.text.len();
                    return FieldResult::None;
                }
                _ => {}
            }
        }
        match k {
            Key::Char(c) => {
                if !c.is_control() && self.text.len() < self.max_len {
                    self.text.insert(self.caret, c);
                    self.caret += 1;
                }
            }
            Key::Backspace => {
                if self.caret > 0 {
                    self.caret -= 1;
                    self.text.remove(self.caret);
                }
            }
            Key::Delete => {
                if self.caret < self.text.len() {
                    self.text.remove(self.caret);
                }
            }
            Key::Left => self.caret = self.caret.saturating_sub(1),
            Key::Right => self.caret = (self.caret + 1).min(self.text.len()),
            Key::Home => self.caret = 0,
            Key::End => self.caret = self.text.len(),
            Key::SelectAll => self.all_selected = !self.text.is_empty(),
            Key::Enter => return FieldResult::Submit(self.string().trim().to_string()),
            Key::Escape => return FieldResult::Cancel,
        }
        FieldResult::None
    }

    /// Caret blinks, but stays solid right after typing.
    pub fn caret_visible(&self, now: f64) -> bool {
        let t = now - self.last_input;
        t < 0.5 || (t * 1.9) as i64 % 2 == 0
    }
}
