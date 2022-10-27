use crate::parser;
use crate::process::ExitStatus;
use tracing::debug;

pub struct Shell;

impl Shell {
    pub fn new() -> Self {
        Self
    }

    pub fn run_script(&mut self, script: &str) -> ExitStatus {
        match parser::parse(script) {
            Ok(ast) => {
                debug!(?ast);
                ExitStatus::ExitedWith(0)
            }
            Err(parser::ParseError::Empty) => {
                // Just ignore.
                ExitStatus::ExitedWith(0)
            }
            Err(parser::ParseError::Fatal(err)) => {
                debug!("parse error: {}", err);
                ExitStatus::ExitedWith(-1)
            }
        }
    }
}
