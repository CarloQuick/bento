use std::path::PathBuf;

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
        /// Name of the container
        #[arg(short, long)]
        name: String,

        /// Name of available image
        #[arg(short, long)]
        image: String,

        /// Location of volume mount
        #[arg(short, long, value_name = "FILE")]
        mount: Option<PathBuf>,

        /// Absolute path for container's working directory
        #[arg(long, value_name = "FILE")]
        cwd: PathBuf,
    },
    /// Starts a container with the data compiled in "create"
    Start {
        /// Name of the already created container
        #[arg(short, long)]
        name: String,
    },
    /// Returns the status of the identified container
    Status {
        /// Name of the already created container
        #[arg(short, long)]
        name: Option<String>,

        /// Name all created container
        #[arg(short, long)]
        all: bool,
    },
    /// Sends SIGTERM to container's process or SIGKILL if taking too long
    Stop {
        /// Name of the container attempt a gracefully end
        #[arg(short, long)]
        name: String,
    },
    /// Sends SIGKILL to container's process
    Kill {
        /// Name of the container to forcibly kill
        #[arg(short, long)]
        name: String,
    },
    /// Execute commands inside a container
    Exec {
        /// Name of the container to align with
        name: String,
        /// Container commands
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
