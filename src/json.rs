use colored::Colorize;
use flate2::read;
use serde::{Deserialize, Serialize};
use serde_json::{Result, to_writer_pretty};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingJson {
    architecture: String,
    config: IncomingConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingConfig {
    #[serde(rename = "Env")]
    env: Vec<String>,
    #[serde(rename = "Cmd")]
    cmd: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BentoConfigJson {
    architecture: String,
    cmd: Vec<String>,
    env: Vec<String>,
    image_layers: ImageLayers,
    image_dir: PathBuf,
}

impl BentoConfigJson {
    fn make_bento_config(
        a: &IncomingJson,
        image_layers: &ImageLayers,
        read_path: &PathBuf,
    ) -> BentoConfigJson {
        BentoConfigJson {
            architecture: a.architecture.to_owned(),
            cmd: a.config.cmd.clone(),
            env: a.config.env.clone(),
            image_layers: image_layers.clone(),
            image_dir: read_path.clone(),
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageLayers {
    layers: Vec<String>,
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

fn create_bento_json<P: AsRef<Path>>(
    read_path: P,
    write_path: P,
    image_layers: ImageLayers,
    image_path: &PathBuf,
) -> Result<BentoConfigJson> {
    // Open the file in read-only mode with buffer.
    let file = File::open(read_path).expect("couldnt open");
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `Address`.
    let a: IncomingJson = serde_json::from_reader(reader)?;
    // let image_path = PathBuf::from(read_path.as_ref());
    let bento_config: BentoConfigJson =
        BentoConfigJson::make_bento_config(&a, &image_layers, &image_path);
    eprint!("Bento Json: {:?}", bento_config);
    write_bento_config(write_path, &bento_config)?;

    Ok(bento_config)
}

fn write_bento_config<P: AsRef<Path>>(write_path: P, bento: &BentoConfigJson) -> Result<()> {
    let file = File::create(write_path).expect("couldnt open");
    let mut writer = BufWriter::new(file);
    to_writer_pretty(&mut writer, &bento).unwrap();
    writer.flush().expect("Failed to flush the writer");
    eprint!("{}\n", "🎉 Bento finished 🎉".cyan());
    Ok(())
}

fn get_config_path(digest: &str) -> Option<PathBuf> {
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
pub fn read_write_json(bento_image_path: &PathBuf, cont_path: &PathBuf) {
    let write_path: PathBuf = PathBuf::from(&cont_path).join("bento_config.json");
    let index_json_path: PathBuf = PathBuf::from(&bento_image_path).join("index.json");
    let index_json = get_index_json(&index_json_path).expect("Could not read from index.json");
    if index_json.manifests[0].media_type.contains("image.index") {
        let nested_digest = &index_json.manifests[0].digest;
        let nested_index_json_path = get_config_path(nested_digest);
        if let Some(nested_path) = &nested_index_json_path {
            let amd64_manifest = get_nested_manifest(&bento_image_path, &nested_index_json_path);
            if let Some(index) = amd64_manifest {
                // now that we have an index we want the nested index.json
                let nested_json = get_index_json(&bento_image_path.join(nested_path))
                    .expect("Failed to get nested JSON");
                let manifest_path = get_config_path(&nested_json.manifests[index].digest);
                match manifest_path {
                    None => panic!("No config"),
                    Some(manifest) => {
                        let please_remove = PathBuf::from(&bento_image_path).join(&manifest);
                        let manifest_json = get_manifest_json(&please_remove)
                            .expect("Couldnt get the manifest.json");
                        let image_layers = get_layers_from_manifest(manifest_json.layers)
                            .expect("Failed to get image layers from manifest");
                        let config_path = get_config_path(&manifest_json.config.digest);
                        match config_path {
                            None => panic!("No config"),
                            Some(bento_config) => {
                                let please_remove =
                                    PathBuf::from(&bento_image_path).join(&bento_config);
                                let bento_config_json = create_bento_json(
                                    please_remove,
                                    write_path,
                                    image_layers,
                                    &bento_image_path,
                                )
                                .expect("Failed to create bento json");
                            }
                        }
                    }
                }
            }
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
