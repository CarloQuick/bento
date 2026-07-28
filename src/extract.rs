use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::PathBuf;
use tar::Archive;
use tracing::{debug, info};

pub fn decompress_tarball(path: &str, destination: &str) -> Result<()> {
    debug!("Decompressing tarball at: {:?}", path);
    let tar_gz = File::open(path).with_context(|| format!("Failed to open tar_gz at {}", path))?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive
        .unpack(destination)
        .context("Failed to unpack archive!")?;
    debug!("Decompression Successful!");
    Ok(())
}
pub fn unpack_archive(source: &PathBuf, dest: &PathBuf) -> Result<()> {
    info!("Unpacking archive.");
    debug!("Unpacking image archive from {:?} to {:?}.", source, dest);
    let mut archive = Archive::new(
        File::open(source).with_context(|| format!("Failed to open archive at {:?}", source))?,
    );
    archive
        .unpack(dest)
        .with_context(|| format!("Failed to unpack archive to {:?}", dest))?;
    info!("Unpacking image archive Successful!");
    Ok(())
}
