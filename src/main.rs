extern crate dotenv;
use std::path::PathBuf;

use bento::{
    bento_cli::{Cli, Commands},
    json,
    runtime::{create, start, stop},
};
use clap::Parser;
use dotenv::dotenv;

fn main() {
    dotenv().ok();
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Create {
            name,
            image,
            mount,
            cwd,
        }) => {
            let mount_dir = match mount {
                Some(m) => m,
                None => &PathBuf::new(),
            };
            match create(name, image, mount_dir, cwd) {
                Ok(_) => eprintln!("🍱 Bento Container {} finished", name),
                Err(e) => panic!("Problem creating the bento manifest: {e:?}"),
            };
        }
        Some(Commands::Start { name }) => {
            // Later we will access the image json
            match json::check_existing_container(name) {
                Some(_container) => {
                    start(name);
                }
                None => {
                    eprintln!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        Some(Commands::Status { name, all }) => {
            if *all {
                json::list_container_manifest();
            } else {
                match name {
                    Some(n) => match json::check_existing_container(n) {
                        Some(container) => {
                            json::print_named_container_state(n, &container.state, container.pid);
                        }
                        None => {
                            eprintln!("Sorry, {} is not an existing Bento container.", n);
                        }
                    },
                    None => {}
                }
            }
        }
        Some(Commands::Stop { name }) => match json::check_existing_container(name) {
            Some(container) => match stop(name, &container) {
                Ok(()) => eprint!("Container {} stopped successfully", name),
                Err(e) => eprint!("{:?}", e),
            },
            None => {
                eprintln!("Sorry, {} is not an existing Bento container.", name);
            }
        },
        None => {}
    }
}
