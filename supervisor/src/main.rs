mod owned_command;
mod process_tree;

use anyhow::{Context, Result};
use clap::Parser;
use owned_command::OwnedCommand;
use std::{
    ffi::OsString,
    process::{Command, ExitCode},
    sync::{Arc, atomic::AtomicUsize},
    time::Duration,
};

#[derive(Parser)]
struct Options {
    #[arg(long, default_value_t = 0)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 10000)]
    grace_ms: u64,
    #[arg(required = true, last = true)]
    command: Vec<OsString>,
}

fn run() -> Result<i32> {
    let options = Options::parse();
    let received = Arc::new(AtomicUsize::new(0));
    for signal in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register_usize(signal, Arc::clone(&received), usize::try_from(signal)?)
            .context("install supervisor shutdown handler")?;
    }
    let executable = options
        .command
        .first()
        .context("missing supervised executable")?;
    let mut command = Command::new(executable);
    command.args(options.command.iter().skip(1));
    let mut owned = OwnedCommand::spawn(command, Duration::from_millis(options.grace_ms))?;
    owned.wait(
        &received,
        (options.timeout_ms != 0).then(|| Duration::from_millis(options.timeout_ms)),
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(error) => {
            eprintln!("process supervisor: {error:#}");
            ExitCode::FAILURE
        }
    }
}
