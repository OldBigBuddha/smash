use tracing_subscriber::{self, fmt, prelude::*, EnvFilter};

use event::SmashState;

mod event;

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    SmashState::new().run();
}
