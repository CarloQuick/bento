use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
#[derive(Serialize, Deserialize, Debug)]
pub struct OciImageConfig {
    pub architecture: String,
    pub config: OciContainerConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OciContainerConfig {
    #[serde(rename = "Env")]
    pub env: Vec<String>,
    #[serde(rename = "Cmd")]
    pub cmd: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OciIndex {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub manifests: Vec<OciManifestDescriptor>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OciManifestDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
    pub platform: Option<Platform>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Platform {
    pub architecture: String,
    pub os: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OciManifest {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub config: ManifestConfig,
    pub layers: Vec<ManifestLayers>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestConfig {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestLayers {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
}

pub fn get_config_path(digest: &str) -> Option<PathBuf> {
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

pub fn get_oci_index(index_json_path: &PathBuf) -> Result<OciIndex> {
    let file = File::open(index_json_path).expect("Couldnt open Index.json");
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let a: OciIndex = serde_json::from_reader(reader)?;
    Ok(a)
}

pub fn get_oci_manifest(manifest_path: &PathBuf) -> Result<OciManifest> {
    let file = File::open(&manifest_path).expect("Couldnt open Index.json");
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let a: OciManifest = serde_json::from_reader(reader)?;
    Ok(a)
}

pub fn get_nested_manifest(
    tmp_path: &PathBuf,
    nested_index_json_path: &Option<PathBuf>,
) -> Option<usize> {
    if let Some(nested_path) = nested_index_json_path {
        let nested_json =
            get_oci_index(&tmp_path.join(nested_path)).expect("Failed to get nested JSON");
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
