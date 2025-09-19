use nix::mount::mount;
use nix::sched::{CloneFlags, unshare};
use nix::{
    mount::MsFlags,
    sys::wait::waitpid,
    unistd::{ForkResult, chroot, fork, getpid, write},
};

use nix::unistd::execve;
use std::ffi::CString;
use std::process;

pub fn make_child_pid() {
    println!("=== Starting Container Creation ===");
    println!("About to create PID namespace...");
    println!("My pid BEFORE unshare is {}", getpid());

    println!("Step 1: Creating mount namespace...");
    //** Create mount namespace (isolates your filesystem operations) **//
    unshare(CloneFlags::CLONE_NEWNS).expect("Failed to create a mounted namespace");
    println!("✅ Mount namespace created successfully!");

    println!("Step 2: Creating container root directory...");
    //** Create your container root directory **//
    let container_dir = "/home/cquick/Desktop/dev/temp/container-practice";
    // fs::create_dir(container_dir).expect("Failed to create path");
    // println!("✅ Created directory: {}", container_dir);

    println!("Step 3: Setting up bind mount...");
    //** Mount/copy your container filesystem into that directory **//
    let source_dir = "/home/cquick/Desktop/dev/temp/temp_untar";
    let source = Some(source_dir);
    let target = container_dir;
    let fstype = None::<&str>;
    let flags = MsFlags::MS_BIND;
    let data = None::<&str>;

    println!("Mounting {} -> {}", source_dir, target);
    mount(source, target, fstype, flags, data).expect("Failed to Mount Filesystem");
    println!("✅ Bind mount successful!");

    println!("Step 4: Creating PID namespace...");
    //** Create PID namespace **//
    unshare(CloneFlags::CLONE_NEWPID).expect("Failed to create a PID namespace");
    println!("✅ PID namespace created!");

    println!("Step 5: Forking process...");
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            println!("🔄 PARENT: Continuing execution in parent process");
            println!("🔄 PARENT: New child has pid: {}", child);
            println!("🔄 PARENT: Waiting for child to complete...");
            waitpid(child, None).unwrap();
            println!("🔄 PARENT: Child process completed!");
        }
        Ok(ForkResult::Child) => {
            println!("\n--- CHILD PROCESS STARTED ---");
            write(std::io::stdout(), "I'm a new child process\n".as_bytes()).ok();
            println!("👶 CHILD: My PID is {}", getpid());

            println!("👶 CHILD: About to chroot into container...");
            //** In the child: chroot into the prepared directory **//
            chroot(container_dir).expect("chroot failed");
            std::env::set_current_dir("/").expect("failed to cd to root");
            println!("👶 CHILD: ✅ chroot successful!");

            println!("👶 CHILD: Testing chroot by reading /bin directory...");
            // Test that chroot worked by trying to read /bin (should be the container's /bin now)
            match std::fs::read_dir("/bin") {
                Ok(entries) => {
                    let count = entries.count();
                    println!(
                        "👶 CHILD: 🎉 chroot worked! Found {} entries in /bin",
                        count
                    );
                }
                Err(e) => {
                    println!("👶 CHILD: ❌ chroot might have failed: {}", e);
                }
            }
            let path = CString::new("/bin/bash").unwrap();
            let arg1 = CString::new("").unwrap();
            // let arg2 = CString::new("-l").unwrap();
            // let arg3 = CString::new("/").unwrap();
            let args = vec![arg1];
            let env_var = CString::new("MY_VAR=hello").unwrap();
            let env = vec![env_var];
            execve(&path, &args, &env).unwrap();
            eprintln!("execve failed!");
            println!("👶 CHILD: Container setup complete! Exiting...");
            process::exit(0);
        }
        Err(e) => {
            println!("❌ Fork failed: {}", e);
        }
    }

    println!("=== Container Creation Complete ===");
}
