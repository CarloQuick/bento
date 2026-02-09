use anyhow::{Context, Result};
use tracing::error;

use std::{env, path::PathBuf};

pub struct Env {
    pub bento_dir: PathBuf,
    pub bento_image_env_path: PathBuf,

    pub bento_containers_env_path: PathBuf,
}

impl Env {
    pub fn new(
        bento_dir: PathBuf,
        bento_image_env_path: PathBuf,
        bento_containers_env_path: PathBuf,
    ) -> Env {
        Env {
            bento_dir,

            bento_image_env_path,

            bento_containers_env_path,
        }
    }

    pub fn get_env_vars() -> Result<Env> {
        let bento_dir: String =
            env::var("BENTO_DIR").context("Failed to get main bento directory from .env")?;

        let bento_image_env_path: String =
            env::var("BENTO_IMAGES_PATH").context("Failed to get images path from .env")?;

        let bento_containers_env_path: String =
            env::var("BENTO_CONTAINERS_PATH").context("Failed to get container path from .env")?;

        if bento_dir.is_empty()
            || bento_image_env_path.is_empty()
            || bento_containers_env_path.is_empty()
        {
            error!("Missing environmental variables.");
            anyhow::bail!("Please set a base bento path in the .env file");
        };

        let envs: Env = Env::new(
            PathBuf::from(bento_dir),
            PathBuf::from(bento_image_env_path),
            PathBuf::from(bento_containers_env_path),
        );

        Ok(envs)
    }
}
