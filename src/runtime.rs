use crate::config::{BentoConfigJson, get_bento_config};
use crate::env::Env;
use crate::json::{Container, State, rollback_container_manifest};
use crate::{extract, json};
use anyhow::{Context, Result, anyhow};
use nix::mount::{mount, umount};
use nix::sched::setns;
use nix::sys::signal::Signal;
use nix::sys::signal::kill;
use nix::sys::wait::waitpid;
use nix::unistd::{ForkResult, execve, fork, sethostname, setsid};
use nix::{mount::MsFlags, unistd::chroot};
use nix::{
    sched::{CloneFlags, unshare},
    unistd::{Pid, getpid},
};
use std::ffi::CString;
use std::fs::{File, create_dir_all, set_permissions};
use std::io::ErrorKind;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::{env, fs};
use std::{process, time};
use tracing::{debug, error, info};
use walkdir::WalkDir;

fn unshare_user_namespace() -> Result<()> {
    let host_uid = nix::unistd::getuid();
    let host_gid = nix::unistd::getgid();
    let uid_map = format!("0 {} 1", host_uid);
    let gid_map = format!("0 {} 1", host_gid);
    unshare(CloneFlags::CLONE_NEWUSER).context("Failed to create user namespace.")?;

    std::fs::write("/proc/self/uid_map", uid_map).context("Failed to write to uid.")?;

    let pid = getpid();
    let pid = pid.as_raw();
    let path = PathBuf::from("/proc")
        .join(pid.to_string())
        .join("setgroups");
    std::fs::write(path, "deny").context("Failed to write to gid.")?;

    std::fs::write("/proc/self/gid_map", gid_map).context("Failed to write to gid.")?;

    Ok(())
}
fn unshare_mount_namespace() -> Result<()> {
    // //** Create mount namespace (isolates your filesystem operations) **//
    unshare(CloneFlags::CLONE_NEWNS).context("Failed to create a mounted namespace.")?;

    Ok(())
}
fn mount_fs_overlay(bento_config: &BentoConfigJson) -> Result<()> {
    let mut lowerdir = String::new();
    for (i, dir) in bento_config.lowerdir.iter().enumerate() {
        debug!("Adding: {} to lowerdir String.", dir);
        if i == bento_config.lowerdir.len() - 1 {
            lowerdir.push_str(dir);
        } else {
            lowerdir.push_str(dir);
            lowerdir.push_str(":");
        }
    }

    let fstype = Some("overlay");
    let flags = MsFlags::empty();

    let overlay_options = format!(
        "lowerdir={},upperdir={},workdir={}",
        lowerdir,
        bento_config.upperdir.display(),
        bento_config.workdir.display()
    );
    debug!("Overlayfs optinos: {}", overlay_options);
    let overlay_options = &overlay_options[..];
    let data = Some(overlay_options);

    debug!("Mounting merge fs at: {:?}", bento_config.merge);
    mount(Some("overlay"), &bento_config.merge, fstype, flags, data).with_context(|| {
        format!(
            "Failed to Mount Filesystem at target {} .",
            &bento_config.merge.display()
        )
    })?;
    debug!("Successful [merge dir] mount operation.");
    if !bento_config.mount.as_os_str().is_empty() {
        let bind_mount = &bento_config.merge.join(&bento_config.mount);
        if !bind_mount.exists() {
            debug!("Creating mount dir at: {:?}", bind_mount);
            fs::create_dir_all(&bind_mount)
                .with_context(|| format!("Failed to create mount at {}.", &bind_mount.display()))?;
        }
        debug!("Attempting to mount mount dir at {:?}", bind_mount);

        mount(
            Some(&bento_config.mount),
            bind_mount,
            None::<&Path>,
            MsFlags::MS_BIND,
            None::<&[u8]>,
        )
        .with_context(|| {
            format!(
                "Failed to Mount USER Filesystem at {}",
                &bento_config.mount.display()
            )
        })?;
        debug!("Successful [mount dir] mount operation.");
    }
    Ok(())
}

