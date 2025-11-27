use crate::config::{BentoConfigJson, get_bento_config};
use crate::json::{Container, State};
use crate::{extract, json};
use anyhow::{Error, Result, anyhow};
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

fn write_to_gid_setgroup() {
    let pid = getpid();
    let pid = pid.as_raw();
    let mut path = PathBuf::new(); // Create an empty PathBuf
    path.push("/proc");
    path.push(pid.to_string());
    // path.push("self");
    path.push("setgroups");
    std::fs::write(path, "deny").expect("Failed to write to gid");
}

fn unshare_user_namespace() {
    let host_uid = nix::unistd::getuid();
    let host_gid = nix::unistd::getgid();
    let uid_map = format!("0 {} 1", host_uid);
    let gid_map = format!("0 {} 1", host_gid);
    unshare(CloneFlags::CLONE_NEWUSER).expect("Failed to create user namespace");

    std::fs::write("/proc/self/uid_map", uid_map).expect("Failed to write to uid");
    write_to_gid_setgroup();
    std::fs::write("/proc/self/gid_map", gid_map).expect("Failed to write to gid");
}
fn unshare_mount_namespace() {
    // //** Create mount namespace (isolates your filesystem operations) **//
    unshare(CloneFlags::CLONE_NEWNS).expect("Failed to create a mounted namespace");
}
fn mount_fs_overlay(bento_config: &BentoConfigJson) {
    let mut lowerdir = String::new();
    for (i, dir) in bento_config.lowerdir.iter().enumerate() {
        assert!(fs::exists(dir).is_ok());
        if i == bento_config.lowerdir.len() - 1 {
            lowerdir.push_str(dir);
        } else {
            lowerdir.push_str(dir);
            lowerdir.push_str(":");
        }
    }

    let fstype = Some("overlay");
    let flags = MsFlags::empty();
    assert!(fs::exists(&bento_config.upperdir).is_ok());
    assert!(fs::exists(&bento_config.workdir).is_ok());
    let overlay_options = format!(
        "lowerdir={},upperdir={},workdir={}",
        lowerdir,
        bento_config.upperdir.display(),
        bento_config.workdir.display()
    );
    let overlay_options = &overlay_options[..];
    let data = Some(overlay_options);

    mount(Some("overlay"), &bento_config.merge, fstype, flags, data)
        .expect("Failed to Mount Filesystem");

    let bind_mount = &bento_config.merge.join("app");
    assert!(fs::exists(&bento_config.merge).is_ok());
    if !bind_mount.exists() {
        fs::create_dir_all(&bind_mount).expect("Failed to create /app");
    }
    // Hardcoded until we add user/cli mount options.
    mount(
        Some(&bento_config.mount),
        bind_mount,
        None::<&Path>,
        MsFlags::MS_BIND,
        None::<&[u8]>,
    )
    .expect("Failed to Mount USER Filesystem");
}

pub fn get_bento_config_path(name: &str) -> PathBuf {
    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");
    let bento_container_path = PathBuf::from(&bento_containers_env).join(name);
    let bento_config_path = bento_container_path.join("bento_config.json");
    bento_config_path
}

fn unshare_pid_and_uts_namespace() {
    //** Create PID namespace **//
    unshare(CloneFlags::CLONE_NEWPID).expect("Failed to create a PID namespace");
    //** UTS namespace **//
    unshare(CloneFlags::CLONE_NEWUTS).expect("Failed to create uts namespace");
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
            unshare_pid_and_uts_namespace(); // Isolate processes

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child }) => {
                    let child_pid = child.as_raw();
                    json::update_container_status(name, Some(child_pid), json::State::Running)?;

                    waitpid(child, None)?;
                    json::update_container_status(name, None, json::State::Stopped)?;
                    unmount_and_clean_up(&bento_config.merge); // Clean exit

                    return Ok(());
                }
                Ok(ForkResult::Child) => {}
                Err(e) => return Err(anyhow!("Failed to fork the repo: {}", e)),
            }

            // Child process continues its work as a daemon
            //** In the child: chroot into the prepared directory **//
            chroot(&bento_config.merge).expect("Failed to chroot");
            std::env::set_current_dir(&bento_config.cwd)
                .expect("Failed to set the container working directory");
            sethostname(name).expect("Failed to set the hostname");
            let (path, args, env) = get_execve_params(bento_config);

            // let path = CString::new("/usr/bin").expect("Not a valid path");
            // let arg1 = CString::new("python").expect("Not a valid argument");
            // let arg2 = CString::new("main.py").expect("Not a valid argument");
            // let args = vec![arg1, arg2];
            // let env_var = CString::new("MY_VAR=hello").expect("Not a env variable");
            // let env = vec![env_var];
            execve(&path, &args, &env).expect("Failed to execute exec function in container");
            process::exit(0);
        }
        Err(e) => return Err(anyhow!("Failed to fork the repo: {}", e)),
    }
}

