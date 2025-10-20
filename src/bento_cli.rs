use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Creates a container without running it
    Create {
        /// Name of the runnable container
        #[arg(short, long)]
        name: String,

        /// Name of available image
        #[arg(short, long)]
        image: String,
    },
}
