extern crate dotenv;
use bento::{
    bento_cli::{Cli, Commands},
    extract, json,
    runtime::start,
};
use clap::Parser;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::{
    collections::HashMap,
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Container {
    state: State,
    pid: u64,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum State {
    Creating,
    Created,
    Running,
    Stopped,
}

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
            json::create(name, &bento_image_path, &bento_container_path);
        }
        Some(Commands::Start { name }) => {
            // Later we will access the image json
            start(name);
        }
        None => {
            let container = Container {
                state: State::Creating,
                pid: 0,
            };
            // let container_json = ContainerJson {
            //     name: String::from("python"),
            //     container,
            // };
            read_container_json(container, "python");
        }
    }
}
pub fn read_container_json(container: Container, name: &str) {
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    let bento_container_path = PathBuf::from(&bento_containers_env).join("container_map.json");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&bento_container_path)
        .expect("Failed to open File with Options");

    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .expect("Failed to read contents to string");

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&bento_container_path)
        .expect("Failed to open File with Options");

    let mut result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).expect("Failed to read json")
    };

    let existing_container = result.get(name);

    match existing_container {
        Some(c) => println!("{:?}", c),
        None => {
            println!("{} not found.", name);
            result.insert(String::from(name), container);
        }
    }
    let buf = to_string_pretty(&result).expect("Failed to create the buf from the result map");
    file.write_all(buf.as_bytes())
        .expect("Failed to write the buf as bytes");
    println!("After checking {:?}", result);
}
