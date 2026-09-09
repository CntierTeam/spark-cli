mod cli;
mod filter;
mod graph;
mod load;
mod render;
mod tutorial;
mod views;

use clap::Parser;
use cli::{Cli, Command};

pub mod spark {
    include!(concat!(env!("OUT_DIR"), "/spark.rs"));
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump(args) => cli::run_dump(args)?,
        Command::Profile(args) => cli::run_profile(args)?,
        Command::Heap(args) => cli::run_heap(args)?,
        Command::Health(args) => cli::run_health(args)?,
        Command::Raw(args) => cli::run_raw(args)?,
        Command::Meta(args) => cli::run_meta(args)?,
        Command::Threads(args) => cli::run_threads(args)?,
        Command::Windows(args) => cli::run_windows(args)?,
        Command::Plugins(args) => cli::run_plugins(args)?,
        Command::Tutorial { topic } => {
            print!("{}", tutorial::render(topic.as_deref()));
        }
    }
    Ok(())
}
