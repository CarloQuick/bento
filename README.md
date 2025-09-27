# Bento Container Runtime

A minimal container runtime built from scratch in Rust, implementing Linux namespaces and overlay filesystems for process isolation.

## Current Features

- **Process Isolation**: Creates isolated containers using PID and UTS namespaces
- **Custom Hostnames**: Support for named containers with `--name` flag
- **Overlay Filesystem**: Implements container layering using overlay mounts
- **Clean Lifecycle**: Proper container startup and shutdown

## Usage

```bash
# Build the project
cargo build

# Create and run a container
sudo ./target/debug/bento create

# Create a named container
sudo ./target/debug/bento create --name=my-container
```

## Requirements

- Linux system with namespace support
- Root privileges (sudo) for namespace operations
- Rust toolchain for building

## Architecture

Bento implements containerization through:

- **Linux Namespaces**: PID and UTS isolation for process separation
- **Overlay Filesystems**: Layered filesystem implementation for container images
- **Process Management**: Direct process execution and lifecycle handling

## Development Status

This is an early-stage learning project implementing core container runtime concepts. Currently supports basic container creation and execution with namespace isolation.

### Current Limitations

- **Manual Base Image**: Requires a pre-extracted Ubuntu image on the filesystem as the overlay lower layer. Future implementation will handle image downloading and extraction automatically.

- **No Proc Mount**: Proc not yet mounted. For example `ps aux` currently is not supported in the container.

## Building

```bash
git clone https://github.com/CarloQuick/bento.git
cd bento
cargo build
```
