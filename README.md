<div align="center">
  <img alt="vasc" src="gitassets/vasc-namespace.png">
  <br/>
  <br/>
  <strong>Full featured tool for Roblox development</strong>
  <br/>
  <br/>
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue" alt="License">
  <img src="https://img.shields.io/badge/built%20on-Vasc-orange" alt="Fork of Vasc">
</div>

---

# VASC

VASC is a powerful CLI tool that elevates the Roblox development experience. It is a fork of [Argon](https://github.com/argon-rbx/argon), extended with additional features and improvements.

This repository is the core of the VASC project — all processing happens here. It works alongside two companion packages:

- [**Vasc-vscode**](https://github.com/vadymcap/Vasc-vscode) — a VS Code extension that wraps this CLI with a user-friendly interface
- [**Vasc-roblox**](https://github.com/vadymcap/Vasc-roblox) — a Roblox Studio plugin required for live syncing

## Features

- **Two-way sync** — keep code and instance properties in sync between your editor and Roblox Studio in real time
- **Project building** — compile projects into Roblox binary (`.rbxl`) or XML (`.rbxlx`) format
- **Beginner and professional friendly** — sensible defaults out of the box, deep customization when you need it
- **Fast and lightweight** — minimal overhead, built for speed
- **Helper commands** — a rich set of utility commands to streamline common tasks
- **Workflow automation** — built-in CI/CD support for automated pipelines
- **LAN collaboration** — real-time filesystem collaboration over a local network or VPN (e.g. Radmin VPN)

## Components

| Package | Description |
|---|---|
| **vasc** *(this repo)* | Core CLI — handles all processing and syncing logic |
| **Vasc-vscode** | VS Code extension — GUI wrapper around the CLI |
| **Vasc-roblox** | Studio plugin — required for live sync to function |

## Run locally (Windows)

1. Install Rust: https://rustup.rs
2. Install Visual C++ build tools (required for `link.exe`):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --accept-package-agreements --accept-source-agreements --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

3. Build the CLI:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

Output binary: `target\release\vasc.exe`

## Create release binaries

Cross-platform binaries are built automatically by GitHub Actions in [`.github/workflows/release.yml`](.github/workflows/release.yml).

Publish a new tag to trigger the build pipeline:

```bash
git tag 2.0.30
git push origin 2.0.30
```

Artifacts uploaded to GitHub Release include:
- `windows-x86_64` (`vasc.exe` in zip)
- `linux-x86_64`
- `macos-x86_64`
- `macos-aarch64`

For local Windows packaging (manual testing):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1 -Version 2.0.30
```

This creates `vasc-2.0.30-windows-x86_64.zip` in repository root.

---

## LAN Collaboration (v1)

VASC includes built-in support for real-time filesystem collaboration over a local network or VPN (e.g. [Radmin VPN](https://www.radmin-vpn.com/)).

> **Scope**: v1 synchronises the project **filesystem** only.  Roblox Studio sync is out of scope for this release.

### How it works

One machine acts as the **host** — it is the authoritative source of truth.  All other machines are **clients** that join the session.

- Host file changes are broadcast to all clients.
- Client file changes are _proposed_ to the host; the host validates them (checking the revision number for conflicts) and, if accepted, applies the change and broadcasts it to every other client.
- Each file has a per-file revision number and a content hash.  If a client's `base_rev` does not match the host's current revision the proposal is rejected with a conflict response.

### Starting a host session

```bash
vasc collab host --project /path/to/my-project --bind 0.0.0.0 --port 8080
```

With token authentication (recommended on a shared VPN):

```bash
vasc collab host --project /path/to/my-project --bind 0.0.0.0 --port 8080 --token mysecret
```

| Flag | Description | Default |
|---|---|---|
| `--project` / `-p` | Project directory to share | *(required)* |
| `--bind` / `-b` | IP address to bind | `0.0.0.0` |
| `--port` / `-P` | Port to listen on | `8080` |
| `--token` / `-t` | Optional shared secret | *(none — open)* |

The host continuously watches the project directory for changes and broadcasts them to connected clients.

### Joining a session

```bash
vasc collab join 192.168.1.10:8080 --token mysecret --dir ./my-project
```

> **Warning** — on join the target directory (`--dir`) is **overwritten** by the host's project.  Any existing content that is not present on the host will be lost.

By default a backup of the existing directory is created at:

```
.vasc-collab-backup/<timestamp>/
```

relative to the parent of `--dir`.  You can opt out with `--no-backup`:

```bash
vasc collab join 192.168.1.10:8080 --token mysecret --dir ./my-project --no-backup
```

| Flag | Description | Default |
|---|---|---|
| `<HOST:PORT>` | Host address | *(required)* |
| `--token` / `-t` | Shared secret (if the host uses one) | *(none)* |
| `--dir` / `-d` | Local directory for the project | *(required)* |
| `--backup` | Back up existing `--dir` before overwrite | *(default)* |
| `--no-backup` | Skip the backup | *(off)* |

After the initial snapshot the client enters a continuous event loop:
1. Polls the host for new changes every 500 ms and applies them locally.
2. Watches the local filesystem and proposes any local changes to the host.

### Typical LAN/VPN workflow

1. Start a VPN session so all machines share a virtual IP range (e.g. Radmin VPN creates a `26.x.x.x` range).
2. On the host machine run `vasc collab host …`.
3. On each client run `vasc collab join <host-ip>:8080 …`.
4. Edit files in your favourite editor on any machine — changes propagate automatically.

---

<div align="center">
  <sub>VASC is a fork of <a href="https://github.com/argon-rbx/argon">Argon</a>, originally created by Dervex. Licensed under Apache 2.0.</sub>
</div>