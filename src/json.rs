use crate::config::{ImageLayers, create_bento_json};
use crate::oci::{
    ManifestLayers, get_config_path, get_nested_manifest, get_oci_index, get_oci_manifest,
};
use serde_json::Result;
use std::path::PathBuf;

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

pub fn create(container_name: &String, bento_image_path: &PathBuf, bento_container_path: &PathBuf) {
    let bento_config_path: PathBuf = PathBuf::from(&bento_container_path).join("bento_config.json");
    let index_json_path: PathBuf = PathBuf::from(&bento_image_path).join("index.json");
    let index_json = get_oci_index(&index_json_path).expect("Could not read from index.json");
    if index_json.manifests[0].media_type.contains("image.index") {
        let nested_digest = &index_json.manifests[0].digest;
        let nested_index_json_path = get_config_path(nested_digest);
        if let Some(nested_path) = &nested_index_json_path {
            let amd64_manifest = get_nested_manifest(&bento_image_path, &nested_index_json_path);
            if let Some(index) = amd64_manifest {
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
