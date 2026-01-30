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
use std::fs::File;
use std::io::ErrorKind;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::{env, fs};
use std::{process, time};

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
    let overlay_options = &overlay_options[..];
    let data = Some(overlay_options);

    mount(Some("overlay"), &bento_config.merge, fstype, flags, data).with_context(|| {
        format!(
            "Failed to Mount Filesystem at target {} .",
            &bento_config.merge.display()
        )
    })?;

    if !bento_config.mount.as_os_str().is_empty() {
        let bind_mount = &bento_config.merge.join(&bento_config.mount);
        if !bind_mount.exists() {
            fs::create_dir_all(&bind_mount)
                .with_context(|| format!("Failed to create mount at {}.", &bind_mount.display()))?;
        }

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
    }
    Ok(())
}

pub fn get_bento_config_path(name: &str) -> Result<PathBuf> {
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").context("Failed to get container path from .env")?;
    let bento_container_path = PathBuf::from(&bento_containers_env).join(name);
    let bento_config_path = bento_container_path.join("bento_config.json");
    Ok(bento_config_path)
}

fn fork_into_namespaces(bento_config: &BentoConfigJson, name: &str) -> Result<()> {
    //** Fork into the namespace **//
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child: _ }) => {
            return Ok(());
        }
        Ok(ForkResult::Child) => {
            // Creates need session with the child the session leader.

            if let Err(e) = setsid() {
                eprintln!(
                    "Setsid() failed to make the child process session leader: {}",
                    e
                );
                process::exit(1);
            }
            unshare(CloneFlags::CLONE_NEWPID).context("Failed to create a PID namespace.")?;
            //** UTS namespace **//
            unshare(CloneFlags::CLONE_NEWUTS).context("Failed to create uts namespace.")?;

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    let child_pid = child.as_raw();
                    json::update_container_status(name, Some(child_pid), json::State::Running)
                        .with_context(|| {
                            format!("Failed to change container {} status to Running.", &name)
                        })?;

                    waitpid(child, None).with_context(|| {
                        format!(
                            "Failed to recieve a change of signal from child process: {} .",
                            &child
                        )
                    })?;
                    json::update_container_status(name, None, json::State::Stopped).with_context(
                        || format!("Failed to change container {} status to Stopped.", &name),
                    )?;

                    unmount_and_clean_up(&bento_config)
                        .with_context(|| format!("Failed to unmount container {}.", &name))?;

                    return Ok(());
                }
                Ok(ForkResult::Child) => {
                    // Child process continues its work as a daemon
                    //** In the child: chroot into the prepared directory **//
                    chroot(&bento_config.merge).with_context(|| {
                        format!(
                            "Failed change root directory (chroot) at {}.",
                            &bento_config.merge.display()
                        )
                    })?;
                    match std::env::set_current_dir(&bento_config.cwd) {
                        Ok(()) => {
                            fs::create_dir_all("/proc").context(
                                "Failed to create /proc before mounting the process' proc.",
                            )?;
                            mount(
                                Some("proc"),
                                "/proc",
                                Some("proc"),
                                MsFlags::empty(),
                                None::<&[u8]>,
                            )
                            .context("Failed to Mount /proc.")?;
                            sethostname(name).context("Failed to set the hostname.")?;
                            let (path, args, env) = get_execve_params(bento_config)
                                .context("Faile to get PATH, ARGS, or ENV.")?;
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
    let mut args: Vec<CString> = Vec::new();
    let mut env: Vec<CString> = Vec::new();
    let cmds = if let Some(user_cmd) = &bento_config.user_cmd {
        user_cmd.clone()
    } else {
        bento_config.cmd.clone()
    };
    for arg in cmds.iter() {
        args.push(CString::new(arg.to_owned()).unwrap());
    }
    for e in bento_config.env.iter() {
        env.push(CString::new(e.to_owned()).unwrap());
    }
    let path =
        get_path_from_config(bento_config).context("Failed to get the bento config path.")?;

    let path = CString::new(path).context("Failed to convert PATH to CString.")?;
    Ok((path, args, env))
}

fn unmount_and_clean_up(bento_config: &BentoConfigJson) -> Result<()> {
    //** Unmount the container filesystem **//
    let merge = &bento_config.merge;
    let proc_path = &merge.join("proc");
    if proc_path.exists() {
        umount(proc_path)
            .with_context(|| format!("Failed to unmount proc path at {}.", proc_path.display()))?;
    }
    if !bento_config.mount.as_os_str().is_empty() {
        let bind_mount = &merge.join(&bento_config.mount);
        if bind_mount.exists() {
            umount(bind_mount).with_context(|| {
                format!("Failed to unmount bind mount at {}.", bind_mount.display())
            })?;
        }
    }
    if merge.exists() {
        umount(merge)
            .with_context(|| format!("Failed to unmount merge mount at {}.", merge.display()))?;
    }
    Ok(())
}
fn _clean_up(container_dir: &PathBuf) -> Result<()> {
    fs::remove_dir_all(container_dir)
        .with_context(|| format!("Failed to remove dir at {}.", container_dir.display()))?;

    Ok(())
}

pub fn start(name: &str) -> Result<()> {
    let bento_config_path =
        get_bento_config_path(name).context("Failed to get bento config path.")?;

    let bento_config = get_bento_config(&bento_config_path).with_context(|| {
        format!(
            "Container {} failed to load the bento_config.json at {}.",
            name,
            bento_config_path.display()
        )
    })?;
    unshare_user_namespace().with_context(|| {
        format!(
            "Container {} failed to go rootless by unsharing user namespace.",
            name
        )
    })?;
    unshare_mount_namespace()
        .with_context(|| format!("Container {} failed  to unshare mount namespace.", name))?;
    mount_fs_overlay(&bento_config)
        .with_context(|| format!("Container {} failed to unshare mount namespace.", name))?;
    fork_into_namespaces(&bento_config, name)
        .with_context(|| format!("Container {} faild to fork the process.", name))?;

    Ok(())
}

pub fn create(
    name: &String,
    image: &String,
    mount: &PathBuf,
    cwd: &PathBuf,
    user_cmd: &Option<Vec<String>>,
) -> Result<()> {
    let (image, name) = &format_create_params(name, image);
    let (bento_images_env, bento_containers_env) =
        &get_bento_envs().context("Failed to bento environmental variables.")?;

    let (new_bento_image_path, new_bento_container_path) =
        &create_container_dirs(bento_images_env, bento_containers_env, name, image);

    if let Err(e) = unpack_image(image, bento_images_env, new_bento_image_path) {
        rollback_dirs(vec![new_bento_image_path, new_bento_container_path]);
        panic!("Error: {}. unpacking image.", e)
    }
    // must return a result
    let (container_name, created_container_path) = match json::create_bento_config(
        name,
        &new_bento_image_path,
        &new_bento_container_path,
        mount,
        cwd,
        user_cmd,
    ) {
        Ok((cont_name, cont_path)) => (cont_name, cont_path),
        Err(e) => {
            rollback_dirs(vec![new_bento_image_path, new_bento_container_path]);
            panic!(
                "Error create bento config: {}. Failed to create container {}.",
                e, name
            );
        }
    };

    if let Err(e) = json::add_to_container_manifest(&container_name, &created_container_path) {
        rollback_container_manifest(&container_name).with_context(|| {
            format!(
                "Failed to rollback container manifest for {} on error: {}.",
                container_name, e
            )
        })?;
    }
    Ok(())
}

fn apply_signal(pid: Pid, signal: Signal) -> Result<()> {
    kill(pid, signal)
        .with_context(|| format!("Failed to kill pid {} with signal {}", pid, signal))?;
    Ok(())
}

pub fn stop(name: &str, container: &Container, env: &Env) -> Result<()> {
    if let Some(c_pid) = container.pid {
        let pid = Pid::from_raw(c_pid);
        match apply_signal(pid, Signal::SIGTERM) {
            Ok(()) => {
                for i in 1..=10 {
                    if let Some(c) = json::check_existing_container(name, env) {
                        match c.state {
                            State::Stopped => return Ok(()),
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
}

pub fn kill_proc(container: &Container) -> Result<()> {
    if let Some(c_pid) = container.pid {
        let pid = Pid::from_raw(c_pid);
        match apply_signal(pid, Signal::SIGKILL) {
            Ok(()) => return Ok(()),
            Err(e) => return Err(e),
        }
    } else {
        return Err(anyhow!(ErrorKind::NotFound));
    };
}

pub fn exec(name: &String, container: &Container, cmd: &String, args: &Vec<CString>) -> Result<()> {
    if let Some(pid) = container.pid {
        let container_proc: PathBuf = PathBuf::from("/proc").join(&pid.to_string()).join("ns");
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
        setns(borrowed_user_fd, CloneFlags::CLONE_NEWUSER)
            .context("Failed to setns for user namespace")?;
        setns(borrowed_mount_fd, CloneFlags::CLONE_NEWNS)
            .context("Failed to setns for mount namespace")?;
        setns(borrowed_pid_fd, CloneFlags::CLONE_NEWPID)
            .context("Failed to setns for pid namespace")?;
        setns(borrowed_uts_fd, CloneFlags::CLONE_NEWUTS)
            .context("Failed to setns for uts namespace")?;
        let bento_config_path = get_bento_config_path(name)
            .with_context(|| format!("Container {} failed to get the bento config path", name))?;
        let bento_config = get_bento_config(&bento_config_path).with_context(|| {
            format!(
                "Container {} failed to get the bento config at {}",
                name,
                &bento_config_path.display()
            )
        })?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                waitpid(child, None).with_context(|| {
                    format!(
                        "Container {} failed to wait for signal change in pid {}",
                        name, &pid
                    )
                })?;
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
    let image = hyphen_for_colon(image);
    let name = hyphen_for_colon(name);
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
    bento_images_env: &String,
    bento_containers_env: &String,
    name: &String,
    image: &String,
) -> (PathBuf, PathBuf) {
    let new_bento_image_path = PathBuf::from(&bento_images_env).join(image);
    let new_bento_container_path = PathBuf::from(&bento_containers_env).join(name);
    if let Err(create_error) = fs::create_dir_all(&new_bento_image_path) {
        if create_error.kind() == ErrorKind::AlreadyExists {
            panic!("File already exists at: {:?}", new_bento_image_path);
        } else {
            println!("Rolling back bento_image dirs");
            rollback_dirs(vec![&new_bento_image_path]);
        }
        panic!("Error: {}", create_error);
    } else {
        if let Err(create_error) = fs::create_dir_all(&new_bento_container_path) {
            if create_error.kind() == ErrorKind::AlreadyExists {
                panic!("File already exists at: {:?}", new_bento_container_path);
            } else {
                println!("Rolling back bento_image dirs");
                rollback_dirs(vec![&new_bento_image_path, &new_bento_container_path]);
            }

            panic!("Error: {}", create_error);
        }
    }
    (new_bento_image_path, new_bento_container_path)
}

fn rollback_dirs(dirs: Vec<&PathBuf>) {
    for dir in dirs.iter() {
        if let Err(remove_error) = fs::remove_dir(dir) {
            eprintln!("Error: {}. removing failed Image directory", remove_error)
        } else {
            eprintln!("Removed {:?} after failed execution.", dir);
        }
    }
}

fn unpack_image(
    image: &String,
    bento_images_env: &String,
    bento_image_path: &PathBuf,
) -> Result<(), std::io::Error> {
    let mut tar = String::from(image);
    tar.push_str(".tar");
    let image_tar_path = PathBuf::from(&bento_images_env).join(&tar);
    let res = extract::unpack_archive(&image_tar_path, &bento_image_path);
    res
}

pub fn hyphen_for_colon(image: &String) -> String {
    let str = image.replace(":", "-");
    str
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_image_colon() {
        let image = String::from("python:trixie");
        let result = hyphen_for_colon(&image);
        let new_image = String::from("python-trixie");
        assert_eq!(result, new_image);
    }
}
