# Installer Engine Documentation

Installer Engine is a YAML-driven Windows installer framework built in Rust.
It combines a native backend (requirements checks, install steps, rollback) with a WebView2 UI layer and a builder CLI (`hagane`) for packaging and compilation.

## What It Solves

- Native requirement checks instead of PowerShell/CMD scripts.
- Declarative installer authoring via `installer.yaml`.
- Brandable HTML/CSS UI via WebView2.
- Compressed archive embedding with Zstd.
- A single setup executable output for distribution.

## Core Workflow

1. Author an `installer.yaml` manifest.
2. Place assets and payload folders next to the manifest.
3. Run `hagane run <manifest> --release`.
4. Distribute the generated `<app>-setup.exe`.

## Manifest Overview

Typical manifest areas:

- `app`: name, version, publisher, icons, install directory, elevation.
- `theme`: optional visual tokens (colors, typography, dimensions).
- `pages`: UI flow order (`welcome`, `license`, `requirements`, `install`, `finish`, etc.).
- `requirements`: OS, RAM, disk, registry, and custom checks.
- `steps`: install actions such as extract, file ops, shortcuts, registry, services, env vars.
- `components`: optional install choices (for example PATH scope).

## Build Commands

Installed Hagane (after installing Hagane on the machine):

```powershell
hagane run installer.yaml --release
```

Build from source (from this repository):

```powershell
cargo build --release -p builder --bin hagane
.\target\release\hagane.exe run .\path\to\installer.yaml --release
```

For iterative work during development:

```powershell
cargo run -p builder --bin hagane -- run .\path\to\installer.yaml --release
```

## Project Structure

- `engine/`: parser, validator, requirements, step execution, rollback, state.
- `runner/`: Windows host process, resource embedding, WebView2 integration.
- `builder/`: CLI that packages manifest/assets and compiles runner.
- `ui/`: HTML pages, CSS theme tokens, JS bridge.
- `sdk/`: schema + sample manifest/assets.
- `hagane/`: packaging project used to ship Hagane itself.

## Requirements Engine (Native APIs)

- Windows version: `RtlGetVersion()`
- RAM: `GlobalMemoryStatusEx()`
- Disk: `GetDiskFreeSpaceEx()`
- Registry-based checks for .NET/VC++ detection

## Authoring Tips

- Keep archive folder names aligned with `extract.archive` names.
  Example: `payload.zst` expects a `payload/` folder next to the manifest.
- Use schema hints in manifests for better IDE completion:

```yaml
# yaml-language-server: $schema=../../sdk/schema/installer.schema.json
```

- You ship only the generated setup executable to end users.
  The YAML manifest is embedded at build time.

## Testing Notes

- Use `TESTING.md` for manual/automated test flow.
- Validate both GUI and silent install (`/S`) behavior.
- Validate install and uninstall side effects (files, PATH, registry, shortcuts).
