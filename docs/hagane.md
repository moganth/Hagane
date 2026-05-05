# Hagane Shipping Guide

This document covers building, packaging, installing, and validating the Hagane CLI that is shipped to users. Both Windows and Linux targets are covered.

## What Is Shipped

### Windows install layout

- `C:\Program Files\Hagane\bin\hagane.exe`
- `C:\Program Files\Hagane\runtime\...` (embedded workspace used at build time)

### Linux install layout

- `/usr/local/hagane/bin/hagane`
- `/usr/local/bin/hagane` → symlink created by post-install hook for immediate PATH access
- `/etc/profile.d/hagane-path.sh` and `/etc/bash.bashrc` entry (if system PATH component selected)
- `/usr/local/hagane/uninstall.sh` — generated uninstall script

The installed `hagane` binary compiles user installers from any directory by using the bundled runtime workspace.

### Runtime Source Of Truth

- The authoritative code lives in root workspace crates: `engine/`, `runner/`, and `ui/`.
- `hagane/payload/runtime` is generated at build time when packaging `hagane/installer.yaml`.
- This avoids maintaining duplicate runtime source trees in git while still shipping a self-contained installed Hagane.

## Build Hagane CLI

### Windows

```powershell
cargo build -p builder --bin hagane --release
```

Output: `target/release/hagane.exe`

### Linux

```bash
cargo build -p builder --bin hagane --release
```

Output: `target/release/hagane`

### Workspace Version Management

The version is defined once in the root `Cargo.toml` under `[workspace.package]` and inherited by all crates (`engine`, `runner`, `builder`). To bump the version, edit only the root `Cargo.toml`:

```toml
[workspace.package]
version = "0.1.5"
edition = "2021"
```

All crates pick it up automatically via `version.workspace = true`.

## Stage Hagane Into Its Own Payload

### Windows

```powershell
Copy-Item .\target\release\hagane.exe .\hagane\payload\bin\hagane.exe -Force
```

### Linux

```bash
cp ./target/release/hagane ./hagane/payload/bin/hagane
chmod +x ./hagane/payload/bin/hagane
```

## Build Hagane Installer

### Windows

With auto-discovery (if `hagane/installer.yaml` is the only YAML in the current directory):

```powershell
hagane run --release
```

Or with an explicit path:

```powershell
hagane run .\hagane\installer.yaml --release
```

Expected output: `hagane\bin\hagane-setup.exe`

### Linux

```bash
# From the repo root — builds the Linux tarball/installer binary
./target/release/hagane run hagane/installer.yaml --release
```

Expected output: `hagane/bin/hagane-linux-x86_64`

> **Auto-discovery**: If exactly one `.yaml` or `.yml` file exists in the current directory, Hagane selects it automatically. If multiple are found, Hagane prints a warning listing all of them and asks you to specify explicitly. If none are found, Hagane tries `hagane/installer.yaml` as a fallback.

## Install And Verify

### Windows

```powershell
Start-Process .\hagane\bin\hagane-setup.exe -Wait
Test-Path "C:\Program Files\Hagane\bin\hagane.exe"
& "C:\Program Files\Hagane\bin\hagane.exe" --version
```

### Linux

The installer requires root to write to `/usr/local/`. When `require_admin: true`, it re-launches itself with `sudo` automatically.

```bash
./hagane/bin/hagane-linux-x86_64
```

Verify after the GUI installer completes:

```bash
# Symlink created by post-install hook:
ls -la /usr/local/bin/hagane

# Binary accessible in PATH (new terminal):
hagane --version

# System PATH entry (if system PATH component was selected):
cat /etc/profile.d/hagane-path.sh
grep hagane /etc/bash.bashrc
```

> Open a **new terminal** after installation. The PATH update takes effect in new terminal sessions. No re-login required \u2014 `/etc/bash.bashrc` is sourced for all non-login interactive shells (including the default WSL2 terminal).

## Test User Flow

### Windows — Installed Hagane

```powershell
# Auto-discovery (single YAML in directory)
Set-Location C:\your-installer.yaml-folder-path
hagane run --release

# Explicit manifest
hagane run installer.yaml --release

# From source build (developer workflow)
.\target\release\hagane.exe run .\path\to\installer.yaml --release
```

### Linux — Installed Hagane

```bash
# Auto-discovery
cd /path/to/your-project
hagane run --release

# Explicit manifest
hagane run installer.yaml --release

# From source build (developer workflow)
./target/release/hagane run ./path/to/installer.yaml --release
```

Expected output: `<manifest-dir>/bin/<app-name>-linux-x86_64`

## Linux Uninstall

The Linux installer generates `/usr/local/hagane/uninstall.sh`. Run it as root:

```bash
sudo /usr/local/hagane/uninstall.sh
```

The script performs these steps in order:

1. `rm -rf /usr/local/hagane` — removes the entire install directory
2. Removes `/usr/local/bin/hagane` symlink if it points into the install directory
3. Removes PATH entries from `~/.bashrc` and `~/.profile` — uses `$SUDO_USER` to target the actual user's home, not root's
4. Removes the `# hagane:` PATH entry from `/etc/bash.bashrc`
5. Removes `/etc/profile.d/hagane-path.sh`

Verify after uninstall:

```bash
test -d /usr/local/hagane && echo "FAIL" || echo "OK: install dir removed"
ls /usr/local/bin/hagane 2>/dev/null && echo "FAIL" || echo "OK: symlink removed"
grep -c hagane /etc/bash.bashrc 2>/dev/null || echo "OK: bash.bashrc clean"
```

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
2. Use `mode: auto` for lifecycle logs, or `mode: manual_only` to suppress normal lifecycle logging.
3. For file logs, ensure `logging.path` and `logging.file_name` are set.
4. Use a writable path during testing, such as `{{TEMP}}`.
5. Check [LOGGING.md](LOGGING.md) for behavior details and [ERROR_CODES.md](ERROR_CODES.md) for code-level troubleshooting.

Variable syntax note:

- Preferred manifest variable syntax is `{{KEY}}` (for example `{{INSTDIR}}/logs`).
- Use `{{KEY}}` syntax consistently in new manifests.

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
- Confirm the destination path is writable by the installer process.

## Release Checklist

### Windows
- Build `hagane.exe` in release mode.
- Stage binary into `hagane/payload/bin/hagane.exe`.
- Build `hagane-setup.exe`.
- Install on clean machine or VM.
- Verify PATH integration (user/system choices).
- Verify ability to build external installer projects.
- Verify EXE icon and UI branding.
- Verify installation logs and error codes are emitted correctly during a failing test manifest.

### Linux
- Build `hagane` in release mode (`cargo build -p builder --bin hagane --release`).
- Stage binary into `hagane/payload/bin/hagane` and `chmod +x`.
- Build `hagane-linux-x86_64`.
- Install on clean machine or WSL2 VM (`sudo`).
- Verify `/usr/local/bin/hagane` symlink is created by the post-install hook.
- Open a new terminal and confirm `hagane --version` works without modifying PATH manually.
- Verify user PATH scope writes to the correct user's `~/.bashrc` (check `$SUDO_USER`).
- Verify system PATH scope writes to both `/etc/profile.d/hagane-path.sh` and `/etc/bash.bashrc`.
- Verify `uninstall.sh` removes all installed files, symlinks, and PATH entries cleanly.
