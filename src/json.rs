use crate::config::{ImageLayers, create_bento_json};
use crate::oci::{
    ManifestLayers, get_config_path, get_nested_manifest, get_oci_index, get_oci_manifest,
};
use core::panic;
use serde_json::Result;
use std::path::PathBuf;
extern crate dotenv;

use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::{Read, Write},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Container {
    dir: String,
    pub state: State,
    pid: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum State {
    Creating,
    Created,
    Running,
    Stopped,
}

impl State {
    fn print(&self) -> String {
        match self {
            State::Created => String::from("created"),
            State::Creating => String::from("creating"),
            State::Running => String::from("running"),
            State::Stopped => String::from("stopped"),
        }
    }
}

fn get_layers_from_manifest(layers: Vec<ManifestLayers>) -> Result<ImageLayers> {
    let mut image_layers: Vec<String> = Vec::with_capacity(layers.len());
    for (_, val) in layers.iter().enumerate() {
        if let Some(colon_index) = val.digest.find(":") {
            let mut layer_path = String::from("/blobs/");
            layer_path.push_str(&val.digest[0..colon_index]);
            layer_path.push_str("/");
            layer_path.push_str(&val.digest[colon_index + 1..]);
            image_layers.push(layer_path);
        }
    }
    Ok(ImageLayers {
        layers: image_layers,
    })
}
pub fn check_existing_container(name: &str) -> Option<Container> {
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    let bento_container_path = PathBuf::from(&bento_containers_env).join("container_manifest.json");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&bento_container_path)
        .expect("Failed to open File with Options");

    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .expect("Failed to read contents to string");

    let result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).expect("Failed to read json")
    };

    if let Some(existing_container) = result.get(name) {
        Some(existing_container.clone())
    } else {
        None
    }
}

pub fn print_named_container_state(name: &str, state: &State) {
    eprintln!("{:<15} | {:<10}", "Name", "State");
    eprintln!("----------------|----------");
    eprintln!("{:<15} | {:<10}", name, state.print());
}

pub fn get_container_manifest_path() -> PathBuf {
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    let bento_container_path = PathBuf::from(&bento_containers_env).join("container_manifest.json");
    bento_container_path
}

pub fn add_to_container_manifest(name: &str, dir: &PathBuf) -> Result<()> {
    let bento_container_path = get_container_manifest_path();
    let container = Container {
        dir: String::from(dir.to_string_lossy()),
        state: State::Created,
        pid: None,
    };
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&bento_container_path)
        .expect("Failed to open File with Options");

    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .expect("Failed to read contents to string");

    let mut result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).expect("Failed to read json")
    };

    let existing_container = result.get(name);

    match existing_container {
        Some(_c) => {
            println!("Updating existing container of the same name: {}", name);
        }
        None => {
            result.insert(String::from(name), container);
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&bento_container_path)
        .expect("Failed to open File with Options");
    let buf = to_string_pretty(&result).expect("Failed to create the buf from the result map");
    file.write_all(buf.as_bytes())
        .expect("Failed to write container config");

    Ok(())
}

pub fn update_container_status(name: &str, pid: Option<u32>, new_state: State) {
    let bento_container_path = get_container_manifest_path();

    // Open the container manifest with options
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&bento_container_path)
        .expect("Failed to open File with Options");
    // Get the json contents
    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .expect("Failed to read contents to string");

    // Read the resulting json as a HashMap
    let mut result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).expect("Failed to read json")
    };

    result
        .entry(name.to_string())
        .and_modify(|container: &mut Container| container.state = new_state);

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&bento_container_path)
        .expect("Failed to open File with Options");
    let buf = to_string_pretty(&result).expect("Failed to create the buf from the result map");
    file.write_all(buf.as_bytes())
        .expect("Failed to write container config");
    // key: name, value: Container
    let existing_container = result.get(name);

    match existing_container {
        Some(_c) => {}
        None => {
            panic!("Container not present in the Container Manifest");
        }
    }
}

pub fn list_container_manifest() {
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    let bento_container_path = PathBuf::from(&bento_containers_env).join("container_manifest.json");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&bento_container_path)
        .expect("Failed to open File with Options");

    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .expect("Failed to read contents to string");

    let result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).expect("Failed to read json")
    };
    if result.is_empty() {
        eprintln!("No containers available.");
    } else {
        for (k, v) in result.iter() {
            eprintln!("{}", k);
            eprintln!("==> {:?}", v.state)
        }
    }
}

#[cfg(target_arch = "x86")]
pub fn return_cpu_architecture() -> String {
    String::from("x86")
}
#[cfg(target_arch = "x86_64")]
pub fn return_cpu_architecture() -> String {
    String::from("amd64")
}
#[cfg(target_arch = "arm")]
pub fn return_cpu_architecture() -> String {
    String::from("arm")
}
#[cfg(target_arch = "aarch64")]
pub fn return_cpu_architecture() -> String {
    String::from("aarch64")
}

pub fn create_bento_config(
    container_name: &String,
    bento_image_path: &PathBuf,
    bento_container_path: &PathBuf,
    mount: &PathBuf,
    cwd: &PathBuf,
) -> (String, PathBuf) {
    let bento_config_path: PathBuf = PathBuf::from(&bento_container_path).join("bento_config.json");
    let index_json_path: PathBuf = PathBuf::from(&bento_image_path).join("index.json");
    let index_json = get_oci_index(&index_json_path).expect("Could not read from index.json");
    if index_json.manifests[0].media_type.contains("image.index") {
        let nested_digest = &index_json.manifests[0].digest;
        let nested_index_json_path = get_config_path(nested_digest);
        if let Some(nested_path) = &nested_index_json_path {
            let target_arch = return_cpu_architecture();
            let arch_specific_manifest =
                get_nested_manifest(&bento_image_path, &nested_index_json_path, &target_arch);
            if let Some(index) = arch_specific_manifest {
                // now that we have an index we want the nested index.json
                let nested_json = get_oci_index(&bento_image_path.join(nested_path))
                    .expect("Failed to get nested JSON");
                let manifest_path_option = get_config_path(&nested_json.manifests[index].digest);
                match manifest_path_option {
                    None => panic!("No config"),
                    Some(manifest_path) => {
                        let full_manifest_path =
                            PathBuf::from(&bento_image_path).join(&manifest_path);
                        let manifest_json = get_oci_manifest(&full_manifest_path)
                            .expect("Couldnt get the manifest.json");
                        let image_layers = get_layers_from_manifest(manifest_json.layers)
                            .expect("Failed to get image layers from manifest");
                        let manifest_config_path_option =
                            get_config_path(&manifest_json.config.digest);
                        match manifest_config_path_option {
                            None => panic!("No config"),
                            Some(manifest_config_path) => {
                                let full_manifest_config_path =
                                    PathBuf::from(&bento_image_path).join(&manifest_config_path);
                                create_bento_json(
                                    container_name,
                                    full_manifest_config_path,
                                    bento_config_path,
                                    image_layers,
                                    &bento_image_path,
                                    &bento_container_path,
                                    mount,
                                    cwd,
                                )
                                .expect("Failed to create bento json");
                            }
                        }
                    }
                }
            }
        }
    }
    (container_name.to_string(), bento_container_path.to_owned())
}
