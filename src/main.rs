extern crate dotenv;
use bento::{extract, json};
use dotenv::dotenv;
use std::{env, fs, path::PathBuf};

fn main() {
    dotenv().ok();
    // .bento/images
    let s_path: String =
        env::var("BENTO_IMAGES_PATH").expect("Failed to get images path from .env");
    // .bento/containers
    let d_path: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    // python.tar = hardcoded tar
    let cont_name = String::from("python");
    let mut tar = String::from(&cont_name);
    tar.push_str(".tar");

    // .bento/images/python.tar
    let source = PathBuf::from(&s_path).join(&tar);
    // .bento/containers/python
    let dest = PathBuf::from(&d_path).join(&cont_name);
    // .bento/containers/python/tmp
    let extract_dest = dest.join("tmp");
    // Creates .bento/containers/python/tmp and its parent directories
    fs::create_dir_all(&extract_dest).expect("Failed to create container dir");
    // .bento/images/python.tar => .bento/containers/python/tmp
    extract::unpack_archive(source, extract_dest);
    // .bento/containers/python and reads index.json and blobs
    json::read_write_json(dest);
}
