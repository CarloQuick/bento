extern crate dotenv;
use bento::{
    bento_cli::{Cli, Commands},
    json,
    runtime::{create, start},
};
use clap::Parser;
use dotenv::dotenv;

fn main() {
    dotenv().ok();
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Create { name, image }) => {
            match create(name, image) {
                Ok(_) => eprintln!("{}\n", "🎉 Bento finished 🎉"),
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
                            json::print_named_container_state(n, &container.state);
                        }
                        None => {
                            eprintln!("Sorry, {} is not an existing Bento container.", n);
                        }
                    },
                    None => {}
                }
            }
        }
        None => {}
    }
}
