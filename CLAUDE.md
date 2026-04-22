# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

quicssh-rs is a QUIC proxy for SSH connections that enables SSH over QUIC protocol. It's a Rust implementation of quicssh that provides connection stability and migration capabilities for SSH sessions over unreliable networks.

## Commands

### Build and Development
```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run the project
cargo run -- server  # or client

# Check code
cargo check

# Format code
cargo fmt

# Run lints
cargo clippy

# Run tests
cargo test
```

### Usage
```bash
# Start server (listens on 0.0.0.0:4433 by default, proxies to 127.0.0.1:22)
cargo run -- server
cargo run -- server --listen 0.0.0.0:4433 --proxy-to 127.0.0.1:22

# Start server with custom MTU upper bound
cargo run -- server --mtu-upper-bound 1200
cargo run -- server --mtu-upper-bound safety  # Use RFC-compliant 1200 bytes

# Run client
cargo run -- client quic://hostname:4433

# Run client with custom MTU upper bound
cargo run -- client --mtu-upper-bound 1200 quic://hostname:4433
cargo run -- client --mtu-upper-bound safety quic://hostname:4433
```

## Architecture

The application consists of three main modules:

### Core Components
- **main.rs**: Entry point with CLI parsing using clap, handles subcommands and logging configuration
- **server.rs**: QUIC server that accepts connections and proxies to SSH server
- **client.rs**: QUIC client that connects to server and tunnels SSH traffic

### Network Flow
1. SSH client connects to quicssh-rs client via ProxyCommand
2. quicssh-rs client establishes QUIC connection to quicssh-rs server
3. quicssh-rs server proxies traffic to actual SSH server over TCP
4. QUIC provides connection migration and better weak network handling

### Key Libraries
- **quinn**: QUIC protocol implementation
- **tokio**: Async runtime
- **rustls**: TLS/QUIC crypto (with self-signed certs for QUIC)
- **clap**: CLI argument parsing
- **log4rs**: Logging framework

### Security Notes
- Server uses self-signed certificates generated via rcgen
- **Client skips certificate verification by default** (for ease of use with self-signed certs)
  - SSH layer provides end-to-end encryption and host key verification
  - QUIC acts as transport tunnel (similar to TCP)
  - Risk: DNS/IP spoofing could enable traffic interception (but not plaintext exposure due to SSH)
- Both modules handle MTUD (Maximum Transmission Unit Discovery) where supported

> Note: Certificate verification flags (`--verify-cert` on client, `--cert`/`--key` on server) are **not implemented yet**. Until they ship, deployment should assume QUIC cert verification is disabled.

<!-- TODO: Implement certificate verification option
Currently, the client always skips QUIC certificate verification (dangerous_configuration).
While SSH provides its own security layer, QUIC cert verification would prevent:
- DNS/IP spoofing attacks
- Traffic interception for future cryptanalysis
- Man-in-the-middle positioning

IMPORTANT: This requires BOTH server and client changes:

Server-side changes (src/server.rs):
1. Add --cert <path> option to specify TLS certificate file (PEM format)
2. Add --key <path> option to specify private key file (PEM format)
3. Modify configure_server() to:
   - Load cert/key from files when options are provided
   - Fall back to self-signed certificate (current behavior) when not specified
4. Support proper hostnames in self-signed cert (not just "localhost")

Client-side changes (src/client.rs):
1. Add --verify-cert flag to enable certificate verification (default: false)
2. Add --ca-cert <path> option to specify custom CA certificate
3. Modify make_client_endpoint() to:
   - Use rustls::RootCertStore with system certs when --verify-cert is set
   - Support custom CA cert for self-signed server certificates
   - Continue using SkipServerVerification when flag is absent (backward compatibility)
4. Update README to recommend --verify-cert for production use

Example usage:
  # Server with Let's Encrypt certificate
  quicssh-rs server --cert /etc/letsencrypt/live/example.com/fullchain.pem \
                           --key /etc/letsencrypt/live/example.com/privkey.pem

  # Client with system CA verification
  quicssh-rs client --verify-cert quic://example.com:4433

  # Self-signed certificate workflow
  quicssh-rs server  # generates self-signed cert, prints fingerprint
  quicssh-rs client --verify-cert --ca-cert /path/to/server.crt quic://hostname:4433

