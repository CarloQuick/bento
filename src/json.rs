use crate::config::{ImageLayers, create_bento_json};
use crate::oci::{
    ManifestLayers, get_config_path, get_nested_manifest, get_oci_index, get_oci_manifest,
};
use core::panic;
use std::fs::File;
use std::io::Seek;
use std::path::{Path, PathBuf};
extern crate dotenv;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{Read, Write},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Container {
    dir: String,
    pub state: State,
    pub pid: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
pub fn check_existing_container(name: &str, container_path: &PathBuf) -> Option<Container> {
    let manifest_path = container_path.join("container_manifest.json");

    let mut container_manifest = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&manifest_path)
        .expect("Failed to open File with Options");

    let mut json_contents = String::new();
    container_manifest
        .read_to_string(&mut json_contents)
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

pub fn print_named_container_state(name: &str, state: &State, pid: Option<i32>) {
    let pid_str = pid.map_or("N/A".to_string(), |pid| pid.to_string());

    eprintln!("{:<15} | {:<10} | {:<10}", "Name", "State", "PID");
    eprintln!("----------------+------------+------------");
    eprintln!("{:<15} | {:<10} | {:<10}", name, state.print(), pid_str);
}

pub fn add_to_container_manifest(
    name: &str,
    dir: &PathBuf,
    container_path: &PathBuf,
) -> Result<()> {
    let container_manifest_path = container_path.join("container_manifest.json");

    let container = Container {
        dir: String::from(dir.to_string_lossy()),
        state: State::Created,
        pid: None,
    };
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(&container_manifest_path)
        .with_context(|| {
            format!(
                "Failed to open container manifest at {:?}.",
                container_manifest_path
            )
        })?;

    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .context("Failed to read JSON from file.")?;

    let mut result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).context("Failed to return JSON contents.")?
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
        .open(&container_manifest_path)
        .context("Failed to open Container Manifest for writing.")?;
    let buf = to_string_pretty(&result)
        .context("Failed serialize string for container manifest JSON.")?;
    file.write_all(buf.as_bytes()).with_context(|| {
        format!(
            "Failed to write to container manifest at {:?}.",
            container_manifest_path
        )
    })?;

    Ok(())
}

pub fn rollback_container_manifest(name: &str, container_path: &PathBuf) -> Result<()> {
    let container_manifest_path = container_path.join("container_manifest.json");
    eprint!("path from rollback: {:?}", container_manifest_path);
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&container_manifest_path)
        .with_context(|| {
            format!(
                "Failed to open container manifest at {:?}.",
                container_manifest_path
            )
        })?;

    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)
        .context("Failed to read JSON from file.")?;

    let mut result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents).context("Failed to return JSON contents.")?
    };

    let removed_container = result.remove(name);

    match removed_container {
        Some(_) => eprintln!("Removed container: {}", name),
        None => {}
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&container_manifest_path)
        .context("Failed to open Container Manifest for writing.")?;
    let buf = to_string_pretty(&result)
        .context("Failed serialize string for container manifest JSON.")?;
    file.write_all(buf.as_bytes()).with_context(|| {
        format!(
            "Failed to write to container manifest at {:?}.",
            container_manifest_path
        )
    })?;

    Ok(())
}

pub fn get_map_from_json(mut file: &File) -> Result<HashMap<String, Container>> {
    let mut json_contents = String::new();
    file.read_to_string(&mut json_contents)?;

    // Read the resulting json as a HashMap
    let result: HashMap<String, Container> = if json_contents.is_empty() {
        HashMap::new() // Empty file? Start with empty HashMap
    } else {
        serde_json::from_str(&json_contents)?
    };

    Ok(result)
}

