# quicssh-rs

> **This is a fork of [oowl/quicssh-rs](https://github.com/oowl/quicssh-rs)** focused on **stabilization and robustness** for production use.

## Fork Purpose

This fork aims to stabilize quicssh-rs for reliable SSH-over-QUIC connections in diverse network environments.

### Key Modifications

- **Configurable MTU Upper Bound**: Control MTU discovery via `--mtu-upper-bound` option
  - **Default**: Uses Quinn's default (1452 bytes) for standard networks
  - **`--mtu-upper-bound safety`**: Conservative 1200 bytes per RFC 9000 §14.1 and RFC 8899 §5.1.2
    - Ensures compatibility with VPN tunnels (Tailscale, WireGuard) and IPv6 minimum MTU (RFC 8200)
    - Prevents UDP packet rejection on constrained network interfaces
  - **Custom values**: Specify any numeric MTU (e.g., `--mtu-upper-bound 1300`)
- **Fixed Linux MTU Discovery**: Corrected `cfg` condition to properly enable MTU discovery on Linux

---

## Installation

### Quick server install (systemd Linux x86_64 / aarch64)

One-liner — downloads the latest release, installs `/usr/local/bin/quicssh-rs`, writes a hardened systemd unit, and starts it on port 4433:

```bash
curl -fsSL https://raw.githubusercontent.com/aruz/quicssh-rs/master/scripts/install.sh | sudo bash
```

Flags are forwarded after `--`:

```bash
curl -fsSL https://raw.githubusercontent.com/aruz/quicssh-rs/master/scripts/install.sh \
    | sudo bash -s -- --port 4343 --proxy-to 127.0.0.1:22
```

Supported flags: `--version vX.Y.Z`, `--port`, `--listen`, `--proxy-to`.

In-place upgrade on an already-deployed host:

```bash
curl -fsSL https://raw.githubusercontent.com/aruz/quicssh-rs/master/scripts/update.sh | sudo bash
```

### Prebuilt binaries

Grab the archive for your platform from [Releases](https://github.com/aruz/quicssh-rs/releases/latest):

- `quicssh-rs-Linux-x86_64-musl.tar.gz`
- `quicssh-rs-Linux-aarch64-musl.tar.gz`

Each archive ships with a matching `.sha256` sidecar.

### Build from source

```bash
git clone https://github.com/aruz/quicssh-rs.git
cd quicssh-rs
cargo build --release
```

---

## About quicssh-rs

> :smile: **quicssh-rs** is a QUIC proxy that allows to use QUIC to connect to an SSH server without needing to patch the client or the server.

`quicssh-rs` is [quicssh](https://github.com/moul/quicssh) rust implementation. It is based on [quinn](https://github.com/quinn-rs/quinn) and [tokio](https://github.com/tokio-rs/tokio)

Why use QUIC? Because SSH is vulnerable in TCP connection environments, and most SSH packets are actually small, so it is only necessary to maintain the SSH connection to use it in any network environment. QUIC is a good choice because it has good weak network optimization and an important feature called connection migration. This means that I can switch Wi-Fi networks freely when remote, ensuring a stable SSH connection.

## Demo

https://user-images.githubusercontent.com/39181969/235409750-234de94a-1189-4288-93c2-45f62a9dfc48.mp4

## Why not mosh?

Because the architecture of mosh requires the opening of many ports to support control and data connections, which is not very user-friendly in many environments. In addition, vscode remote development does not support mosh.

## ⚠️ Security Notice

**IMPORTANT**: By default, this tool **disables QUIC certificate verification** for ease of use with self-signed certificates. This is acceptable for most SSH use cases because:

- SSH itself provides end-to-end encryption and host key verification
- The QUIC layer acts as a transport tunnel, similar to TCP
- The primary risk is **DNS/IP spoofing** leading to potential traffic analysis (not plaintext exposure)

**However, you should be aware that:**

1. **Without QUIC certificate verification**, an attacker who can spoof DNS or hijack IP routing could:
   - Intercept encrypted traffic for future decryption attempts
   - Perform traffic analysis (timing, packet sizes)
   - Set up a man-in-the-middle position (though SSH host key verification would still protect the session)

2. **To eliminate this risk**, you can:
   - Plan: use a future `--verify-cert` flag with proper TLS certificates (**not implemented yet; not available in current releases**)
   - Deploy in trusted network environments only
   - Rely on SSH's host key verification as the primary security layer

**Recommendation**: For sensitive environments, consider implementing certificate verification or using SSH's built-in security features (host key pinning, certificate authentication) as your primary defense.

## Architecture

Standard SSH connection

```
┌───────────────────────────────────────┐             ┌───────────────────────┐
│                  bob                  │             │         wopr          │
│ ┌───────────────────────────────────┐ │             │ ┌───────────────────┐ │
│ │           ssh user@wopr           │─┼────tcp──────┼▶│       sshd        │ │
│ └───────────────────────────────────┘ │             │ └───────────────────┘ │
└───────────────────────────────────────┘             └───────────────────────┘
```

---

SSH Connection proxified with QUIC

```
┌─────────────────────────────────────┐             ┌───────────────────────┐
│                 bob                 │             │         wopr          │
│ ┌─────────────────────────────────┐ │             │ ┌───────────────────┐ │
│ │ssh -o ProxyCommand=             │ │             │ │       sshd        │ │
│ │ "quicssh-rs client       │ │             │ └───────────────────┘ │
│ │  quic://%h:4433" user@wopr      │ │             │          ▲            │
│ └─────────────────────────────────┘ │             │          │            │
│                  │                  │             │          │            │
│               process               │             │  tcp to localhost:22  │
│                  │                  │             │          │            │
│                  ▼                  │             │          │            │
│ ┌─────────────────────────────────┐ │             │ ┌───────────────────┐ │
│ │quicssh-rs client         │─┼─quic (udp)─▶│ │quicssh-rs  │ │
│ │                    wopr:4433    │ │             │ │      server       │ │
│ └─────────────────────────────────┘ │             │ └───────────────────┘ │
└─────────────────────────────────────┘             └───────────────────────┘
```

## Usage

```console
$ quicssh-rs -h
A simple ssh server based on quic protocol

Usage: quicssh-rs <COMMAND>

Commands:
  server  Server
  client  Client
  help    Print this message or the help of the given subcommand(s)

Options:
      --log <LOG_FILE>         Location of log, Default if
      --log-level <LOG_LEVEL>  Log level, Default Error
  -h, --help                   Print help
  -V, --version                Print version
```

### Client

```console
$ quicssh-rs client -h
Client

Usage: quicssh-rs client [OPTIONS] <URL>

Arguments:
  <URL>  Server address

Options:
  -b, --bind <BIND_ADDR>                Client address
      --mtu-upper-bound <MTU_UPPER_BOUND>
                                        MTU upper bound: numeric value (e.g., 1200) or "safety" for RFC-compliant 1200 bytes
  -h, --help                            Print help
  -V, --version                         Print version
```

#### Client SSH Config

```console
╰─$ cat ~/.ssh/config
Host test
    HostName test.test
    User root
    Port 22333
    ProxyCommand /path/to/quicssh-rs client quic://%h:%p

╰─$ ssh test
Last login: Mon May  1 13:32:15 2023 from 127.0.0.1
```

### Server

```console
$ quicssh-rs server -h
Server

Usage: quicssh-rs server [OPTIONS]

Options:
  -l, --listen <LISTEN>                 Address to listen on [default: 0.0.0.0:4433]
  -p, --proxy-to <PROXY_TO>             Address of the ssh server
  -F, --conf <CONF_PATH>
      --mtu-upper-bound <MTU_UPPER_BOUND>
                                        MTU upper bound: numeric value (e.g., 1200) or "safety" for RFC-compliant 1200 bytes
  -h, --help                            Print help
  -V, --version                         Print version
```

[![Powered by DartNode](https://dartnode.com/branding/DN-Open-Source-sm.png)](https://dartnode.com "Powered by DartNode - Free VPS for Open Source")