fn fork_into_namespaces(
    bento_config: &BentoConfigJson,
    name: &str,
    container_manifest_path: &PathBuf,
) -> Result<()> {
    //** Fork into the namespace **//
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child: _ }) => {
            debug!("Grandparent process exits.");
            return Ok(());
        }
        Ok(ForkResult::Child) => {
            debug!("First child / parent process attempts to set the session id.");
            if let Err(e) = setsid() {
                error!(
                    "Setsid() failed to make the child process session leader: {}",
                    e
                );
                process::exit(1);
            }
            debug!("First child / parent process setsid() successful.");
            debug!("First child / parent process unsharing pid namespace.");
            unshare(CloneFlags::CLONE_NEWPID).context("Failed to create a PID namespace.")?;
            //** UTS namespace **//
            debug!("First child / parent process unsharing uts namespace.");
            unshare(CloneFlags::CLONE_NEWUTS).context("Failed to create uts namespace.")?;

            debug!("Attempting second fork.");
            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    let child_pid = child.as_raw();
                    json::update_container_status(
                        name,
                        Some(child_pid),
                        json::State::Running,
                        container_manifest_path,
                    )?;

                    debug!("Parent [1st Child] waiting for [2nd Child] process signal.");
                    waitpid(child, None).with_context(|| {
                        format!(
                            "Parent [1st Child] Failed to recieve a change of signal from child process: {} .",
                            &child
                        )
                    })?;
                    json::update_container_status(
                        name,
                        None,
                        json::State::Stopped,
                        container_manifest_path,
                    )
                    .with_context(|| {
                        format!(
                            "Parent [1st Child] Failed to change container {} status to Stopped.",
                            &name
                        )
                    })?;

                    unmount_and_clean_up(&bento_config).with_context(|| {
                        format!("Parent [1st Child] Failed to unmount container {}.", &name)
                    })?;

                    return Ok(());
                }
                Ok(ForkResult::Child) => {
                    debug!("[2nd Child] process working as a daemon.");
                    // Child process continues its work as a daemon
                    //** In the child: chroot into the prepared directory **//
                    debug!("[2nd Child] chrooting to {:?}", &bento_config.merge);
                    chroot(&bento_config.merge).with_context(|| {
                        format!(
                            "[2nd Child] Failed to change root directory to {:?}.",
                            &bento_config.merge
                        )
                    })?;
                    debug!("[2nd Child] setting cwd to {:?}", &bento_config.cwd);
                    match std::env::set_current_dir(&bento_config.cwd) {
                        Ok(()) => {
                            debug!("[2nd Child] mounting /proc");
                            fs::create_dir_all("/proc").context(
                                "[2nd Child] Failed to create /proc before mounting the process' proc.",
                            )?;
                            mount(
                                Some("proc"),
                                "/proc",
                                Some("proc"),
                                MsFlags::empty(),
                                None::<&[u8]>,
                            )
                            .context("[2nd Child] Failed to Mount /proc.")?;
                            debug!("[2nd Child] setting hostname: {}", name);
                            sethostname(name).context("[2nd Child] Failed to set the hostname.")?;
                            let (path, args, env) = get_execve_params(bento_config)
                                .context("[2nd Child] Failed to get PATH, ARGS, or ENV.")?;

                            debug!("[2nd Child] attempting execve.");
                            match execve(&path, &args, &env) {
                                Err(e) => {
                                    println!("execve failed: {}", e);
                                    process::exit(1);
                                }
                            }
                        }
                        Err(e) => {
                            println!("Failed to set the container working directory: {}", e);
                            process::exit(1);
                        }
                    }
                }
                Err(e) => return Err(anyhow!("Failed to fork the process: {}", e)),
            }
        }
        Err(e) => return Err(anyhow!("Failed to fork the process: {}", e)),
    }
}

