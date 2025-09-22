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
use std::process;

pub fn create_namespace() {
    //** Create mount namespace (isolates your filesystem operations) **//
    unshare(CloneFlags::CLONE_NEWNS).expect("Failed to create a mounted namespace");

    //** Create your container root directory **//
    let upper_dir = "/home/cquick/Desktop/dev/temp/container-practice/upper";
    // fs::create_dir(upper_dir).expect("Failed to create path");
    let workdir = "/home/cquick/Desktop/dev/temp/container-practice/workdir";
    let merge = "/home/cquick/Desktop/dev/temp/container-practice/merged";
    // fs::create_dir(workdir).expect("Failed to create path");

    //** Mount/copy your container filesystem into that directory **//
    let lower_dir = "/home/cquick/Desktop/dev/temp/temp_untar";

    // Values for the filesystemtype argument supported by the kernel are
    // listed in /proc/filesystems
    let fstype = Some("overlay");
    // mount flags
    let flags = MsFlags::empty();
    //
    let overlay_options = format!(
        "lowerdir={},upperdir={},workdir={}",
        { lower_dir },
        { upper_dir },
        { workdir },
    );
    let overlay_options = &overlay_options[..];
    let data = Some(overlay_options);

    mount(Some("overlay"), merge, fstype, flags, data).expect("Failed to Mount Filesystem");

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
            chroot(merge).expect("chroot failed");
            std::env::set_current_dir("/").expect("failed to cd to root");

            let mut generator = Generator::with_naming(Name::Numbered);
            sethostname(generator.next().unwrap()).expect("Failed to set hostname");
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
    umount(merge).expect("Failed to Unmount");
}