fn get_execve_params(bento_config: &BentoConfigJson) -> (CString, Vec<CString>, Vec<CString>) {
    let mut args: Vec<CString> = Vec::new();
    let mut env: Vec<CString> = Vec::new();
    for arg in bento_config.cmd.iter() {
        args.push(CString::new(arg.to_owned()).unwrap());
    }
    for e in bento_config.env.iter() {
        env.push(CString::new(e.to_owned()).unwrap());
    }
    let mut path = String::new();

    if let Some(cmd) = bento_config.cmd.get(0) {
        if cmd.contains("/") {
            // hunt in the provided paths
            path.push_str(cmd);
        } else {
            let env_v = get_executable_paths(&bento_config.env);
            for e in env_v.iter() {
                if Path::new(e).join(cmd).is_file() {
                    // let p_ath = Path::new(e).join(cmd);
                    // let str = p_ath.to_string_lossy();
                    match PathBuf::from(e).join(cmd).to_str() {
                        Some(p) => {
                            path.push_str(p);
                        }
                        None => panic!("Failed to convert execve pathbuf to string"),
                    }
                }
            }
        }
    }
    (CString::new(path).unwrap(), args, env)
}

fn unmount_and_clean_up(merge: &PathBuf) {
    //** Unmount the container filesystem **//
    let app = merge.join("app");
    umount(&app).expect("Failed to Unmount");
    umount(merge).expect("Failed to Unmount");
}

fn _clean_up(container_dir: &PathBuf) {
    fs::remove_dir_all(container_dir).expect("Failed to remove dir");
}

pub fn start(name: &str) {
    let bento_config_path = get_bento_config_path(name);
    let bento_config =
        get_bento_config(&bento_config_path).expect("Failed to load the bento_config.json");

    unshare_user_namespace(); // Get privileges
    unshare_mount_namespace(); // Isolate filesystem
    mount_fs_overlay(&bento_config); // Set up container root
    if let Err(e) = fork_into_namespaces(&bento_config, name) {
        eprintln!("Start failed: {}", e) // Clean exit
    }
}

pub fn create(name: &String, image: &String, mount: &PathBuf, cwd: &PathBuf) -> Result<()> {
    let (image, name) = &format_create_params(name, image);
    let (bento_images_env, bento_containers_env) = &get_bento_envs();

    let (new_bento_image_path, new_bento_container_path) =
        &create_container_dirs(bento_images_env, bento_containers_env, name, image);

    if let Err(e) = unpack_image(image, bento_images_env, new_bento_image_path) {
        rollback_dirs(vec![new_bento_image_path, new_bento_container_path]);
        panic!("Error: {}. unpacking image.", e)
    }
    let (container_name, created_container_path) = json::create_bento_config(
        name,
        &new_bento_image_path,
        &new_bento_container_path,
        mount,
        cwd,
    );

    json::add_to_container_manifest(&container_name, &created_container_path)?;
    Ok(())
}

fn apply_signal(pid: Pid, signal: Signal) -> Result<()> {
    kill(pid, signal)?;
    Ok(())
}

pub fn stop(name: &str, container: &Container) -> Result<()> {
    if let Some(c_pid) = container.pid {
        let pid = Pid::from_raw(c_pid);
        match apply_signal(pid, Signal::SIGTERM) {
            Ok(()) => {
                for i in 1..=10 {
                    if let Some(c) = json::check_existing_container(name) {
                        match c.state {
                            State::Stopped => return Ok(()),
                            _ => {
                                if i < 10 {
                                    thread::sleep(time::Duration::from_secs(1));
                                } else {
                                    apply_signal(pid, Signal::SIGKILL)?;
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

pub fn exec(container: &Container) -> Result<()> {
    eprintln!("Yup, she is running alright: {:?}", container);
    if let Some(pid) = container.pid {
        let proc_fd: PathBuf = PathBuf::from("/proc").join(pid.to_string()).join("ns");
        let f = File::open(proc_fd)?;
        let borrowed_fd: BorrowedFd<'_> = f.as_fd();
        setns(borrowed_fd, CloneFlags::CLONE_NEWPID)?;

        return Ok(());
    } else {
        return Err(anyhow!(ErrorKind::NotFound));
    }
}

pub fn get_executable_paths(env: &Vec<String>) -> Vec<&str> {
    let index = get_path_index(env);
    let v: Vec<&str> = env[index].split(":").collect();
    v
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
pub fn get_bento_envs() -> (String, String) {
    let bento_images_env: String =
        env::var("BENTO_IMAGES_PATH").expect("Failed to get images path from .env");

    let bento_containers_env: String =
        env::var("BENTO_CONTAINERS_PATH").expect("Failed to get container path from .env");

    (bento_images_env, bento_containers_env)
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
