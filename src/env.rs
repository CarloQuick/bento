use anyhow::{Context, Result};

use std::env;

pub struct Env {
    pub bento_image_env_path: String,

    pub bento_containers_env_path: String,
}

impl Env {
    pub fn new(bento_image_env_path: String, bento_containers_env_path: String) -> Env {
        Env {
            bento_image_env_path,

            bento_containers_env_path,
        }
    }

    pub fn get_env_vars() -> Result<Env> {
        let bento_image_env_path: String =
            env::var("BENTO_IMAGES_PATH").context("Failed to get images path from .env")?;

        let bento_containers_env_path: String =
            env::var("BENTO_CONTAINERS_PATH").context("Failed to get container path from .env")?;

        let envs: Env = Env::new(bento_image_env_path, bento_containers_env_path);

        Ok(envs)
    }
}
