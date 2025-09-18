use flate2::Compression;
use flate2::write::{GzDecoder, GzEncoder};
use std::fs::File;
use tar::Archive;

/// USAGE
/// let file_name = "image.tar.gz";
/// let path_to_content = "src/hi.txt";
/// let dir_to_create = "docs/hi.txt";
/// tar::create_tar_ball(file_name, path_to_content, dir_to_create).unwrap();
/// assert!(fs::exists(file_name).expect("Can't check the esitense of file doe not exist"));
///
/// tar::decompress_tarball(file_name, ".").unwrap();

pub fn _create_tar_ball(
    file_name: &str,
    path_to_content: &str,
    dir_to_create: &str,
) -> Result<(), std::io::Error> {
    let tar_gz = File::create(file_name)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_path_with_name(path_to_content, dir_to_create)?;
    tar.finish()?;
    Ok(())
}

pub fn _decompress_tarball(path: &str, destination: &str) -> Result<(), std::io::Error> {
    let tar_gz = File::open(path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(destination)?;

    Ok(())
}
