mod accessibility;
mod cli;
mod client;
mod control;
mod frame;
mod input;
mod protocol;
mod render;
mod run;
mod serve;
mod session_bus;
mod state;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Serve(launch) => serve::print_and_serve(cli.display, launch.size).map(|()| ExitCode::SUCCESS),
        Command::Run { launch, command } => run::run(cli.display, launch.size, &command),
        Command::Control(request) => client::send(cli.display, request).map(|()| ExitCode::SUCCESS),
        Command::Accessibility(command) => accessibility::run(cli.display, command).map(|()| ExitCode::SUCCESS),
    };
    result.unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::FAILURE
    })
}
