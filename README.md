# Bento 🍱

A container runtime built from scratch in Rust to learn systems programming fundamentals.

## What is this?

Bento is an educational container runtime that implements core container isolation mechanisms found in production runtimes like Docker, containerd, and youki. This project exists to deeply understand how containers actually work under the hood.

## What it does

- Parses OCI-compliant container images
- Creates isolated container environments using Linux namespaces
- Implements overlay filesystem for copy-on-write functionality
- Manages container lifecycle (create, start, stop)
- Provides process isolation and filesystem isolation

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
- `status`: Check container state

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

*Bento: containers compartmentalized like a bento box*
