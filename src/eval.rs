use crate::builtins::BuiltinCommandError;
use crate::expand::expand_words;
use crate::parser::{self, Ast, RunIf, Term};
use crate::process::{
    run_external_command, run_in_foreground, run_internal_command, wait_for_job, Context,
    ExitStatus, ProcessState,
};
use crate::shell::Shell;

use nix::unistd::{close, pipe, setpgid};
use tracing::debug;

pub fn eval(shell: &mut Shell, ast: &Ast) -> ExitStatus {
    debug!("ast: {:#?}", ast);
    run_terms(shell, &ast.terms)
}

pub fn run_terms(shell: &mut Shell, terms: &[Term]) -> ExitStatus {
    let mut last_status = ExitStatus::ExitedWith(0);
    for term in terms {
        for pipeline in &term.pipelines {
            match (last_status, &pipeline.run_if) {
                (ExitStatus::ExitedWith(0), RunIf::Success) => (),
                (ExitStatus::ExitedWith(_), RunIf::Failure) => (),
                (_, RunIf::Always) => (),
                _ => continue,
            }

            last_status = run_pipeline(shell, &term.code, pipeline, term.background);
        }
    }

    last_status
}

fn run_pipeline(
    shell: &mut Shell,
    code: &str,
    pipeline: &parser::Pipeline,
    background: bool,
) -> ExitStatus {
    // Invoke commands in a pipeline.
    let mut last_result = None;
    let mut iter = pipeline.commands.iter().peekable();
    let mut childs = Vec::new();
    let mut pgid = None;
    while let Some(command) = iter.next() {
        let pipes = if iter.peek().is_some() {
            // There is a next command in the pipeline (e.g. date in
            // `date | hexdump`). Create and connect a pipe.
            let (pipe_out, pipe_in) = pipe().expect("failed to create a pipe");
            Some((pipe_out, pipe_in))
        } else {
            // The last command in the pipeline.
            None
        };

        let result = run_command(
            shell,
            command,
            &Context {
                pgid,
                background,
                interactive: shell.interactive(),
            },
        );

        if let Some((_, pipe_in)) = pipes {
            // `pipe_in` is used by a child process and is no longer needed.
            close(pipe_in).expect("failed to close pipe_in");
        }

        last_result = match result {
            Ok(ExitStatus::Running(pid)) => {
                if pgid.is_none() {
                    // The first child (the process group leader) pid is used for pgid.
                    pgid = Some(pid);
                }

                if shell.interactive {
                    setpgid(pid, pgid.unwrap()).expect("failed to setpgid");
                }

                childs.push(pid);
                Some(ExitStatus::Running(pid))
            }
            Ok(ExitStatus::ExitedWith(status)) => Some(ExitStatus::ExitedWith(status)),
            Err(err) => {
                unimplemented!("error: {}", err);
            }
        };
    }

    // Wait for the last command in the pipeline.
    match last_result {
        Some(ExitStatus::ExitedWith(status)) => {
            shell.set_last_status(status);
            ExitStatus::ExitedWith(status)
        }
        Some(ExitStatus::Running(_)) => {
            let cmd_name = code.to_owned();
            let job = shell.create_job(cmd_name, pgid.unwrap(), childs);

            if !shell.interactive {
                match wait_for_job(shell, &job) {
                    ProcessState::Completed(status) => {
                        shell.set_last_status(status);
                        ExitStatus::ExitedWith(status)
                    }
                    ProcessState::Stopped(_) => ExitStatus::Running(pgid.unwrap()),
                    _ => unreachable!(),
                }
            } else {
                match run_in_foreground(shell, &job) {
                    ProcessState::Completed(status) => ExitStatus::ExitedWith(status),
                    ProcessState::Stopped(_) => ExitStatus::Running(pgid.unwrap()),
                    _ => unreachable!(),
                }
            }
        }
        None => {
            debug!("nothing to execute");
            ExitStatus::ExitedWith(0)
        }
    }
}

fn run_command(
    shell: &mut Shell,
    command: &parser::Command,
    ctx: &Context,
) -> anyhow::Result<ExitStatus> {
    debug!("run_command: {:?}", command);
    let result = match command {
        parser::Command::SimpleCommand { argv } => run_simple_command(ctx, shell, argv)?,
    };

    Ok(result)
}

fn run_simple_command(
    ctx: &Context,
    shell: &mut Shell,
    argv: &[parser::Word],
) -> anyhow::Result<ExitStatus> {
    debug!("run_simple_command");
    let argv = expand_words(shell, argv)?;
    if argv.is_empty() {
        return Ok(ExitStatus::ExitedWith(0));
    }

    // TODO: support functions

    // Internal commands
    let result = run_internal_command(shell, &argv);
    match result {
        Ok(status) => return Ok(status),
        Err(err) => match err.downcast_ref::<BuiltinCommandError>() {
            Some(BuiltinCommandError::NotFound) => (),
            _ => return Err(err),
        },
    }

    debug!("argv: {:?}", argv);
    // TODO: External commands
    run_external_command(ctx, shell, argv)
}
