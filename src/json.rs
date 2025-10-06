#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingJson {
    architecture: String,
    config: IncomingConfig,
    rootfs: RootFs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingConfig {
    #[serde(rename = "Hostname")]
    hostname: Option<String>,
    #[serde(rename = "Domainname")]
    domainname: Option<String>,
    #[serde(rename = "User")]
    user: Option<String>,
    #[serde(rename = "AttachStdin")]
    attach_stdin: Option<bool>,
    #[serde(rename = "AttachStdout")]
    attach_stdout: Option<bool>,
    #[serde(rename = "AttachStderr")]
    attach_stderr: Option<bool>,
    #[serde(rename = "Tty")]
    tty: Option<bool>,
    #[serde(rename = "OpenStdin")]
    open_stdin: Option<bool>,
    #[serde(rename = "StdinOnce")]
    stdin_once: Option<bool>,
    #[serde(rename = "Env")]
    env: Vec<String>,
    #[serde(rename = "Cmd")]
    cmd: Vec<String>,
    #[serde(rename = "Image")]
    image: Option<String>,
    #[serde(rename = "Volumes")]
    volumes: Option<()>,
    #[serde(rename = "WorkingDir")]
    working_dir: Option<String>,
    #[serde(rename = "Entrypoint")]
    entrypoint: Option<()>,
    #[serde(rename = "OnBuild")]
    on_build: Option<()>,
    #[serde(rename = "Labels")]
    labels: Option<Labels>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Labels {
    #[serde(rename = "org.opencontainers.image.ref.name")]
    org_opencontainers_image_ref_name: String,
    #[serde(rename = "org.opencontainers.image.version")]
    org_opencontainers_image_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RootFs {
    #[serde(rename = "type")]
    fs_type: String,
    diff_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BentoConfigJson {
    architecture: String,
    cmd: Vec<String>,
    env: Vec<String>,
    rootfs: RootFs,
}

impl BentoConfigJson {
    fn make_bento_config(a: &IncomingJson) -> BentoConfigJson {
        BentoConfigJson {
            architecture: a.architecture.to_owned(),
            cmd: a.config.cmd.clone(),
            env: a.config.env.clone(),
            rootfs: a.rootfs.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IndexJson {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(rename = "mediaType")]
    media_type: String,
    manifests: Vec<IndexManifest>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IndexManifest {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    platform: Option<Platform>,
    size: u32,
    annotations: Annotations,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Platform {
    architecture: String,
    os: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Annotations {
    #[serde(rename = "io.containerd.image.name")]
    io_containerd_image_name: String,
    #[serde(rename = "org.opencontainers.image.ref.name")]
    org_opencontainers_image_ref_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestJson {
    #[serde(rename = "schemaVersion")]
    schema_version: i32,
    #[serde(rename = "mediaType")]
    media_type: String,
    config: ManifestConfig,
    layers: Vec<ManifestLayers>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestConfig {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    size: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestLayers {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    size: i32,
}
