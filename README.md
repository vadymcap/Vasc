<div align="center">
  <img alt="vasc" src="gitassets/vasc-namespace.png">
  <br/>
  <br/>
  <strong>Full featured tool for Roblox development</strong>
  <br/>
  <br/>
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue" alt="License">
  <img src="https://img.shields.io/badge/built%20on-Argon-orange" alt="Fork of Argon">
</div>

---

# VASC

VASC is a powerful CLI tool that elevates the Roblox development experience. It is a fork of [Argon](https://github.com/argon-rbx/argon), extended with additional features and improvements.

This repository is the core of the VASC project — all processing happens here. It works alongside two companion packages:

- [**vasc-vscode**](https://github.com/vadymcap/vasc-vscode) — a VS Code extension that wraps this CLI with a user-friendly interface
- [**vasc-roblox**](https://github.com/vadymcap/vasc-roblox) — a Roblox Studio plugin required for live syncing

## Features

- **Two-way sync** — keep code and instance properties in sync between your editor and Roblox Studio in real time
- **Project building** — compile projects into Roblox binary (`.rbxl`) or XML (`.rbxlx`) format
- **Beginner and professional friendly** — sensible defaults out of the box, deep customization when you need it
- **Fast and lightweight** — minimal overhead, built for speed
- **Helper commands** — a rich set of utility commands to streamline common tasks
- **Workflow automation** — built-in CI/CD support for automated pipelines

## Components

| Package | Description |
|---|---|
| **vasc** *(this repo)* | Core CLI — handles all processing and syncing logic |
| **vasc-vscode** | VS Code extension — GUI wrapper around the CLI |
| **vasc-roblox** | Studio plugin — required for live sync to function |

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

<div align="center">
  <sub>VASC is a fork of <a href="https://github.com/argon-rbx/argon">Argon</a>, originally created by Dervex. Licensed under Apache 2.0.</sub>
</div>