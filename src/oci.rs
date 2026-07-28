use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use tracing::debug;
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
    debug!("Looking for config path from: `{}`", digest);
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
    debug!("Getting the OCI Image Index at {:?}.", index_json_path);
    let file = File::open(index_json_path)
        .with_context(|| format!("Couldnt open Index.json at {}.", index_json_path.display()))?;
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let oci_index: OciIndex = serde_json::from_reader(reader).with_context(|| {
        format!(
            "Failed to read OciIndex from: {}.",
            index_json_path.display()
        )
    })?;
    debug!("OCI Index contents:\n{:#?}.", oci_index);
    Ok(oci_index)
}

pub fn get_oci_manifest(manifest_path: &PathBuf) -> Result<OciManifest> {
    debug!("Checking existence of Oci Manifest Path");
    if !manifest_path.exists() {
        return Err(anyhow!(
            "Manifest path does not exist at {:?}.",
            manifest_path
        ));
    }
    let file = File::open(&manifest_path)
        .with_context(|| format!("Failed to open OciManifest at {}.", manifest_path.display()))?;
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let oci_manifest: OciManifest = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to read OciManifest at {}.", manifest_path.display()))?;

    debug!("Contents of OCI manifest:\n{:#?}", oci_manifest);
    Ok(oci_manifest)
}

pub fn get_nested_manifest(
    tmp_path: &PathBuf,
    nested_index_json_path: &Option<PathBuf>,
    target_arch: &String,
) -> Result<Option<usize>> {
    debug!(
        "Getting nested manifest that matches machine's architecture: {}",
        target_arch
    );
    if let Some(nested_path) = nested_index_json_path {
        match get_oci_index(&tmp_path.join(nested_path)) {
            Ok(nested) => {
                for (i, manifest) in nested.manifests.iter().enumerate() {
                    if let Some(platform_arch) = &manifest.platform {
                        if platform_arch.architecture == *target_arch {
                            debug!("Found target arch index at: {}.", i);
                            return Ok(Some(i));
                        }
                    }
                }
            }
            Err(err) => {
                return Err(anyhow!(
                    "Error occurred looking for architecture index.\n\t{}.",
                    err
                ));
            }
        }
    }
    debug!("No nested path found.");
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_hash_to_path() {
        let result = get_config_path(
            "sha256:e1bb3adc44d158b8b37d647dbfa5864b74f387552c5f12abe9cc95ae773b7c69",
        );
        let config_path = PathBuf::from(
            "blobs/sha256/e1bb3adc44d158b8b37d647dbfa5864b74f387552c5f12abe9cc95ae773b7c69",
        );
        assert_eq!(result, Some(config_path));
    }
    #[test]
    fn no_hash_in_config() {
        let result =
            get_config_path("e1bb3adc44d158b8b37d647dbfa5864b74f387552c5f12abe9cc95ae773b7c69");
        assert_eq!(result, None);
    }
}