fn get_path_from_config(bento_config: &BentoConfigJson) -> Result<String> {
    let mut path = String::new();
    let cmds = if let Some(user_cmd) = &bento_config.user_cmd {
        user_cmd.clone()
    } else {
        bento_config.cmd.clone()
    };
    match cmds.get(0) {
        Some(cmd) => {
            if cmd.contains("/") {
                // hunt in the provided paths
                path.push_str(cmd);
            } else {
                let env_v = get_executable_paths(&bento_config.env)
                    .context("Failed to get env from bento config.")?;
                for e in env_v.iter() {
                    if Path::new(e).join(cmd).is_file() {
                        match PathBuf::from(e).join(cmd).to_str() {
                            Some(p) => {
                                path.push_str(p);
                                break;
                            }
                            None => {
                                return Err(anyhow!("Failed to get the pathbuf as a string"));
                            }
                        }
                    }
                }
                if path.is_empty() {
                    return Err(anyhow!(
                        "Failed to find a suitable path for the command: {}",
                        cmd
                    ));
                }
            }
        }
        None => return Err(anyhow!("No commands available in this config or image.")),
    }
    Ok(path)
}

fn get_path_from_cmd(
    cmd: &String,
    args: &Vec<CString>,
    bento_config: &BentoConfigJson,
) -> Result<(CString, Vec<CString>, Vec<CString>)> {
    let mut env: Vec<CString> = Vec::new();
    let mut arg_v: Vec<CString> = Vec::new();
    let cmd_c_str = CString::new(cmd.to_owned())
        .with_context(|| format!("Failed to convert cmd: {} to a CString.", &cmd))?;
    arg_v.push(cmd_c_str);
    for arg in args.iter() {
        arg_v.push(
            CString::new(arg.to_owned())
                .with_context(|| format!("Failed to convert arg: {:?} to a CString.", &arg))?,
        );
    }
    for e in bento_config.env.iter() {
        env.push(
            CString::new(e.to_owned())
                .with_context(|| format!("Failed to convert arg: {} to a CString.", &e))?,
        );
    }
    let mut path = String::new();

    if cmd.contains("/") {
        // hunt in the provided paths
        path.push_str(cmd);
    } else {
        let env_v = get_executable_paths(&bento_config.env).with_context(|| {
            format!(
                "Failed to convert arg: {:?} to a CString.",
                &bento_config.env
            )
        })?;
        for e in env_v.iter() {
            if Path::new(e).join(cmd).is_file() {
                match PathBuf::from(e).join(cmd).to_str() {
                    Some(p) => {
                        path.push_str(p);
                        break;
                    }
                    None => return Err(anyhow!("Failed to convert exec pathbuf to string.")),
                }
            }
        }
    }
    Ok((
        CString::new(path).context("Failed to convert PATH to CString.")?,
        arg_v,
        env,
    ))
}

fn get_execve_params(
    bento_config: &BentoConfigJson,
) -> Result<(CString, Vec<CString>, Vec<CString>)> {
    debug!("Getting execve params.");

    let mut args: Vec<CString> = Vec::new();
    let mut env: Vec<CString> = Vec::new();
    let cmds = if let Some(user_cmd) = &bento_config.user_cmd {
        user_cmd.clone()
    } else {
        bento_config.cmd.clone()
    };
    for arg in cmds.iter() {
        args.push(CString::new(arg.to_owned())?);
    }
    for e in bento_config.env.iter() {
        env.push(CString::new(e.to_owned())?);
    }
    let path =
        get_path_from_config(bento_config).context("Failed to get the bento config path.")?;

    let path = CString::new(path).context("Failed to convert PATH to CString.")?;
    debug!(
        "Results of getting execve params from bento config\n\tpath: {:?}\n\targs: {:#?}\n\tenv: {:#?}",
        path, args, env
    );
    Ok((path, args, env))
}

