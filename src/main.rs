extern crate dotenv;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use bento::{
    bento_cli::{Cli, Commands},
    env::Env,
    json::{self, State},
    runtime::{create, exec, kill_proc, start, stop},
};
use clap::Parser;
use dotenv::dotenv;
use nix::NixPath;

fn main() -> Result<()> {
    dotenv().ok();
    let env: Env = Env::get_env_vars().context("Failed to retrieve env variables.")?;
    if env.bento_dir.is_empty() {
        anyhow::bail!("Please set a base bento path in the .env file");
    }

    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Create {
            name,
            image,
            mount,
            current_working_directory,
            command,
        }) => {
            let cwd = match current_working_directory {
                Some(c) => c,
                None => &PathBuf::from("/"),
            };
            let mount_dir = match mount {
                Some(m) => m,
                None => &PathBuf::new(),
            };

            match create(name, image, mount_dir, cwd, command, &env) {
                Ok(_) => {
                    eprintln!("🍱 Bento Container {} finished", name)
                }
                Err(e) => return Err(anyhow!("Container not created.\n Error: {}.", e)),
            };
        }
        Some(Commands::Start { name }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(_container) => {
                    if let Err(e) = start(name, &env) {
                        anyhow::bail!("Starting {} failed! Error: {}.", name, e);
                    }
                }
                None => {
                    anyhow::bail!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        Some(Commands::Status { name, all }) => {
            if *all {
                json::list_container_manifest(&env.bento_containers_env_path);
            } else {
                match name {
                    Some(n) => {
                        match json::check_existing_container(n, &env.bento_containers_env_path) {
                            Some(container) => {
                                json::print_named_container_state(
                                    n,
                                    &container.state,
                                    container.pid,
                                );
                            }
                            None => {
                                eprintln!("Sorry, {} is not an existing Bento container.", n);
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        Some(Commands::Stop { name }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => match stop(name, &container, &env) {
                    Ok(()) => eprintln!("Container {} stopped successfully", name),
                    Err(e) => eprintln!("{:?}", e),
                },
                None => {
                    eprintln!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        Some(Commands::Kill { name }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => match kill_proc(&container) {
                    Ok(()) => eprintln!("Container {} killed successfully", name),
                    Err(e) => eprintln!("{:?}", e),
                },
                None => {
                    eprintln!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        Some(Commands::Exec { name, cmd, args }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => match container.state {
                    State::Running => match exec(name, &container, cmd, args, &env) {
                        Ok(()) => eprintln!("exec successful"),
                        Err(e) => eprintln!("{:?}", e),
                    },
                    _ => eprintln!("Sorry, not a running container."),
                },
                None => {
                    eprintln!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        None => {}
    }

    Ok(())
}
