extern crate dotenv;
use bento::{
    bento_cli::{Cli, Commands},
    extract, json,
    runtime::start,
};
use clap::Parser;
use dotenv::dotenv;
use std::{
    env,
    fs::{self},
    path::PathBuf,
};

fn main() {
    dotenv().ok();
    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Create { name, image }) => {
            let bento_images_env: String =
                env::var("BENTO_IMAGES_PATH").expect("Failed to get images path from .env");

            let bento_containers_env: String =
                env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");

            let mut tar = String::from(image);
            tar.push_str(".tar");

            let bento_image_path = PathBuf::from(&bento_images_env).join(image);
            let image_tar_path = PathBuf::from(&bento_images_env).join(&tar);
            let bento_container_path = PathBuf::from(&bento_containers_env).join(name);

            fs::create_dir_all(&bento_image_path).expect("Failed to create image dir");
            fs::create_dir_all(&bento_container_path).expect("Failed to create container dir");
            extract::unpack_archive(&image_tar_path, &bento_image_path);
            let (container_name, created_container_path) =
                json::create(name, &bento_image_path, &bento_container_path);
            json::add_to_container_manifest(&container_name, &created_container_path, 0);
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
                let map = json::read_all_container_manifest();
                for (k, v) in map.iter() {
                    eprintln!("{}", k);
                    eprintln!("==> {:?}", v.state)
                }
            } else {
                match name {
                    Some(n) => match json::check_existing_container(n) {
                        Some(container) => {
                            eprintln!("{}", n);
                            eprintln!("==> {:?}", container.state);
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
