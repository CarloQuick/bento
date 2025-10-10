use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::{Result, to_writer_pretty};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingJson {
    architecture: String,
    config: IncomingConfig,
    rootfs: RootFs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingConfig {
    #[serde(rename = "Env")]
    env: Vec<String>,
    #[serde(rename = "Cmd")]
    cmd: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RootFs {
    #[serde(rename = "type")]
    fs_type: String,
    diff_ids: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct BentoConfigJson {
    architecture: String,
    cmd: Vec<String>,
    env: Vec<String>,
    rootfs: RootFs,
}

impl BentoConfigJson {
    fn make_bento_config(a: &IncomingJson) -> BentoConfigJson {
        BentoConfigJson {
            architecture: a.architecture.to_owned(),
            cmd: a.config.cmd.clone(),
            env: a.config.env.clone(),
            rootfs: a.rootfs.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IndexJson {
    #[serde(rename = "mediaType")]
    media_type: String,
    manifests: Vec<Manifests>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifests {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    platform: Option<Platform>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Platform {
    architecture: String,
    os: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestJson {
    #[serde(rename = "mediaType")]
    media_type: String,
    config: ManifestConfig,
    layers: Vec<ManifestLayers>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestConfig {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestLayers {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
}

fn create_bento_json<P: AsRef<Path>>(read_path: P, write_path: P) -> Result<()> {
    eprint!("{}", "Reading image config\n".blue());
    // Open the file in read-only mode with buffer.
    let file = File::open(read_path).expect("couldnt open");
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `Address`.
    let a: IncomingJson = serde_json::from_reader(reader)?;
    eprint!("Incoming Json: {:?}", a);
    let bento_config: BentoConfigJson = BentoConfigJson::make_bento_config(&a);
    eprint!("Bento Json: {:?}", bento_config);
    write_bento_config(write_path, bento_config)?;
    Ok(())
}

fn write_bento_config<P: AsRef<Path>>(write_path: P, bento: BentoConfigJson) -> Result<()> {
    eprint!("{}\n", "Creating and writing config.🍱".green());
    let file = File::create(write_path).expect("couldnt open");
    let mut writer = BufWriter::new(file);
    to_writer_pretty(&mut writer, &bento).unwrap();
    writer.flush().expect("Failed to flush the writer");
    eprint!("{}\n", "🎉 Bento finished 🎉".cyan());
    Ok(())
}

fn get_config_path(digest: &str) -> Option<PathBuf> {
    // let mut new_diff_ids: Vec<String> = Vec::with_capacity(rootfs.diff_ids.len());
    match digest.find(":") {
        None => None,
        Some(colon_index) => {
            let mut config_path = PathBuf::from("blobs");
            config_path.push(&digest[0..colon_index]);
            config_path.push(&digest[colon_index + 1..]);
            Some(config_path)
        }
    }
}

fn get_index_json(index_json_path: &PathBuf) -> Result<IndexJson> {
    let file = File::open(index_json_path).expect("Couldnt open Index.json");
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let a: IndexJson = serde_json::from_reader(reader)?;
    Ok(a)
}

fn get_manifest_json(manifest_path: &PathBuf) -> Result<ManifestJson> {
    let file = File::open(&manifest_path).expect("Couldnt open Index.json");
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let a: ManifestJson = serde_json::from_reader(reader)?;
    Ok(a)
}

fn get_nested_manifest(
    tmp_path: &PathBuf,
    nested_index_json_path: &Option<PathBuf>,
) -> Option<usize> {
    if let Some(nested_path) = nested_index_json_path {
        let nested_json =
            get_index_json(&tmp_path.join(nested_path)).expect("Failed to get nested JSON");
        for (i, manifest) in nested_json.manifests.iter().enumerate() {
            if let Some(platform_arch) = &manifest.platform {
                if platform_arch.architecture == "amd64" {
                    return Some(i);
                }
            }
        }
    }
    None
}

// TESTING .bento/containers/python
pub fn read_write_json(cont_path: PathBuf) {
    // it writes config to bento container path
    let write_path: PathBuf = PathBuf::from(&cont_path).join("bento_config.json");
    eprintln!("write_path {:?}\n", write_path);
    // the untarred images path at tmp
    let tmp_path: PathBuf = PathBuf::from(&cont_path).join("tmp");
    eprintln!("tmp_path {:?}\n", tmp_path);
    // blobs | index.json | manifest.json | oci-layout
    let index_json_path: PathBuf = PathBuf::from(&tmp_path).join("index.json");
    eprintln!("index_json_path {:?}\n", index_json_path);
    let index_json = get_index_json(&index_json_path).expect("Could not read from index.json");
    eprintln!("index_json {:?}\n", index_json);
    if index_json.manifests[0].media_type.contains("image.index") {
        eprintln!(
            "index_json.manifests[0].media_type {:?}\n",
            index_json.manifests[0].media_type
        );
        let nested_digest = &index_json.manifests[0].digest;
        eprintln!("nested_digest {:?}\n", nested_digest);
        let nested_index_json_path = get_config_path(nested_digest);
        eprintln!("nested_index_json_path {:?}\n", nested_index_json_path);

        if let Some(nested_path) = &nested_index_json_path {
            let amd64_manifest = get_nested_manifest(&tmp_path, &nested_index_json_path);
            if let Some(index) = amd64_manifest {
                // now that we have an index we want the nested index.json
                let nested_json =
                    get_index_json(&tmp_path.join(nested_path)).expect("Failed to get nested JSON");
                let manifest_path = get_config_path(&nested_json.manifests[index].digest);
                eprintln!("manifest_path: {:?}", manifest_path);

                match manifest_path {
                    None => panic!("No config"),
                    Some(manifest) => {
                        let please_remove = PathBuf::from(&tmp_path).join(&manifest);
                        eprintln!("please_remove: {:?}", please_remove);

                        let manifest_json = get_manifest_json(&please_remove)
                            .expect("Couldnt get the manifest.json");
                        let config_path = get_config_path(&manifest_json.config.digest);
                        match config_path {
                            None => panic!("No config"),
                            Some(bento_config) => {
                                let please_remove = PathBuf::from(&tmp_path).join(&bento_config);
                                create_bento_json(please_remove, write_path)
                                    .expect("Failed to create bento json");
                            }
                        }
                    }
                }
            }
        }
    }
}

fn _get_layer_dir(rootfs: &RootFs) -> RootFs {
    let mut new_diff_ids: Vec<String> = Vec::with_capacity(rootfs.diff_ids.len());
    for (_, val) in rootfs.diff_ids.iter().enumerate() {
        if let Some(colon_index) = val.find(":") {
            let mut layer_path = String::from("/blobs/");
            layer_path.push_str(&val[0..colon_index]);
            layer_path.push_str("/");
            layer_path.push_str(&val[colon_index + 1..]);
            new_diff_ids.push(layer_path);
        }
    }

    let updated_rootfs = RootFs {
        fs_type: rootfs.fs_type.clone(),
        diff_ids: new_diff_ids,
    };
    updated_rootfs
}
