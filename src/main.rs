extern crate dotenv;
use anyhow::{Result, anyhow};
use bento::{
    bento_cli::{Cli, Commands},
    env::Env,
    json::{self, State},
    runtime::{create, exec, kill_container, start, stop},
};
use clap::Parser;
use dotenv::dotenv;
use std::path::PathBuf;
use tracing::{info, level_filters::LevelFilter};

fn main() -> Result<()> {
    dotenv().ok();
    let env: Env = Env::get_env_vars()?;
    let cli = Cli::parse();

    let filter = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        match &cli.log {
            Some(log_level) => log_level.to_level_filter(),
            None => LevelFilter::INFO,
        }
    };

    tracing_subscriber::fmt()
        .without_time()
        .with_max_level(filter)
        .init();

    match &cli.command {
        Some(Commands::Create {
            name,
            image,
            mount,
            current_working_directory,
            command,
        }) => match json::check_existing_container(name, &env.bento_containers_env_path) {
            Some(_) => {
                anyhow::bail!(
                    "Container '{}' already exists!\n\tRun `status {}` for information about the already created container.",
                    name,
                    name
                );
            }
            None => {
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
        },
        Some(Commands::Start { name }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => {
                    if let Err(e) = start(&container, name, &env) {
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
                info!("Looking for all container's status");
                json::list_container_manifest(&env.bento_containers_env_path);
            } else {
                match name {
                    Some(n) => {
                        info!("Looking for {}'s status", n);
                        match json::check_existing_container(n, &env.bento_containers_env_path) {
                            Some(container) => {
                                json::print_named_container_state(
                                    n,
                                    &container.state,
                                    container.pid,
                                );
                            }
                            None => {
                                anyhow::bail!("Sorry, {} is not an existing Bento container.", n);
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        Some(Commands::Stop { name }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => match stop(&container, name, &env) {
                    Ok(()) => info!("Container {} stopped successfully", name),
                    Err(e) => anyhow::bail!("{:?}", e),
                },
                None => {
                    anyhow::bail!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        Some(Commands::Kill { name }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => match kill_container(&container) {
                    Ok(()) => info!("Container {} killed successfully", name),
                    Err(e) => anyhow::bail!("{:?}", e),
                },
                None => {
                    anyhow::bail!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        Some(Commands::Exec { name, cmd, args }) => {
            match json::check_existing_container(name, &env.bento_containers_env_path) {
                Some(container) => match container.state {
                    State::Running => match exec(name, &container, cmd, args, &env) {
                        Ok(()) => info!("exec successful"),
                        Err(e) => anyhow::bail!("{:?}", e),
                    },
                    _ => anyhow::bail!("Sorry, not a running container."),
                },
                None => {
                    anyhow::bail!("Sorry, {} is not an existing Bento container.", name);
                }
            }
        }
        None => {}
    }

    Ok(())
}
