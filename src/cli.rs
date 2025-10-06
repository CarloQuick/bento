use clap::{Parser, Subcommand};
use names::{Generator, Name};

use crate::runtime;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Creates a container without running it
    Create {
        /// Name of the runnable container
        #[arg(short, long)]
        name: Option<String>,
    },
}

pub fn bento_cli() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Create { name }) => {
            if let Some(name) = name {
                runtime::create_container(name);
            } else {
                let mut generator: Generator<'_> = Generator::with_naming(Name::Numbered);
                let name = generator.next().unwrap();
                runtime::create_container(&name);
            }
        }
        None => {}
    }
}
