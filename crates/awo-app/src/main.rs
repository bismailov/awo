mod cli;
mod handlers;
mod output;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{AppCommand, Cli};
use output::{OutputMode, print_json_error};

fn main() -> Result<()> {
    handlers::initialize_tracing()?;

    let cli = Cli::parse();
    let output = OutputMode { json: cli.json };
    let command = cli.command.unwrap_or(AppCommand::Tui);
    let result = handlers::execute(command, output);

    match result {
        Ok(()) => Ok(()),
        Err(error) if output.json => {
            print_json_error(&error);
            Ok(())
        }
        Err(error) => Err(error),
    }
}
