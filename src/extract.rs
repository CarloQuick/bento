use flate2::read::GzDecoder;
use std::fs::File;
use std::path::PathBuf;
use tar::Archive;

pub fn decompress_tarball(path: &str, destination: &str) -> Result<(), std::io::Error> {
    let tar_gz = File::open(path).unwrap();
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(destination).unwrap();

    Ok(())
}
pub fn unpack_archive(source: &PathBuf, dest: &PathBuf) -> Result<(), std::io::Error> {
    let mut ar = Archive::new(File::open(source).unwrap());
    ar.unpack(dest).unwrap();

    Ok(())
}
