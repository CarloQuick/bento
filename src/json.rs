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

fn create_bento_json<P: AsRef<Path>>(read_path: P, write_path: P) -> Result<()> {
    eprint!("{}", "Reading image config\n".blue());
    // Open the file in read-only mode with buffer.
    let file = File::open(read_path).expect("couldnt open");
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `Address`.
    let a: IncomingJson = serde_json::from_reader(reader)?;
    eprint!("Incoming Json: {:?}", a);
    let bento_config: BentoConfigJson = BentoConfigJson::make_bento_config(&a);
    eprint!("Bento Json: {:?}", bento_config);
    write_bento_config(write_path, bento_config)?;
    Ok(())
}

fn write_bento_config<P: AsRef<Path>>(write_path: P, bento: BentoConfigJson) -> Result<()> {
    eprint!("{}\n", "Creating and writing config.🍱".green());
    let file = File::create(write_path).expect("couldnt open");
    let mut writer = BufWriter::new(file);
    to_writer_pretty(&mut writer, &bento).unwrap();
    writer.flush().expect("Failed to flush the writer");
    eprint!("{}\n", "🎉 Bento finished 🎉".cyan());
    Ok(())
}

fn get_config_path(digest: &str) -> Option<PathBuf> {
    // let mut new_diff_ids: Vec<String> = Vec::with_capacity(rootfs.diff_ids.len());
    match digest.find(":") {
        None => None,
        Some(colon_index) => {
            let mut config_path = PathBuf::from("blobs");
            config_path.push(&digest[0..colon_index]);
            config_path.push(&digest[colon_index + 1..]);
            Some(config_path)
        }
    }
}

fn get_index_json(mut cont_path: PathBuf) -> Result<IndexJson> {
    cont_path.push("index.json");
    let file = File::open(&cont_path).expect("Couldnt open Index.json");
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let a: IndexJson = serde_json::from_reader(reader)?;
    Ok(a)
}

fn get_manifest_json(manifest_path: &PathBuf) -> Result<ManifestJson> {
    // cont_path.push("index.json");
    let file = File::open(&manifest_path).expect("Couldnt open Index.json");
    let reader = BufReader::new(file);
    // Read the JSON contents of the file as an instance of `Address`.
    let a: ManifestJson = serde_json::from_reader(reader)?;
    Ok(a)
}

///Users/c_quick/dev/containers/test_images/python
fn read_write_json() {
    let container_str = String::from("/Users/c_quick/dev/containers/test_images/python");
    let write_path: PathBuf =
        PathBuf::from("/Users/c_quick/dev/containers/test_images/python/bento_config.json");
    let cont_path: PathBuf = PathBuf::from(&container_str);
    let index_json = get_index_json(cont_path).expect("Could not read from index.json");
    let manifest_path = get_config_path(&index_json.manifests[0].digest);
    match manifest_path {
        None => panic!("No config"),
        Some(manifest) => {
            let please_remove = PathBuf::from(&container_str).join(&manifest);
            let manifest_json =
                get_manifest_json(&please_remove).expect("Couldnt get the manifest.json");
            let config_path = get_config_path(&manifest_json.config.digest);
            match config_path {
                None => panic!("No config"),
                Some(bento_config) => {
                    let please_remove = PathBuf::from(&container_str).join(&bento_config);
                    create_bento_json(please_remove, write_path)
                        .expect("Failed to create bento json");
                }
            }
        }
    }
}

fn _get_layer_dir(rootfs: &RootFs) -> RootFs {
    let mut new_diff_ids: Vec<String> = Vec::with_capacity(rootfs.diff_ids.len());
    for (_, val) in rootfs.diff_ids.iter().enumerate() {
        if let Some(colon_index) = val.find(":") {
            let mut layer_path = String::from("/blobs/");
            layer_path.push_str(&val[0..colon_index]);
            layer_path.push_str("/");
            layer_path.push_str(&val[colon_index + 1..]);
            new_diff_ids.push(layer_path);
        }
    }

    let updated_rootfs = RootFs {
        fs_type: rootfs.fs_type.clone(),
        diff_ids: new_diff_ids,
    };
    updated_rootfs
}
