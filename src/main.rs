mod cli;
mod command_tree;
mod log_tree;
mod model;
mod shell_out;
mod terminal;
mod update;
mod view;

use crate::model::{Model, State};
use crate::update::update;
use crate::view::view;

use anyhow::Result;
use clap::Parser;
use cli::Args;
use shell_out::JjCommand;
use terminal::Term;

fn main() {
    let result = run();
    if let Err(err) = result {
        // Avoids a redundant message "Error: Error:"
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let repository = JjCommand::ensure_valid_repo(&args.repository)?;
    let model = Model::new(repository, args.revisions)?;

    let terminal = terminal::init_terminal()?;
    let result = tui_loop(model, terminal);
    terminal::relinquish_terminal()?;

    result
}

fn tui_loop(mut model: Model, terminal: Term) -> Result<()> {
    while model.state != State::Quit {
        terminal.borrow_mut().draw(|f| view(&mut model, f))?;
        update(terminal.clone(), &mut model)?;
    }
    Ok(())
}
