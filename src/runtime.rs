use nix::mount::{mount, umount};
use nix::unistd::{execve, getgid, getuid, sethostname};
use nix::{
    mount::MsFlags,
    sys::wait::waitpid,
    unistd::{ForkResult, chroot, fork},
};
use nix::{
    sched::{CloneFlags, unshare},
    unistd::getpid,
};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process;
use std::{env, fs};

fn _print_uid_map_from_pid() {
    let pid = getpid();
    let pid = pid.as_raw();
    let mut path = PathBuf::new(); // Create an empty PathBuf
    path.push("/proc");
    path.push(pid.to_string());
    // path.push("self");
    path.push("uid_map");
    println!("proc_dir: {:?}", path);
    let contents = std::fs::read(path).expect("Failed to read mapping");
    // Convert the bytes to a string for printing (assuming valid UTF-8)
    let text = String::from_utf8_lossy(&contents);
    println!("File contents: {}", text);
}
fn _print_gid_map_from_pid() {
    let pid = getpid();
    let pid = pid.as_raw();
    let mut path = PathBuf::new(); // Create an empty PathBuf
    path.push("/proc");
    path.push(pid.to_string());
    // path.push("self");
    path.push("gid_map");
    println!("proc_dir: {:?}", path);
    let contents = std::fs::read(path).expect("Failed to read mapping");
    // Convert the bytes to a string for printing (assuming valid UTF-8)
    let text = String::from_utf8_lossy(&contents);
    println!("File contents: {}", text);
}

fn _print_mappings() {
    let host_uid = getuid();
    let host_gid = getgid();
    println!("uid {} gid {}", host_uid, host_gid);
}
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
fn mount_fs_overlay(name: &str) -> (PathBuf, PathBuf) {
    let path = env::var("BENTO_PATH").expect("Path var to be set.");

    let container_dir = Path::new(&path).join(name);
    fs::create_dir_all(&container_dir).expect("Failed to create container_dir");

    //** Create your container root directory **//
    let upperdir = container_dir.join("upper");
    let workdir = container_dir.join("workdir");
    let merge = container_dir.join("merge");
    fs::create_dir(&upperdir).expect("Failed to create upperdir");
    fs::create_dir(&workdir).expect("Failed to creat workdir");
    fs::create_dir(&merge).expect("Failed to creat merge");

    //** Mount/copy your container filesystem into that directory **//
    // iterate over vec and make it
    let lowerdir = Path::new(&path).join("temp_untar");

    // Values for the filesystemtype argument supported by the kernel are
    // listed in /proc/filesystems
    let fstype = Some("overlay");
    // mount flags
    let flags = MsFlags::empty();
    //
    let overlay_options = format!(
        "lowerdir={},upperdir={},workdir={}",
        lowerdir.display(),
        upperdir.display(),
        workdir.display()
    );
    let overlay_options = &overlay_options[..];
    let data = Some(overlay_options);

    mount(Some("overlay"), &merge, fstype, flags, data).expect("Failed to Mount Filesystem");
    (merge, container_dir)
}
fn unshare_pid_and_uts_namespace() {
    //** Create PID namespace **//
    unshare(CloneFlags::CLONE_NEWPID).expect("Failed to create a PID namespace");
    //** UTS namespace **//
    unshare(CloneFlags::CLONE_NEWUTS).expect("Failed to create uts namespace");
}
fn fork_into_namespaces(merge: &PathBuf, name: &str) {
    //** Fork into the namespace **//
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            waitpid(child, None).expect("Unable to wait for pid change");
        }
        Ok(ForkResult::Child) => {
            //** In the child: chroot into the prepared directory **//
            chroot(merge).expect("chroot failed");
            std::env::set_current_dir("/").expect("failed to cd to root");
            sethostname(name).expect("Failed to set hostname");
            let path = CString::new("/bin/bash").unwrap();
            let arg1 = CString::new("bash").unwrap();
            let args = vec![arg1];
            let env_var = CString::new("MY_VAR=hello").unwrap();
            let env = vec![env_var];

            execve(&path, &args, &env).expect("Failed to replace process image.");
            process::exit(0);
        }
        Err(e) => {
            println!("❌ Fork failed: {}", e);
        }
    }
}

fn unmount_and_clean_up(merge: &PathBuf, container_dir: &PathBuf) {
    //** Unmount the container filesystem **//
    umount(merge).expect("Failed to Unmount");
    fs::remove_dir_all(container_dir).expect("Failed to remove dir");
}
pub fn create_container(name: &str) {
    unshare_user_namespace(); // Get privileges
    unshare_mount_namespace(); // Isolate filesystem
    let (merge, container_dir) = mount_fs_overlay(&name); // Set up container root
    unshare_pid_and_uts_namespace(); // Isolate processes
    fork_into_namespaces(&merge, name); // Run container
    unmount_and_clean_up(&merge, &container_dir); // Clean exit
}
