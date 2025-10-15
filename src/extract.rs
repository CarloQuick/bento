// use flate2::Compression;
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::PathBuf;
use tar::Archive;

pub fn decompress_tarball(path: &str, destination: &str) -> Result<(), std::io::Error> {
    println!("Opening file: {}", path);
    let tar_gz = File::open(path).unwrap();

    println!("Creating GzDecoder");
    let tar = GzDecoder::new(tar_gz);

    println!("Creating Archive");
    let mut archive = Archive::new(tar);

    println!("Unpacking to: {}", destination);
    archive.unpack(destination).unwrap();

    println!("Unpack complete");
    Ok(())
}
pub fn unpack_archive(source: &PathBuf, dest: &PathBuf) {
    let mut ar = Archive::new(File::open(source).unwrap());
    ar.unpack(dest).unwrap();
}
