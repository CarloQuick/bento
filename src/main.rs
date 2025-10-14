extern crate dotenv;
use bento::{extract, json};
use dotenv::dotenv;
use std::{env, fs, path::PathBuf};

fn main() {
    dotenv().ok();
    // .bento/images
    let bento_images_env: String =
        env::var("BENTO_IMAGES_PATH").expect("Failed to get images path from .env");
    // .bento/containers
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    // python.tar = hardcoded tar
    let cont_name = String::from("python");
    let mut tar = String::from(&cont_name);
    tar.push_str(".tar");

    // .bento/images/python.tar
    let bento_images_path = PathBuf::from(&bento_images_env).join(&tar);
    // .bento/containers/python
    let bento_containers_path = PathBuf::from(&bento_containers_env).join(&cont_name);
    // .bento/containers/python/tmp
    let extract_dest = bento_containers_path.join("tmp");
    // Creates .bento/containers/python/tmp and its parent directories
    fs::create_dir_all(&extract_dest).expect("Failed to create container dir");
    // .bento/images/python.tar => .bento/containers/python/tmp
    extract::unpack_archive(bento_images_path, extract_dest);
    // .bento/containers/python and reads index.json and blobs
    json::read_write_json(bento_containers_path);
}
