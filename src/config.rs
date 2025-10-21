use crate::extract;
use crate::oci::OciImageConfig;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::{Result, to_writer_pretty};
use std::fs::{File, create_dir};
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
    pub lowerdir: Vec<String>,
    pub upperdir: PathBuf,
    pub workdir: PathBuf,
    pub merge: PathBuf,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageLayers {
    pub layers: Vec<String>,
}

impl BentoConfigJson {
    pub fn make_bento_config(
        name: &String,
        oci_image_config: &OciImageConfig,
        image_layers: &ImageLayers,
        image_path: &PathBuf,
        rootfs: &Vec<String>,
        lowerdir: &Vec<String>,
        upperdir: &PathBuf,
        workdir: &PathBuf,
        merge: &PathBuf,
    ) -> BentoConfigJson {
        BentoConfigJson {
            name: name.clone(),
            architecture: oci_image_config.architecture.to_owned(),
            cmd: oci_image_config.config.cmd.clone(),
            env: oci_image_config.config.env.clone(),
            image_layers: image_layers.clone(),
            image_dir: image_path.clone(),
            rootfs: rootfs.clone(),
            lowerdir: lowerdir.clone(),
            upperdir: upperdir.clone(),
            workdir: workdir.clone(),
            merge: merge.clone(),
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

    let oci_image_config: OciImageConfig = serde_json::from_reader(reader)?;

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
    let mut lowerdir_vec: Vec<String> = Vec::with_capacity(rootfs.len());
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
        lowerdir_vec.push(path_to_lower_string.to_owned());
    }
    let (upperdir, workdir, merge) = create_overlayfs(&container_path);

    let bento_config: BentoConfigJson = BentoConfigJson::make_bento_config(
        name,
        &oci_image_config,
        &image_layers,
        &image_path,
        &rootfs,
        &lowerdir_vec,
        &upperdir,
        &workdir,
        &merge,
    );
    write_bento_config(write_path, &bento_config)?;

    Ok(bento_config)
}

pub fn create_overlayfs(container_path: &PathBuf) -> (PathBuf, PathBuf, PathBuf) {
    //** Create your container root directory **//
    let upperdir = container_path.join("upper");
    let workdir = container_path.join("workdir");
    let merge = container_path.join("merge");
    create_dir(&upperdir).expect("Failed to create upperdir");
    create_dir(&workdir).expect("Failed to creat workdir");
    create_dir(&merge).expect("Failed to creat merge");

    (upperdir, workdir, merge)
}

pub fn write_bento_config<P: AsRef<Path>>(write_path: P, bento: &BentoConfigJson) -> Result<()> {
    let file = File::create(write_path).expect("couldnt open");
    let mut writer = BufWriter::new(file);
    to_writer_pretty(&mut writer, &bento).unwrap();
    writer.flush().expect("Failed to flush the writer");
    eprint!("{}\n", "🎉 Bento finished 🎉".cyan());
    Ok(())
}
