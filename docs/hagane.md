# Hagane Shipping Guide

This document covers building, packaging, installing, and validating the Hagane CLI that is shipped to users.

## What Is Shipped

Install target layout:

- `C:\Program Files\Hagane\bin\hagane.exe`
- `C:\Program Files\Hagane\runtime\...` (embedded workspace used at build time)

The installed `hagane.exe` compiles user installers from any directory by using the bundled runtime workspace.

### Runtime Source Of Truth

- The authoritative code lives in root workspace crates: `engine/`, `runner/`, and `ui/`.
- `hagane/payload/runtime` is generated at build time when packaging `hagane/installer.yaml`.
- This avoids maintaining duplicate runtime source trees in git while still shipping a self-contained installed Hagane.

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

### Installer error codes are not visible

Cause: file logging is not configured, the destination is not writable, or the error happened before the installer reached the step runner.

Fix:

1. Add a top-level `logging` block to `installer.yaml`.
2. Use `mode: auto` for lifecycle logs, or `mode: manual_only` for explicit inline-only execution logs.
3. When using inline `log.file` or `log.both`, ensure `logging.path` and `logging.file_name` are set.
4. Use a writable path during testing, such as `{{TEMP}}`.
5. Check [LOGGING.md](LOGGING.md) for behavior details and [ERROR_CODES.md](ERROR_CODES.md) for code-level troubleshooting.

Variable syntax note:

- Preferred manifest variable syntax is `{{KEY}}` (for example `{{INSTDIR}}/logs`).
- Legacy `$KEY` syntax remains supported for backward compatibility.

### PowerShell step fails with access denied

Cause: the script needs elevation, or execution policy blocks the command.

Fix:

- Set `app.require_admin: true` when the script writes to protected locations.
- Confirm the PowerShell command is valid and available in PATH.
- Use `timeout_sec` only if the script is expected to finish quickly.

### Logging file not created

Cause: `logging.path` or `logging.file_name` is missing, or the destination folder cannot be created.

Fix:

- Add `logging.path` and `logging.file_name` to the manifest.
- Use a writable location during development.
- Confirm inline `log.file` messages are non-empty.

## Release Checklist

- Build `hagane.exe` in release mode.
- Stage binary into `hagane/payload/bin/hagane.exe`.
- Build `hagane-setup.exe`.
- Install on clean machine or VM.
- Verify PATH integration (user/system choices).
- Verify ability to build external installer projects.
- Verify EXE icon and UI branding.
- Verify installation logs and error codes are emitted correctly during a failing test manifest.
