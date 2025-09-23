use nix::mount::{mount, umount};
use nix::sched::{CloneFlags, unshare};

use names::{Generator, Name};
use nix::unistd::{execve, sethostname};
use nix::{
    mount::MsFlags,
    sys::wait::waitpid,
    unistd::{ForkResult, chroot, fork},
};
use std::ffi::CString;
use std::path::Path;
use std::process;
use std::{env, fs};
pub fn create_namespace() {
    let mut generator: Generator<'_> = Generator::with_naming(Name::Numbered);
    let container_name = generator.next().unwrap();
    //** Create mount namespace (isolates your filesystem operations) **//
    unshare(CloneFlags::CLONE_NEWNS).expect("Failed to create a mounted namespace");
    let path = env::var("BENTO_PATH").expect("Path var to be set.");

    // home/cquick/Desktop/dev/temp/<something>
    let container_dir = Path::new(&path).join(&container_name);
    fs::create_dir_all(&container_dir).expect("Failed to create container_dir");

    //** Create your container root directory **//
    let upperdir = container_dir.join("upper");
    let workdir = container_dir.join("workdir");
    let merge = container_dir.join("merge");
    fs::create_dir(&upperdir).expect("Failed to create upperdir");
    fs::create_dir(&workdir).expect("Failed to creat workdir");
    fs::create_dir(&merge).expect("Failed to creat merge");

    //** Mount/copy your container filesystem into that directory **//
    let lowerdir = Path::new(&path).join("temp_untar");
    // let lowerdir: &'static str = "/home/cquick/Desktop/dev/temp/temp_untar";

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
    println!("{}", overlay_options);
    let overlay_options = &overlay_options[..];
    let data = Some(overlay_options);

    mount(Some("overlay"), &merge, fstype, flags, data).expect("Failed to Mount Filesystem");

    //** Create PID namespace **//
    unshare(CloneFlags::CLONE_NEWPID).expect("Failed to create a PID namespace");
    unshare(CloneFlags::CLONE_NEWUTS).expect("Failed to create uts namespace");
    //** UTS namespace **//

    //** Fork into the namespace **//
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            waitpid(child, None).expect("Unable to wait for pid change");
        }
        Ok(ForkResult::Child) => {
            //** In the child: chroot into the prepared directory **//
            chroot(&merge).expect("chroot failed");
            std::env::set_current_dir("/").expect("failed to cd to root");

            // let mut generator = Generator::with_naming(Name::Numbered);
            sethostname(&container_name).expect("Failed to set hostname");
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
    //** Unmount the container filesystem **//
    umount(&merge).expect("Failed to Unmount");
    fs::remove_dir_all(container_dir).expect("Failed to remove dir");
}
