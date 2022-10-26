use crossterm::event::{Event as TermEvent, KeyCode, KeyModifiers};
use crossterm::queue;
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode};
use std::io::Write;
use std::time::Duration;
use tracing::debug;

pub struct SmashState {
    columns: usize,
    lines: usize,
    prompt_len: usize,
    input: UserInput,
}

#[derive(Clone, Debug)]
struct UserInput {
    input: String,
}

impl UserInput {
    pub fn new() -> Self {
        Self {
            input: String::with_capacity(256),
        }
    }
}

impl Drop for SmashState {
    fn drop(&mut self) {
        disable_raw_mode().ok();
    }
}

impl SmashState {
    pub fn new() -> Self {
        Self {
            columns: 0,
            lines: 0,
            prompt_len: 0,
            input: UserInput::new(),
        }
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

    pub fn run(&mut self) {
        enable_raw_mode().ok();
        self.render_prompt();

        debug!("start");
        'main: loop {
            match crossterm::event::poll(Duration::from_millis(100)) {
                Ok(true) => loop {
                    if let Ok(TermEvent::Key(ev)) = crossterm::event::read() {
                        match (ev.code, ev.modifiers) {
                            (KeyCode::Char(ch), KeyModifiers::NONE) => {
                                self.input.input.push(ch);
                                debug!(?self.input.input);
                            }
                            (KeyCode::Esc, KeyModifiers::NONE) => break 'main,
                            _ => (),
                        }
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
}
