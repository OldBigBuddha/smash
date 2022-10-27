use crossterm::cursor;
use crossterm::event::{Event as TermEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::{execute, queue};
use std::cmp::min;
use std::io::Write;
use std::time::Duration;
use tracing::debug;

use crate::shell::Shell;

pub struct SmashState {
    shell: Shell,
    columns: usize,
    lines: usize,
    prompt_len: usize,
    input: UserInput,
    clear_above: usize,
    clear_below: usize,
}

#[derive(Clone, Debug)]
struct UserInput {
    input: String,
    cursor: usize,
    indices: Vec<usize>,
    word_split: &'static str,
}

impl UserInput {
    pub fn new() -> Self {
        Self {
            input: String::with_capacity(256),
            cursor: 0,
            indices: Vec::with_capacity(256),
            word_split: " \t/",
        }
    }

    pub fn len(&self) -> usize {
        self.input.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn nth(&self, index: usize) -> Option<char> {
        self.input.chars().nth(index)
    }

    pub fn as_str(&self) -> &str {
        self.input.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    fn byte_index(&self) -> usize {
        if self.cursor == self.indices.len() {
            self.input.len()
        } else {
            self.indices[self.cursor]
        }
    }

    fn update_indices(&mut self) {
        self.indices.clear();
        for index in self.input.char_indices() {
            self.indices.push(index.0);
        }
    }

    pub fn insert(&mut self, ch: char) {
        self.input.insert(self.byte_index(), ch);
        self.update_indices();
        self.cursor += 1;
    }

    pub fn delete(&mut self) {
        if self.cursor < self.len() {
            self.input.remove(self.byte_index());
            self.update_indices();
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.byte_index());
            self.update_indices();
        }
    }

    pub fn clear(&mut self) {
        self.cursor = 0;
        self.input.clear();
        self.indices.clear();
    }

    pub fn move_by(&mut self, offset: isize) {
        if offset < 0 {
            self.cursor = self.cursor.saturating_sub(offset.unsigned_abs());
        } else {
            self.cursor = min(self.len(), self.cursor + offset.unsigned_abs());
        }
    }

    pub fn move_to_begin(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor = self.len();
    }

    pub fn move_to_prev_word(&mut self) {
        // Skip the whitespace at the current position.
        self.cursor = self.cursor.saturating_sub(1);

        while self.cursor > 0 {
            if let Some(next_ch) = self.nth(self.cursor.saturating_sub(1)) {
                if let Some(ch) = self.nth(self.cursor) {
                    if self.word_split.contains(next_ch) && !self.word_split.contains(ch) {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }

            self.cursor -= 1;
        }
    }

    pub fn move_to_next_word(&mut self) {
        // Skip the whitespace at the current position.
        self.cursor += 1;
        if self.cursor > self.input.len() {
            self.cursor = self.input.len();
        }

        while self.cursor < self.input.len() {
            if let Some(prev_ch) = self.nth(self.cursor.saturating_sub(1)) {
                if let Some(ch) = self.nth(self.cursor) {
                    if self.word_split.contains(prev_ch) && !self.word_split.contains(ch) {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }

            self.cursor += 1;
        }
    }
}

impl Drop for SmashState {
    fn drop(&mut self) {
        disable_raw_mode().ok();
    }
}

impl SmashState {
    pub fn new(shell: Shell) -> Self {
        Self {
            shell,
            columns: 0,
            lines: 0,
            prompt_len: 0,
            input: UserInput::new(),
            clear_above: 0,
            clear_below: 0,
        }
    }

    fn run_command(&mut self) {
        self.print_user_input();

        execute!(std::io::stdout(), Print("\r\n")).ok();
        disable_raw_mode().ok();
        self.shell.run_script(self.input.as_str());
        enable_raw_mode().ok();

        self.input.clear();
        self.clear_above = 0;
        self.clear_below = 0;

        self.render_prompt();
        self.print_user_input();
    }

    pub fn render_prompt(&mut self) {
        let screen_size = terminal::size().unwrap();
        self.columns = screen_size.0 as usize;
        self.lines = screen_size.1 as usize;

        debug!(?self.columns);

        let mut stdout = std::io::stdout();
        queue!(
            stdout,
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Reverse),
            Print("$"),
            SetAttribute(Attribute::Reset),
            Print(&format!(
                "{space:>width$}\r",
                space = " ",
                width = self.columns - 1
            ))
        )
        .ok();

        let (mut prompt_str, mut prompt_len) = (String::new(), 0);
        prompt_str.push_str(" $ ");
        queue!(stdout, Print(prompt_str.replace('\n', "\r\n"))).ok();
        prompt_len += prompt_str.len();
        stdout.flush().unwrap();

        self.prompt_len = prompt_len;
    }

    fn print_user_input(&mut self) {
        if cfg!(test) {
            return;
        }

        let mut stdout = std::io::stdout();

        queue!(stdout, cursor::Hide).ok();

        queue!(
            stdout,
            Print("\r"),
            cursor::MoveRight(self.prompt_len as u16),
            Clear(ClearType::UntilNewLine),
            Print(self.input.input.replace('\n', "\r\n"))
        )
        .ok();

        let current_x = self.prompt_len + self.input.len();
        if current_x % self.columns == 0 {
            queue!(stdout, Print("\r\n")).ok();
        }

        let input_height = current_x / self.columns;

        let cursor_y = (self.prompt_len + self.input.cursor()) / self.columns;
        let cursor_x = (self.prompt_len + self.input.cursor()) % self.columns;
        let cursor_y_diff = input_height - cursor_y;
        if cursor_y_diff > 0 {
            queue!(stdout, cursor::MoveUp(cursor_y_diff as u16),).ok();
        }

        queue!(stdout, Print("\r")).ok();
        if cursor_x > 0 {
            queue!(stdout, cursor::MoveRight(cursor_x as u16),).ok();
        }

        queue!(stdout, cursor::Show).ok();

        self.clear_above = cursor_y;
        self.clear_below = input_height - cursor_y;
        stdout.flush().ok();
    }

    pub fn handle_key_event(&mut self, ev: &KeyEvent) {
        let mut needs_redraw = true;
        match (ev.code, ev.modifiers) {
            // cursor
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                let mut stdout = std::io::stdout();
                execute!(stdout, Print("\r\n")).ok();
                self.render_prompt();
                self.input.clear();
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.input.is_empty() {
                    unreachable!();
                } else {
                    self.input.delete();
                }
            }
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.input.move_to_begin();
            }
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.input.move_to_end();
            }
            (KeyCode::Char('f'), KeyModifiers::ALT) => {
                self.input.move_to_next_word();
            }
            (KeyCode::Char('b'), KeyModifiers::ALT) => {
                self.input.move_to_prev_word();
            }
            (KeyCode::Left, KeyModifiers::NONE) => {
                self.input.move_by(-1);
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                self.input.move_by(1);
            }
            // misc
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                self.input.backspace();
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                self.run_command();
                needs_redraw = false;
            }
            (KeyCode::Esc, KeyModifiers::NONE) => {
                disable_raw_mode().ok();
                std::process::exit(0);
            }
            (KeyCode::Char(ch), KeyModifiers::NONE) => {
                self.input.insert(ch);
                debug!(?self.input.input);
            }
            _ => (),
        }

        if needs_redraw {
            self.print_user_input();
        }
    }

    pub fn run(&mut self) {
        enable_raw_mode().ok();
        self.render_prompt();

        debug!("start");
        loop {
            match crossterm::event::poll(Duration::from_millis(100)) {
                Ok(true) => loop {
                    if let Ok(TermEvent::Key(ev)) = crossterm::event::read() {
                        self.handle_key_event(&ev)
                    }

                    match crossterm::event::poll(Duration::from_millis(0)) {
                        Ok(true) => (),
                        _ => break,
                    }
                },
                _ => (),
            }
        }
    }

    #[cfg(test)]
    fn input_str(&mut self, string: &str) {
        for ch in string.chars() {
            let code = match ch {
                '\n' => KeyCode::Enter,
                '\t' => KeyCode::Tab,
                _ => KeyCode::Char(ch),
            };

            self.handle_key_event(&KeyEvent::new(code, KeyModifiers::NONE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_smash_state() -> SmashState {
        let shell = Shell::new();
        SmashState::new(shell)
    }

    macro_rules! key_event {
        ($key:expr, $modifiers:expr) => {
            KeyEvent::new($key, $modifiers)
        };
    }

    #[test]
    fn move_by() {
        let mut m = create_smash_state();
        assert_eq!(m.input.cursor(), 0);
        m.handle_key_event(&key_event!(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 0);
        m.handle_key_event(&key_event!(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 0);

        m.input_str("abc x  123");
        assert_eq!(m.input.cursor(), 10);
        m.handle_key_event(&key_event!(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 9);
        m.handle_key_event(&key_event!(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 8);
        m.handle_key_event(&key_event!(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 7);
        m.handle_key_event(&key_event!(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 8);
        m.handle_key_event(&key_event!(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 9);
        m.handle_key_event(&key_event!(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 10);

        m.input.clear();
        m.input_str("     ");
        m.handle_key_event(&key_event!(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 5);
        m.handle_key_event(&key_event!(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(m.input.cursor(), 4);
    }

    #[test]
    fn move_by_word() {
        let mut m = create_smash_state();
        assert_eq!(m.input.cursor(), 0);
        m.handle_key_event(&key_event!(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 0);
        m.handle_key_event(&key_event!(KeyCode::Char('f'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 0);

        m.input_str("abc x  123");
        assert_eq!(m.input.cursor(), 10);
        m.handle_key_event(&key_event!(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 7);
        m.handle_key_event(&key_event!(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 4);
        m.handle_key_event(&key_event!(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 0);
        m.handle_key_event(&key_event!(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 0);
        m.handle_key_event(&key_event!(KeyCode::Char('f'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 4);
        m.handle_key_event(&key_event!(KeyCode::Char('f'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 7);
        m.handle_key_event(&key_event!(KeyCode::Char('f'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 10);

        m.input.clear();
        m.input_str("     ");
        m.handle_key_event(&key_event!(KeyCode::Char('f'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 5);
        m.handle_key_event(&key_event!(KeyCode::Char('b'), KeyModifiers::ALT));
        assert_eq!(m.input.cursor(), 0);
    }
}
