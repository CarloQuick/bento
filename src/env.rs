use anyhow::{Context, Ok, Result, anyhow};
use nix::NixPath;
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

        let envs: Env = Env::new(
            PathBuf::from(bento_dir),
            PathBuf::from(bento_image_env_path),
            PathBuf::from(bento_containers_env_path),
        );

        envs.check_envs()?;

        Ok(envs)
    }

    pub fn check_envs(&self) -> Result<()> {
        if self.bento_dir.is_empty() || !self.bento_dir.exists() {
            return Err(anyhow!(
                "BENTO_DIR is either empty or path does not exist. Check your .env file!"
            ));
        }
        if self.bento_image_env_path.is_empty() || !self.bento_image_env_path.exists() {
            return Err(anyhow!(
                "BENTO_IMAGES_PATH is either empty or path does not exist. Check your .env file!"
            ));
        }
        if self.bento_containers_env_path.is_empty() || !self.bento_containers_env_path.exists() {
            return Err(anyhow!(
                "BENTO_CONTAINERS_PATH is either empty or path does not exist. Check your .env file!"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, remove_dir_all};

    use super::*;

    #[test]
    fn env_vals_ok() -> Result<()> {
        let test_path = "/tmp/test_path_env_vals_ok";
        create_dir_all(test_path)?;

        let envs: Env = Env::new(
            PathBuf::from(test_path),
            PathBuf::from(test_path),
            PathBuf::from(test_path),
        );

        assert!(&envs.check_envs().is_ok());
        remove_dir_all(test_path)?;
        Ok(())
    }
    #[test]
    fn missing_bento_dir_err() -> Result<()> {
        let test_path = "/tmp/test_path_missing_bento_dir_err";
        let missing_bento_dir = "/tmp/missing_est_path";
        create_dir_all(test_path)?;

        let envs: Env = Env::new(
            PathBuf::from(missing_bento_dir),
            PathBuf::from(test_path),
            PathBuf::from(test_path),
        );

        assert!(&envs.check_envs().is_err());
        remove_dir_all(test_path)?;
        Ok(())
    }
    #[test]
    fn bento_dir_is_empty_err() -> Result<()> {
        let test_path = "/tmp/test_path_bento_dir_is_empty_err";
        let missing_bento_dir = "";
        create_dir_all(test_path)?;

        let envs: Env = Env::new(
            PathBuf::from(missing_bento_dir),
            PathBuf::from(test_path),
            PathBuf::from(test_path),
        );

        assert!(&envs.check_envs().is_err());
        remove_dir_all(test_path)?;
        Ok(())
    }
}
