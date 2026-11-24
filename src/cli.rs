use clap::{Args, Parser, Subcommand};

use crate::{accessibility, protocol::Request};

#[derive(Parser)]
pub struct Cli {
    #[arg(long, global = true, env = "WAYHARNESS")]
    pub display: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Serve(Launch),
    Run {
        #[command(flatten)]
        launch: Launch,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    #[command(flatten)]
    Control(Request),
    #[command(flatten)]
    Accessibility(accessibility::Command),
}

#[derive(Args)]
pub struct Launch {
    #[arg(long, default_value = "1280x720", value_parser = parse_size)]
    pub size: (i32, i32),
}

fn parse_size(text: &str) -> Result<(i32, i32), String> {
    let (width, height) = text.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let parse = |n: &str| n.parse::<i32>().ok().filter(|n| *n > 0).ok_or(format!("{n}: not a positive number"));
    Ok((parse(width)?, parse(height)?))
}