fn unmount_and_clean_up(bento_config: &BentoConfigJson) -> Result<()> {
    debug!("Unmounting and cleaning up!");
    //** Unmount the container filesystem **//
    let merge = &bento_config.merge;
    let proc_path = &merge.join("proc");
    if proc_path.exists() {
        debug!("Proc path found at: {:?}", proc_path);
        umount(proc_path)
            .with_context(|| format!("Failed to unmount proc path at {}.", proc_path.display()))?;
    }
    if !bento_config.mount.as_os_str().is_empty() {
        let bind_mount = &merge.join(&bento_config.mount);
        if bind_mount.exists() {
            debug!("Unmounting bind mount at {:?}", bind_mount);
            umount(bind_mount).with_context(|| {
                format!("Failed to unmount bind mount at {}.", bind_mount.display())
            })?;
        }
    }
    debug!("Looking for merge dir.");
    if merge.exists() {
        debug!("Unmounting merge at {:?}", merge);
        umount(merge)
            .with_context(|| format!("Failed to unmount merge mount at {}.", merge.display()))?;
    }
    Ok(())
}

pub fn start(container: &Container, name: &str, env: &Env) -> Result<()> {
    debug!("Container's state: {:?}", container.state);
    if container.state == State::Created || container.state == State::Stopped {
        debug!("Container eligible to start.");
        let container_config_path = &env
            .bento_containers_env_path
            .join(name)
            .join("bento_config.json");

        let bento_config = get_bento_config(&container_config_path).with_context(|| {
            format!(
                "Container {} failed to load the bento_config.json at {}.",
                name,
                container_config_path.display()
            )
        })?;
        debug!("Contents of bento config: \n{:#?}", bento_config);

        debug!("Unsharing user namespace");
        unshare_user_namespace().with_context(|| {
            format!(
                "Container {} failed to go rootless by unsharing user namespace.",
                name
            )
        })?;
        debug!("Unsharing mount namespace");
        unshare_mount_namespace()
            .with_context(|| format!("Container {} failed  to unshare mount namespace.", name))?;
        if let Err(err) = mount_fs_overlay(&bento_config) {
            error!("Failed to mount overlayfs");
            unmount_and_clean_up(&bento_config)
                .context("Failed to unmount and cleantup after failed start.")?;
            return Err(anyhow!("Err: {}", err));
        };
        debug!("Forking the process.");
        fork_into_namespaces(
            &bento_config,
            name,
            &env.bento_containers_env_path
                .join("container_manifest.json"),
        )
        .with_context(|| format!("Container {} failed to fork the process.", name))?;
    } else {
        anyhow::bail!("Container failed to start! Check its status.");
    }
    Ok(())
}

pub fn create(
    name: &String,
    image: &String,
    mount: &PathBuf,
    cwd: &PathBuf,
    user_cmd: &Option<Vec<String>>,
    env: &Env,
) -> Result<()> {
    debug!("Creating path to new container at: {:?}.", &env.bento_dir);
    create_dir_all(&env.bento_dir).with_context(|| {
        format!(
            "Create failed to access or create a new base container directory at {:?}.",
            env.bento_dir
        )
    })?;

    let (image, name) = &format_create_params(name, image);

    let (new_bento_image_path, new_bento_container_path) = &create_container_dirs(
        &env.bento_image_env_path,
        &env.bento_containers_env_path,
        name,
        image,
    )
    .with_context(|| {
        format!(
            "Faieled to create container directories at:\n\t{:?} / {:?}",
            &env.bento_image_env_path, &env.bento_containers_env_path
        )
    })?;

    if let Err(err) = unpack_image(image, &env.bento_image_env_path, new_bento_image_path) {
        rollback_dirs(vec![new_bento_image_path, new_bento_container_path]).with_context(|| {
            format!(
                "Failed to rollback unpacked images and container folders at {:?}\n\t{}.",
                vec![new_bento_image_path, new_bento_container_path],
                err
            )
        })?;
        return Err(anyhow!("Failed to unpack the image: {}.", image));
    }

    let (container_name, created_container_path) = match json::create_bento_config(
        name,
        &new_bento_image_path,
        &new_bento_container_path,
        mount,
        cwd,
        user_cmd,
    ) {
        Ok((cont_name, cont_path)) => (cont_name, cont_path),
        Err(err) => {
            rollback_dirs(vec![new_bento_image_path, new_bento_container_path]).with_context(
                || {
                    format!(
                        "Failed to rollback unpacked images and container folders at {:?}\n\t{}",
                        vec![new_bento_image_path, new_bento_container_path],
                        err
                    )
                },
            )?;
            return Err(anyhow!("Container not found to update."));
        }
    };

    if let Err(e) = json::add_to_container_manifest(
        &container_name,
        &created_container_path,
        &env.bento_containers_env_path,
    ) {
        rollback_container_manifest(&container_name, &env.bento_containers_env_path).with_context(
            || {
                format!(
                    "Failed to rollback container manifest for {} on error: {}.",
                    container_name, e
                )
            },
        )?;
    }
    Ok(())
}

