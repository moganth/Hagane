# Installer Engine

A fast, YAML-driven installer framework for Windows built with Rust, WebView2, and Zstd.

## Documentation

- Core project documentation: [docs/documentation.md](docs/documentation.md)
- Hagane build and shipping guide: [docs/hagane.md](docs/hagane.md)
- Test flow and validation notes: [TESTING.md](docs/TESTING.md)

## Quick Commands

Installed Hagane (after installing Hagane on the machine):

```powershell
hagane run installer.yaml --release
```

Build from source (from this repository):

```powershell
cargo build --release -p builder --bin hagane
.\target\release\hagane.exe run .\path\to\installer.yaml --release
```

## Repository Highlights

- `engine/` : parser, validation, requirements, installer step execution
- `runner/` : setup executable host (Win32 + WebView2)
- `builder/` : `hagane` CLI for packaging and compilation
- `ui/` : HTML/CSS/JS installer pages and bridge
- `sdk/` : schema and example manifests
- `hagane/` : package definition used to ship Hagane itself
