# Hagane Shipping Guide

This document covers building, packaging, installing, and validating the Hagane CLI that is shipped to users.

## What Is Shipped

Install target layout:

- `C:\Program Files\Hagane\bin\hagane.exe`
- `C:\Program Files\Hagane\runtime\...` (embedded workspace used at build time)

The installed `hagane.exe` compiles user installers from any directory by using the bundled runtime workspace.

## Build Hagane CLI

From workspace root:

```powershell
Set-Location C:\Users\monip\code\Installer-Engine
cargo build -p builder --bin hagane --release
```

## Stage Hagane Into Its Own Payload

Before packaging `hagane-setup.exe`, copy the fresh binary into the Hagane payload:

```powershell
Copy-Item .\target\release\hagane.exe .\hagane\payload\bin\hagane.exe -Force
```

## Build Hagane Installer

```powershell
.\target\release\hagane.exe run .\hagane\installer.yaml --release
```

Expected output:

- `hagane\bin\hagane-setup.exe`

## Install And Verify

Run installer:

```powershell
Start-Process .\hagane\bin\hagane-setup.exe -Wait
```

Verify installation:

```powershell
Test-Path "C:\Program Files\Hagane\bin\hagane.exe"
& "C:\Program Files\Hagane\bin\hagane.exe" --version
```

## Test User Flow

Installed Hagane (recommended for end-user flow validation):

```powershell
Set-Location C:\your-installer.yaml-folder-path
hagane run installer.yaml --release
```

From source build (developer workflow):

```powershell
Set-Location C:\Users\monip\code\Installer-Engine
.\target\release\hagane.exe run C:\your-installer.yaml-folder-path\installer.yaml --release
```

Expected output:

- `C:\Users\monip\code\test-installer\myapp-setup.exe`

## Icon Behavior And Current Fixes

Two icon paths exist:

- UI icon/logo in pages (loaded from manifest assets).
- Windows EXE icon resource (stamped at compile time).

Current implementation passes manifest icon path from builder to runner using `HAGANE_ICON_PATH` and normalizes Windows verbatim paths so winres can embed the icon correctly.

## Troubleshooting

### `bin` folder is empty after install

Cause: `hagane\payload\bin\hagane.exe` was not staged before building `hagane-setup.exe`.

Fix:

1. Rebuild Hagane CLI.
2. Copy it into `hagane\payload\bin`.
3. Rebuild and reinstall `hagane-setup.exe`.

### `Could not find workspace root`

Cause: old Hagane binary or missing runtime structure.

Fix:

- Rebuild and reinstall latest Hagane.
- Ensure installed structure includes `bin\hagane.exe` and `runtime\Cargo.toml`.

### No custom EXE icon in Explorer

Cause: stale build or shell icon cache.

Fix:

1. Rebuild using latest Hagane.
2. Confirm logs show `Using EXE icon:` during pack.
3. Re-open Explorer (or sign out/in) if cache still shows old icon.

## Release Checklist

- Build `hagane.exe` in release mode.
- Stage binary into `hagane/payload/bin/hagane.exe`.
- Build `hagane-setup.exe`.
- Install on clean machine or VM.
- Verify PATH integration (user/system choices).
- Verify ability to build external installer projects.
- Verify EXE icon and UI branding.