fn apply_signal(pid: Pid, signal: Signal) -> Result<()> {
    kill(pid, signal)
        .with_context(|| format!("Failed to kill pid {} with signal {}", pid, signal))?;
    Ok(())
}

pub fn stop(container: &Container, name: &str, env: &Env) -> Result<()> {
    debug!("Container's state: {:?}", container.state);
    if container.state == State::Running {
        if let Some(c_pid) = container.pid {
            debug!("Running container's pid: {}", c_pid);
            let pid = Pid::from_raw(c_pid);
            debug!("Applying SIGTERM signal.");
            match apply_signal(pid, Signal::SIGTERM) {
                Ok(()) => {
                    debug!("Running out 10 seconds.");
                    for i in 1..=10 {
                        if let Some(c) =
                            json::check_existing_container(name, &env.bento_containers_env_path)
                        {
                            match c.state {
                                State::Stopped => {
                                    debug!("Container effectively stopped at: {}s.", i);
                                    return Ok(());
                                }
                                _ => {
                                    if i < 10 {
                                        thread::sleep(time::Duration::from_secs(1));
                                    } else {
                                        apply_signal(pid, Signal::SIGKILL).with_context(|| {
                                            format!("Failed to apply SIGKILL to pid {}", pid)
                                        })?;
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(200));
                    return Ok(());
                }
                Err(e) => return Err(e),
            }
        } else {
            return Err(anyhow!(ErrorKind::NotFound));
        };
    } else {
        anyhow::bail!("Container not currently running.");
    }
}

pub fn kill_container(container: &Container) -> Result<()> {
    debug!("Container's state: {:?}", container.state);
    if container.state == State::Running {
        if let Some(c_pid) = container.pid {
            debug!("Running container's pid: {}", c_pid);
            let pid = Pid::from_raw(c_pid);
            debug!("Applying SIGKILL signal.");
            match apply_signal(pid, Signal::SIGKILL) {
                Ok(()) => return Ok(()),
                Err(e) => return Err(e),
            }
        } else {
            error!("Container does not have a functional PID");
            return Err(anyhow!(ErrorKind::NotFound));
        };
    } else {
        anyhow::bail!("Container not currently running.");
    }
}

pub fn exec(
    name: &String,
    container: &Container,
    cmd: &String,
    args: &Vec<CString>,
    env: &Env,
) -> Result<()> {
    if let Some(pid) = container.pid {
        debug!("Container has active PID of {}", pid);
        let container_proc: PathBuf = PathBuf::from("/proc").join(&pid.to_string()).join("ns");
        debug!("Looking inside {:?}", container_proc);
        if !container_proc.exists() {
            return Err(anyhow!("Container process {} not found in /proc", pid));
        }
        let user_ns: PathBuf = PathBuf::from(&container_proc).join("user");
        let mount_ns: PathBuf = PathBuf::from(&container_proc).join("mnt");
        let pid_ns: PathBuf = PathBuf::from(&container_proc).join("pid");
        let uts_ns: PathBuf = PathBuf::from(&container_proc).join("uts");

        if !user_ns.exists() {
            return Err(anyhow!(
                "Container process {} not found in /proc//user",
                pid
            ));
        }
        if !mount_ns.exists() {
            return Err(anyhow!("Container process {} not found in /proc//mnt", pid));
        }
        if !pid_ns.exists() {
            return Err(anyhow!("Container process {} not found in proc//pid", pid));
        }
        if !uts_ns.exists() {
            return Err(anyhow!("Container process {} not found in /proc//uts", pid));
        }
        let user_ns_file = File::open(user_ns).context("Failed to open namespace")?;
        let mount_ns_file = File::open(mount_ns).context("Failed to open namespace")?;
        let pid_ns_file = File::open(pid_ns).context("Failed to open namespace")?;
        let uts_ns_file = File::open(uts_ns).context("Failed to open namespace")?;

        let borrowed_user_fd: BorrowedFd<'_> = user_ns_file.as_fd();
        let borrowed_mount_fd: BorrowedFd<'_> = mount_ns_file.as_fd();
        let borrowed_pid_fd: BorrowedFd<'_> = pid_ns_file.as_fd();
        let borrowed_uts_fd: BorrowedFd<'_> = uts_ns_file.as_fd();

        debug!("Attempting setns into container.");
        setns(borrowed_user_fd, CloneFlags::CLONE_NEWUSER)
            .context("Failed to setns for user namespace")?;
        setns(borrowed_mount_fd, CloneFlags::CLONE_NEWNS)
            .context("Failed to setns for mount namespace")?;
        setns(borrowed_pid_fd, CloneFlags::CLONE_NEWPID)
            .context("Failed to setns for pid namespace")?;
        setns(borrowed_uts_fd, CloneFlags::CLONE_NEWUTS)
            .context("Failed to setns for uts namespace")?;

        let container_config_path = env
            .bento_containers_env_path
            .join(name)
            .join("bento_config.json");
        debug!("Container config located at: {:?}", container_config_path);
        let bento_config = get_bento_config(&container_config_path).with_context(|| {
            format!(
                "Container {} failed to get the bento config at {}",
                name,
                &container_config_path.display()
            )
        })?;
        debug!("Contents of bento config: \n{:#?}", bento_config);

        debug!("Forking into process at PID: {}", pid);
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                waitpid(child, None).with_context(|| {
                    format!(
                        "Container {} failed to wait for signal change in pid {}",
                        name, &pid
                    )
                })?;
                debug!("Exiting grandparent process.");
                return Ok(());
            }
            Ok(ForkResult::Child) => match chroot(&bento_config.merge) {
                Ok(()) => match std::env::set_current_dir(&bento_config.cwd) {
                    Ok(()) => match get_path_from_cmd(cmd, args, &bento_config) {
                        Ok((path, args, env)) => match execve(&path, &args, &env) {
                            Err(e) => {
                                eprintln!("execve failed: {}", e);
                                process::exit(1);
                            }
                        },
                        Err(e) => {
                            println!("failed to get path {}", e);
                            process::exit(1);
                        }
                    },
                    Err(e) => {
                        println!("failed to set current dir: {}", e);
                        process::exit(1);
                    }
                },
                Err(e) => {
                    println!("chroot failed: {}", e);
                    process::exit(1);
                }
            },
            Err(e) => return Err(anyhow!("Failed to fork the exec process: {}", e)),
        }
    } else {
        return Err(anyhow!(ErrorKind::NotFound));
    }
}
pub fn delete(container: &Container, name: &str, force: bool) -> Result<()> {
    debug!("Container's state: {:?}", container.state);
    match container.state {
        State::Created | State::Stopped => {
            rollback_dirs(vec![&PathBuf::from(&container.dir)])?;
            // rollback_container_manifest(name, &PathBuf::from(&container.dir))?;
        }
        State::Creating | State::Running => {
            if force {
                match container.pid {
                    // kill
                    Some(pid) => match apply_signal(Pid::from_raw(pid), Signal::SIGKILL) {
                        Ok(()) => {
                            rollback_dirs(vec![&PathBuf::from(&container.dir)])?;
                            rollback_container_manifest(name, &PathBuf::from(&container.dir))?;
                        }
                        Err(err) => {
                            return Err(anyhow!(
                                "Failed to delete due to missing pid: {:#?}\n\t{}",
                                container,
                                err
                            ));
                        }
                    },
                    None => {
                        return Err(anyhow!(
                            "Failed to delete due to missing pid: {:#?}",
                            container
                        ));
                    }
                }
            } else {
                return Err(anyhow!(
                    "Failed to delete due to incompatible container state: {:#?}",
                    container.state
                ));
            }
        }
    }
    Ok(())
}

pub fn get_executable_paths(env: &Vec<String>) -> Result<Vec<&str>> {
    let index = get_path_index(env);
    let v: Vec<&str> = env[index].split(":").collect();
    Ok(v)
}
pub fn get_path_index(env: &Vec<String>) -> usize {
    for (_, e) in env.iter().enumerate() {
        match e.find("PATH") {
            None => continue,
            Some(inx) => return inx,
        }
    }
    panic!("Failed to find PATH in config");
}

fn format_create_params(name: &String, image: &String) -> (String, String) {
    debug!("Removing `:` from name and image.");
    debug!("Original name: {} | Original image: {}", name, image);
    let image = hyphen_for_colon(image);
    let name = hyphen_for_colon(name);
    debug!("Returned name: {} | Returned image: {}", name, image);
    (image, name)
}
pub fn get_bento_envs() -> Result<(String, String)> {
    let bento_images_env: String =
        env::var("BENTO_IMAGES_PATH").context("Failed to get images path from .env")?;

    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").context("Failed to get container path from .env")?;

    Ok((bento_images_env, bento_containers_env))
}

fn create_container_dirs(
    bento_images_env: &PathBuf,
    bento_containers_env: &PathBuf,
    name: &String,
    image: &String,
) -> Result<(PathBuf, PathBuf)> {
    let new_bento_image_path = PathBuf::from(&bento_images_env).join(image);
    let new_bento_container_path = PathBuf::from(&bento_containers_env).join(name);
    debug!(
        "Creating bento image and container dirs:\n\timage path: {:?}\n\tcontainer path: {:?}",
        new_bento_image_path, new_bento_container_path
    );
    if let Err(create_error) = fs::create_dir_all(&new_bento_image_path) {
        error!("Error creating image dir. Attempting rollback of bento image dir.");
        rollback_dirs(vec![&new_bento_image_path]).with_context(|| {
            format!(
                "Failed to rollback directories from {:?}\n\t{}",
                vec![&new_bento_image_path],
                create_error
            )
        })?;
    } else {
        if let Err(create_error) = fs::create_dir_all(&new_bento_container_path) {
            error!(
                "Error creating image dir. Attempting rollback of both image and container dirs."
            );
            rollback_dirs(vec![&new_bento_image_path, &new_bento_container_path]).with_context(
                || {
                    format!(
                        "Failed to rollback directories from {:?}\n\t{}",
                        vec![&new_bento_image_path, &new_bento_container_path],
                        create_error
                    )
                },
            )?;
        }
    }
    Ok((new_bento_image_path, new_bento_container_path))
}

fn set_dir_permissions(path: &PathBuf) -> Result<()> {
    for entry in WalkDir::new(path) {
        let e = entry?;
        let p = e.path();
        debug!("Walking: {:?}", path);
        let metadata = e.metadata()?;
        let mut permissions = metadata.permissions();
        debug!("setting permissions for: {:#?}", permissions);
        permissions.set_mode(0o040775);
        set_permissions(p, permissions.clone())?;
        debug!("permissions after: {:#?}", permissions);
    }

    Ok(())
}

pub fn rollback_dirs(dirs: Vec<&PathBuf>) -> Result<()> {
    debug!("Dirs to rollback: {:#?}", dirs);
    for dir in dirs.iter() {
        debug!("Attempting to remove: {:?}", dir);
        set_dir_permissions(&dir)?;
        if let Err(remove_error) = fs::remove_dir_all(dir) {
            return Err(anyhow!(
                "Error: {}. removing failed Image directory",
                remove_error
            ));
        } else {
            info!("Removed {:?}.", dir);
        }
    }
    debug!("Successfully rolledback dirs");
    Ok(())
}

fn unpack_image(
    image: &String,
    bento_image_env_path: &PathBuf,
    bento_image_path: &PathBuf,
) -> Result<()> {
    let mut tar = String::from(image);
    tar.push_str(".tar");
    let image_tar_path = PathBuf::from(&bento_image_env_path).join(&tar);
    debug!("Unpacking image tar at: {:?}", image_tar_path);
    let res = extract::unpack_archive(&image_tar_path, &bento_image_path);
    res
}

pub fn hyphen_for_colon(image: &String) -> String {
    let str = image.replace(":", "-");
    str
}

#[cfg(test)]
mod tests {
    use std::fs::remove_dir_all;

    use super::*;

    #[test]
    fn remove_image_colon() {
        let image = String::from("python:trixie");
        let result = hyphen_for_colon(&image);
        let new_image = String::from("python-trixie");
        assert_eq!(result, new_image);
    }
    #[test]
    fn rolling_back_dirs() -> Result<()> {
        let imgs_dir: PathBuf = PathBuf::from("/tmp/images");
        let containers_dir: PathBuf = PathBuf::from("/tmp/containers");
        let _ = remove_dir_all(&imgs_dir);
        let _ = remove_dir_all(&containers_dir);

        create_dir_all(&imgs_dir).expect("Failed to create test image dir");
        create_dir_all(&containers_dir).expect("Failed to create test image dir");

        assert_eq!(imgs_dir.exists(), true);
        assert_eq!(containers_dir.exists(), true);

        rollback_dirs(vec![&imgs_dir, &containers_dir]).with_context(|| {
            format!(
                "Failed to rollback directories {:?}",
                vec![&imgs_dir, &containers_dir]
            )
        })?;

        assert_eq!(!imgs_dir.exists(), true);
        assert_eq!(!containers_dir.exists(), true);
        Ok(())
    }
    #[test]
    fn container_limited_from_starting_by_state() {
        let container: Container = Container {
            dir: String::from("temp"),
            state: State::Running,
            pid: None,
        };
        let env = Env {
            bento_dir: PathBuf::from("/test_dir"),
            bento_image_env_path: PathBuf::from("/test_image_path"),
            bento_containers_env_path: PathBuf::from("/test_container_path"),
        };

        let result = start(&container, &"container_name", &env);

        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(
            error.to_string(),
            "Container failed to start! Check its status."
        );
    }
    #[test]
    fn container_must_be_running_to_stop() {
        let container: Container = Container {
            dir: String::from("temp"),
            state: State::Stopped,
            pid: None,
        };
        let env = Env {
            bento_dir: PathBuf::from("/test_dir"),
            bento_image_env_path: PathBuf::from("/test_image_path"),
            bento_containers_env_path: PathBuf::from("/test_container_path"),
        };

        let result = stop(&container, &"container_name", &env);

        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "Container not currently running.");
    }
    #[test]
    fn container_must_be_running_to_kill() {
        let container: Container = Container {
            dir: String::from("temp"),
            state: State::Stopped,
            pid: None,
        };

        let result = kill_container(&container);

        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "Container not currently running.");
    }
}
