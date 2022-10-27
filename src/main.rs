use crossterm::tty::IsTty;
use tracing_subscriber::{self, fmt, prelude::*, EnvFilter};

use event::SmashState;
use shell::Shell;
use variable::Value;

#[macro_use]
mod macros;

mod builtins;
mod eval;
mod event;
mod expand;
mod parser;
mod path;
mod process;
mod shell;
mod variable;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let mut shell = Shell::new();

    for (key, value) in std::env::vars() {
        shell.set(&key, Value::String(value.to_owned()), false);
    }

    let is_tty = std::io::stdout().is_tty();
    shell.set_interactive(is_tty);
    SmashState::new(shell).run();
}
