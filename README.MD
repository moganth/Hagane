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

Publish the installer as a GitHub release asset:

```powershell
gh release create v0.1.0 .\hagane\bin\hagane-setup.exe --title "v0.1.0" --notes "Hagane installer release"
```

## Banner and Logo Asset Sizes

With the current shell layout:

- Logo: 96 x 96 px
- Banner: 684 x 96 px (for default `window_width: 780`)
- Icon (`icon.ico`): include 16, 24, 32, 48, 64, 128, and 256 px layers

High-DPI recommended exports:

- Logo: 192 x 192 px (2x)
- Banner: 1368 x 192 px (2x)

For `icon.ico`, keep 256 x 256 as the largest layer (PNG-compressed inside ICO is fine).

If you change `window_width` in theme settings, use this formula:

- Banner width = `window_width - 96`
- Banner height = `96`

Keep banner height fixed at 96 px and recalculate only the width when `window_width` changes.

## Repository Highlights

- `engine/` : parser, validation, requirements, installer step execution
- `runner/` : setup executable host (Win32 + WebView2)
- `builder/` : `hagane` CLI for packaging and compilation
- `ui/` : HTML/CSS/JS installer pages and bridge
- `sdk/` : schema and example manifests
- `hagane/` : package definition used to ship Hagane itself
