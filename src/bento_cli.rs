use std::{ffi::CString, path::PathBuf};

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
        name: String,

        /// Name of available image
        image: String,

        /// Absolute path for container's working directory
        #[arg(long, value_name = "FILE")]
        current_working_directory: Option<PathBuf>,

        /// Location of volume mount
        #[arg(short, long, value_name = "FILE")]
        mount: Option<PathBuf>,

        /// Container default command override
        #[arg(long, allow_hyphen_values = true, num_args = 1..)]
        command: Option<Vec<String>>,
    },
    /// Starts a container with the data compiled in "create"
    Start {
        /// Name of the already created container
        name: String,
    },
    /// Returns the status of the identified container
    #[group(required = true, multiple = false)]
    Status {
        /// Name of the already created container
        name: Option<String>,

        /// Name all created container
        #[arg(short, long)]
        all: bool,
    },
    /// Sends SIGTERM to container's process or SIGKILL if taking too long
    Stop {
        /// Name of the container attempt a gracefully end
        name: String,
    },
    /// Sends SIGKILL to container's process
    Kill {
        /// Name of the container to forcibly kill
        name: String,
    },
    /// Execute commands inside a container
    Exec {
        /// Name of the container to align with
        name: String,
        /// Container command
        #[arg(allow_hyphen_values = true)]
        cmd: String,
        /// Container args
        #[arg(allow_hyphen_values = true)]
        args: Vec<CString>,
    },
}
