use crate::process::ExitStatus;
use crate::shell::Shell;

use thiserror::Error;

mod cd;
mod exit;

pub trait BuiltinCommand {
    fn run(&self, ctx: &mut BuiltinCommandContext) -> ExitStatus;
}

pub struct BuiltinCommandContext<'a> {
    pub argv: &'a [String],
    pub shell: &'a mut Shell,
}

#[derive(Debug, Error)]
pub enum BuiltinCommandError {
    #[error("command not found")]
    NotFound,
}

pub fn builtin_command(name: &str) -> Option<Box<dyn BuiltinCommand>> {
    match name {
        "exit" => Some(Box::new(exit::Exit)),
        "cd" => Some(Box::new(cd::Cd)),
        _ => None,
    }
}
