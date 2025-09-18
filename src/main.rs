use clap::{Parser, Subcommand};
// use nix::sys::utsname::uname;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Creates a container without running it
    Create {
        /// Image for writable container layer
        #[arg(short, long)]
        image: String,
        /// Name of the runnable container
        #[arg(short, long)]
        name: Option<String>,
    },
}

fn _bento_cli() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Create { image, name }) => {
            println!("Image to pull: {}", image);
            if let Some(name) = name {
                println!("Container name: {name}")
            }
        }
        None => {}
    }
}
fn main() {
    _bento_cli();
}
