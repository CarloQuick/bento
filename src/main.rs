extern crate dotenv;
use bento::{
    bento_cli::{Cli, Commands},
    extract, json,
};
use clap::Parser;
use dotenv::dotenv;
use std::{env, fs, path::PathBuf};
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
            let bento_containers_path = PathBuf::from(&bento_containers_env).join(image);

            fs::create_dir_all(&bento_image_path).expect("Failed to create image dir");
            fs::create_dir_all(&bento_containers_path).expect("Failed to create container dir");
            extract::unpack_archive(&image_tar_path, &bento_image_path);
            json::read_write_json(name, &bento_image_path, &bento_containers_path);
        }
        None => {}
    }
}
