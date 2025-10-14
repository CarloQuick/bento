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
    let cont_name = String::from("python:3.14.0rc3-slim-trixie");
    let mut tar = String::from(&cont_name);
    tar.push_str(".tar");

    let bento_image_path = PathBuf::from(&bento_images_env).join(&cont_name);
    // .bento/images/python:3.14.0rc3-slim-trixie.tar
    let image_tar_path = PathBuf::from(&bento_images_env).join(&tar);
    // .bento/containers/python:3.14.0rc3-slim-trixie
    let bento_containers_path = PathBuf::from(&bento_containers_env).join(&cont_name);

    fs::create_dir_all(&bento_image_path).expect("Failed to create image dir");
    fs::create_dir_all(&bento_containers_path).expect("Failed to create container dir");
    // .bento/images/python:3.14.0rc3-slim-trixie.tar => .bento/containers/python:3.14.0rc3-slim-trixie/tmp
    extract::unpack_archive(&image_tar_path, &bento_image_path);
    // .bento/containers/python:3.14.0rc3-slim-trixie and reads index.json and blobs
    json::read_write_json(&bento_image_path, &bento_containers_path);
}
