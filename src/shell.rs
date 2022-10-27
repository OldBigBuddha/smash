use crate::eval::eval;
use crate::parser;
use crate::process::{ExitStatus, Job, JobId, ProcessState};

use nix::sys::termios::Termios;
use nix::unistd::{getpid, Pid};
use std::collections::HashMap;
use std::rc::Rc;
use tracing::debug;

pub struct Shell {
    last_status: i32,
    pub interactive: bool,
    pub shell_termios: Option<Termios>,
    states: HashMap<Pid, ProcessState>,
    pub shell_pgid: Pid,
    jobs: HashMap<JobId, Rc<Job>>,
    pub last_fore_job: Option<Rc<Job>>,
    pid_job_mapping: HashMap<Pid, Rc<Job>>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            last_status: 0,
            interactive: false,
            shell_termios: None,
            states: HashMap::new(),
            shell_pgid: getpid(),
            jobs: HashMap::new(),
            last_fore_job: None,
            pid_job_mapping: HashMap::new(),
        }
    }

    pub fn set_interactive(&mut self, interactive: bool) {
        self.interactive = interactive;
    }

    pub fn get_process_state(&self, pid: Pid) -> Option<&ProcessState> {
        self.states.get(&pid)
    }

    pub fn set_process_state(&mut self, pid: Pid, state: ProcessState) {
        self.states.insert(pid, state);
    }

    pub fn set_last_status(&mut self, status: i32) {
        self.last_status = status;
    }

    pub fn run_script(&mut self, script: &str) -> ExitStatus {
        self.run_script_with_stdio(script)
    }

    pub fn ifs(&self) -> String {
        "\n\t ".to_string()
    }

    pub fn create_job(&mut self, name: String, pgid: Pid, childs: Vec<Pid>) -> Rc<Job> {
        let id = self.alloc_job_id();
        let job = Rc::new(Job::new(id, pgid, name, childs.clone()));
        for child in childs {
            self.set_process_state(child, ProcessState::Running);
            self.pid_job_mapping.insert(child, job.clone());
        }

        self.jobs_mut().insert(id, job.clone());
        job
    }

    pub fn jobs_mut(&mut self) -> &mut HashMap<JobId, Rc<Job>> {
        &mut self.jobs
    }

    fn alloc_job_id(&mut self) -> JobId {
        let mut id = 1;
        while self.jobs.contains_key(&JobId::new(id)) {
            id += 1;
        }

        JobId::new(id)
    }

    /// Parse and run a script in the given context
    pub fn run_script_with_stdio(&mut self, script: &str) -> ExitStatus {
        match parser::parse(script) {
            Ok(ast) => eval(self, &ast),
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
