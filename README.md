# Bento 🍱

A container runtime built from scratch in Rust to learn systems programming fundamentals.

## What is this?

Bento is an educational container runtime that implements core container isolation mechanisms found in production runtimes like Docker, containerd, and youki. This project exists to deeply understand how containers actually work under the hood.

## What it does

- Parses OCI-compliant container images
- Creates isolated container environments using Linux namespaces
- Implements overlay filesystem for copy-on-write functionality
- Manages container lifecycle (create, start, stop, kill, status, exec)
- Provides process isolation and filesystem isolation

## Tutorial

Requires [Docker](https://www.docker.com/get-started/) to pull OCI compliant images

Make the directories to house all your bento containers and use docker to pull and tar images.

The example below demonstrates creating a busybox container.

```bash
mkdir -p ~/.bento/containers
mkdir -p ~/.bento/images

docker pull busybox
docker save -o ~/.bento/images/busybox.tar busybox
```

Clone or fork the repo to create and start your first container.

```bash
git clone https://github.com/CarloQuick/bento.git
cd bento
```

By default, bento containers are rootless, so you can create containers without given the process sudo permissions.

```bash
cargo run -- create busybox-container busybox
```

**Note:**
This will create the container, but the current terminal will no longer be useful. There is no `pty` hooked up via the `fork()` therefore you will need to open a new terminal and `exec` into the container. Here you can do as you please inside the container.

```bash
cargo run -- start busybox-container
```

in a new terminal in the same directory

```bash
cargo run -- exec busybox-container ls -la
```

to end the process, you can gracefull end or kill it by name.

```bash
cargo run -- stop busybox-container
```

or

```bash
cargo run -- kill busybox-container
```

## Technical implementation

**Isolation mechanisms:**

- User namespaces (rootless container execution)
- PID namespaces (isolated process trees)
- Mount namespaces (isolated filesystem)
- UTS namespaces (isolated hostname)

**Filesystem handling:**

- OCI image format parsing (index.json, manifest.json, config.json)
- Layer extraction and overlay filesystem mounting
- Container-specific upperdir/workdir/merge directories

**Current functionality:**

- `create`: Parse OCI image, extract layers, configure container filesystem
- `start`: Fork process into isolated namespaces, mount overlay filesystem, execute container command
- `stop`: Gracefully end a container
- `kill`: Forcefully terminate a container
- `status`: Check container state
- `exec`: Run command in already running container

## Why Bento?

Container runtimes sit at the intersection of operating systems, filesystems, and process management. Building one requires understanding:

- Linux syscalls and kernel interfaces
- Filesystem layering and mount mechanics
- Process forking and namespace isolation
- OCI image specifications
- Systems-level error handling in Rust

This is a learning project focused on depth over features.

## Current status

**Experimental** - This runtime is not production-ready and should not be used in production environments. It exists purely for educational purposes and to demonstrate understanding of container internals.

## Built with

- Rust
- Linux namespaces (via `nix` crate)
- OCI image format specifications

## Learning resources

This project was built by working through:

- OCI Runtime Specification
- Linux namespaces and cgroups documentation
- Production runtime codebases (youki, runc)
- The Rust Programming Language book

---

_Bento: containers compartmentalized like a bento box_