pub fn update_container_status(
    name: &str,
    pid: Option<i32>,
    new_state: State,
    container_manifest_path: &PathBuf,
) -> Result<()> {
    // Open the container manifest with options
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(container_manifest_path)
        .with_context(|| {
            format!(
                "Failed to open manifest at: {:?} .",
                container_manifest_path
            )
        })?;

    let mut result = get_map_from_json(&file)?;

    match result.get_mut(name) {
        Some(container) => {
            container.state = new_state;
            container.pid = pid;
        }
        None => return Err(anyhow!("Container not found to update.")),
    }

    file.rewind()
        .context("Failed to rewind the container manifest.")?;
    file.set_len(0)
        .context("Failed to rewind the container manifest.")?;

    let buf = to_string_pretty(&result)
        .context("Failed to converst string to json format update the container manifest.")?;
    file.write_all(buf.as_bytes())
        .context("Failed to update the container manifest.")?;

    Ok(())
}

pub fn list_container_manifest(container_manifest_path: &Path) {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // Create the file if it doesn't exist
        .open(container_manifest_path)
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
    user_cmd: &Option<Vec<String>>,
) -> Result<(String, PathBuf)> {
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
                                    user_cmd,
                                )
                                .expect("Failed to create bento json");
                            }
                        }
                    }
                }
            }
        }
    }
    Ok((container_name.to_string(), bento_container_path.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, remove_dir_all};

    use crate::env::Env;

    use super::*;

    #[test]
    fn adds_new_container_to_manifest() {
        let test_dir: PathBuf = PathBuf::from("/tmp/bento_test1");
        let _ = remove_dir_all(&test_dir);
        create_dir_all(&test_dir).expect("Failed to create test dir");

        if let Err(e) = add_to_container_manifest(
            "test_container",
            &PathBuf::from("/test_path".to_string()),
            &test_dir,
        ) {
            if test_dir.exists() {
                remove_dir_all(&test_dir).expect("Failed to remove test dir");
            } else {
                eprint!(
                    "Failed to removed the test directory after fauled addition to container manifest.{}",
                    e
                );
            }
        };

        let env = Env {
            bento_dir: PathBuf::from("/test_dir"),
            bento_image_env_path: PathBuf::from("/test_image_path"),
            bento_containers_env_path: PathBuf::from(&test_dir),
        };

        let res: Option<Container> =
            check_existing_container("test_container", &env.bento_containers_env_path);

        assert_eq!(
            res,
            Some(Container {
                dir: "/test_path".to_string(),
                state: State::Created,
                pid: None
            })
        );
        if test_dir.exists() {
            remove_dir_all(&test_dir).expect("Failed to remove test dir");
        } else {
            eprint!("done");
        }
    }
    #[test]
    fn rollsback_container_from_manifest() {
        let test_dir: PathBuf = PathBuf::from("/tmp/bento_test2");
        let _ = remove_dir_all(&test_dir);
        create_dir_all(&test_dir).expect("Failed to create test dir");

        let container_name = "test_container";

        if let Err(e) = add_to_container_manifest(
            container_name,
            &PathBuf::from("/test_path".to_string()),
            &test_dir,
        ) {
            if test_dir.exists() {
                remove_dir_all(&test_dir).expect("Failed to remove test dir");
            }
            eprint!(
                "Failed to removed the test directory after fauled addition to container manifest.{}",
                e
            );
        };

        let env = Env {
            bento_dir: PathBuf::from("/test_dir"),
            bento_image_env_path: PathBuf::from("/test_image_path"),
            bento_containers_env_path: PathBuf::from(&test_dir),
        };

        let test_container: Option<Container> =
            check_existing_container("test_container", &env.bento_containers_env_path);

        assert_eq!(
            test_container,
            Some(Container {
                dir: "/test_path".to_string(),
                state: State::Created,
                pid: None
            })
        );

        rollback_container_manifest(container_name, &env.bento_containers_env_path)
            .expect("Failed to rollback container manifest.");

        let test_container: Option<Container> =
            check_existing_container("test_container", &env.bento_containers_env_path);

        assert_eq!(test_container, None);

        if test_dir.exists() {
            remove_dir_all(&test_dir).expect("Failed to remove test dir");
        } else {
            eprint!("done");
        }
    }
}
