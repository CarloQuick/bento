use crate::extract;
use crate::oci::OciImageConfig;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::{Result, to_writer_pretty};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BentoConfigJson {
    pub name: String,
    pub architecture: String,
    pub cmd: Vec<String>,
    pub env: Vec<String>,
    pub image_layers: ImageLayers,
    pub image_dir: PathBuf,
    pub rootfs: Vec<String>,
    pub lower_dir: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageLayers {
    pub layers: Vec<String>,
}

impl BentoConfigJson {
    pub fn make_bento_config(
        name: &String,
        a: &OciImageConfig,
        image_layers: &ImageLayers,
        read_path: &PathBuf,
        rootfs: &Vec<String>,
        lower_dir: &Vec<String>,
    ) -> BentoConfigJson {
        BentoConfigJson {
            name: name.clone(),
            architecture: a.architecture.to_owned(),
            cmd: a.config.cmd.clone(),
            env: a.config.env.clone(),
            image_layers: image_layers.clone(),
            image_dir: read_path.clone(),
            rootfs: rootfs.clone(),
            lower_dir: lower_dir.clone(),
        }
    }
}
pub fn create_bento_json<P: AsRef<Path>>(
    name: &String,
    read_path: P,
    write_path: P,
    image_layers: ImageLayers,
    image_path: &PathBuf,
    container_path: &PathBuf,
) -> Result<BentoConfigJson> {
    // Open the file in read-only mode with buffer.
    let read_file =
        File::open(read_path).expect("Failed to open read path while creating bento_config");
    let reader = BufReader::new(read_file);

    let a: OciImageConfig = serde_json::from_reader(reader)?;

    let mut rootfs: Vec<String> = Vec::with_capacity(image_layers.layers.len());
    for (_, val) in image_layers.layers.iter().enumerate() {
        let mut path = image_path
            .clone()
            .into_os_string()
            .into_string()
            .expect("Failed to create rootfs path");
        path.push_str(val);
        rootfs.push(path.to_string());
    }
    let mut lower_dir_vec: Vec<String> = Vec::with_capacity(rootfs.len());
    for (i, path) in rootfs.iter().enumerate().rev() {
        let mut lowerdir = String::from("lowerdir");
        lowerdir.push_str(&i.to_string());
        let path_to_lower: PathBuf = PathBuf::from(&image_path).join(&lowerdir);
        let path_to_lower_string = &path_to_lower
            .clone()
            .into_os_string()
            .into_string()
            .expect("Failed to create rootfs path");
        extract::decompress_tarball(&path, &path_to_lower_string)
            .expect("Failed to ungzip tarball");
        lower_dir_vec.push(path_to_lower_string.to_owned());
    }
    let bento_config: BentoConfigJson = BentoConfigJson::make_bento_config(
        name,
        &a,
        &image_layers,
        &image_path,
        &rootfs,
        &lower_dir_vec,
    );
    write_bento_config(write_path, &bento_config)?;

    Ok(bento_config)
}

pub fn write_bento_config<P: AsRef<Path>>(write_path: P, bento: &BentoConfigJson) -> Result<()> {
    let file = File::create(write_path).expect("couldnt open");
    let mut writer = BufWriter::new(file);
    to_writer_pretty(&mut writer, &bento).unwrap();
    writer.flush().expect("Failed to flush the writer");
    eprint!("{}\n", "🎉 Bento finished 🎉".cyan());
    Ok(())
}