References:
  - src/server.rs:35-61 (configure_server, self-signed cert generation)
  - src/client.rs:147-161 (SkipServerVerification implementation)
-->

## Configuration

- Default server listen address: `0.0.0.0:4433`
- Default SSH proxy target: `127.0.0.1:22`
- Logging can be configured via `--log` and `--log-level` flags
- Test suite includes unit tests and integration tests (smoke test)

## Coding Guidelines

### Comments and Documentation

- **Always write comments in English**, not Japanese
- **Avoid emojis in code and metadata files** (Cargo.toml, comments, etc.)
  - Use plain text alternatives (e.g., "WARNING:" instead of "⚠️")
  - Emojis are acceptable in README.md for visual emphasis
- Reference relevant RFCs and standards when applicable
- Example: When setting MTU values, cite RFC 9000, RFC 8899, or RFC 8200

### Commit Messages

- **Always write commit messages in English**
- Follow conventional commits format: `type(scope): description`
- Common types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`
- Keep the first line under 72 characters
- Reference issue numbers when applicable

### MTU Configuration

The MTU (Maximum Transmission Unit) upper bound can be configured via the `--mtu-upper-bound` option:

**Default behavior (no option specified):**
- Uses Quinn's default MTU discovery with upper bound of **1452 bytes**
- Suitable for most standard network environments

**Conservative mode (`--mtu-upper-bound safety` or `--mtu-upper-bound 1200`):**
- Sets MTU upper bound to **1200 bytes** for maximum compatibility
- **RFC 9000 Section 14.1**: QUIC Initial packets must be at least 1200 bytes
- **RFC 8899 Section 5.1.2**: Recommends 1200 bytes as BASE_PLPMTU for UDP
- **RFC 8200**: IPv6 minimum link MTU is 1280 bytes
  - 1200-byte payload + 40-byte IPv6 header + 8-byte UDP header = 1248 bytes (fits within 1280)
- Ensures compatibility across all network environments, including VPN tunnels and IPv6-only networks

**Custom MTU:**
- Specify any numeric value (e.g., `--mtu-upper-bound 1300`)
- Useful for specific network requirements or testing

**Usage examples:**
```bash
# Server with safety MTU
quicssh-rs server --mtu-upper-bound safety

# Client with custom MTU
quicssh-rs client --mtu-upper-bound 1300 quic://hostname:4433

# Server with default Quinn MTU (no option)
quicssh-rs server
```

## CI/CD Pipeline

Two minimal GitHub Actions workflows.

### [ci.yml](.github/workflows/ci.yml) — on every push and PR
Single job on `ubuntu-latest`:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --locked -- -D warnings`
- `cargo test --locked`

### [release.yml](.github/workflows/release.yml) — on tags `v*`
Build matrix with two targets:
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

Each target produces a `quicssh-rs-Linux-<arch>-musl.tar.gz` + `.sha256` sidecar, uploaded to the GitHub Release via `softprops/action-gh-release`.

### Release process
1. Bump `version` in `Cargo.toml`, commit.
2. Tag `vX.Y.Z` and push the tag. The release workflow builds and publishes tarballs automatically.

### Binary distribution

- **GitHub Releases**: tarball per arch (`quicssh-rs-Linux-x86_64-musl.tar.gz`, `quicssh-rs-Linux-aarch64-musl.tar.gz`).
- **Install script** ([scripts/install.sh](scripts/install.sh)): `curl -fsSL https://raw.githubusercontent.com/aruz/quicssh-rs/master/scripts/install.sh | sudo bash` — downloads the latest release, installs `/usr/local/bin/quicssh-rs`, writes a hardened systemd unit, and enables it.
- **Update script** ([scripts/update.sh](scripts/update.sh)): in-place upgrade to latest (or `update.sh vX.Y.Z`).
- **Manual build**: `cargo build --release`.
