use tracing_subscriber::{self, fmt, prelude::*, EnvFilter};

use event::SmashState;
use shell::Shell;

mod event;
mod parser;
mod process;
mod shell;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let shell = Shell::new();
    SmashState::new(shell).run();
}
