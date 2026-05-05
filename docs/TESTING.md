# Testing the Installer Engine

## Complete Workflow: Build & Test

### Prerequisites

✅ All crates compile successfully:
```powershell
cargo build --release
```

---

## 1️⃣ Build the `hagane` CLI Tool

```powershell
cargo build --release -p builder
```

Output: `target/release/hagane.exe`

---

## 2️⃣ Prepare Payload (Example Installer)

The builder compresses directories referenced in your `install.components` block. For the example, you need:

### Create minimal test payload:

```powershell
cd c:\Users\monip\code\Installer-Engine\sdk\example

# Create payload directories
mkdir -Force payload docs samples

# Add dummy files (required for archives to exist)
echo "MyApp version 2.1.0" > payload\version.txt
echo "Sample documentation" > docs\README.txt
echo "Sample project files" > samples\example.txt
```

### Directory structure should look like:

```
sdk/example/
├── installer.yaml
├── assets/
│   ├── logo.png
│   ├── banner.png
│   └── icon.ico
├── payload/                  ← compressed to payload.zst
│   └── version.txt
├── docs/                     ← compressed to docs.zst
│   └── README.txt
└── samples/                  ← compressed to samples.zst
    └── example.txt
```

---

## 3️⃣ Build the Installer Executable

```powershell
cd sdk\example

# Auto-discovery: if installer.yaml is the only YAML in the directory
..\..\target\release\hagane.exe run --release

# Or with an explicit manifest path
..\..\target\release\hagane.exe run installer.yaml --release
```

> **Auto-discovery**: Hagane automatically finds a manifest when exactly one `.yaml`/`.yml` file is present in the current directory. If multiple YAML files are found, Hagane prints a warning listing them all and exits — specify the manifest explicitly in that case.

### What it does:

1. ✅ Loads and validates `installer.yaml`
2. ✅ Loads assets (logo, banner, icon)
3. ✅ Compresses payload directories (payload/, docs/, samples/)
4. ✅ Generates runtime embedded artifacts
5. ✅ Runs `cargo build --release` to compile the final `.exe`

### Output:

```
target/release/MyApp-setup.exe
```

---

## 4️⃣ Test the Installer

### **GUI Mode (Default)**

```powershell
# Run the installer
..\..\target\release\MyApp-setup.exe
```

This opens the WebView2-based GUI with pages:
- Welcome screen
- License agreement
- System requirements check
- Installation directory picker
- Component selection
- Installation summary
- Progress bar
- Finish screen

### **Silent Mode (No UI)**

```powershell
# Install without UI
..\..\target\release\MyApp-setup.exe /S
```

Uses default settings from the manifest's `silent:` section.

---

## 5️⃣ Verify Installation

### Default install location:
```
C:\Program Files\Acme\MyApp\
```

### What should be installed:
- `version.txt` (from payload)
- `docs/` (from docs archive)
- `samples/` (from samples archive)
- `uninstall.exe` (auto-generated)

### Check registry:
```powershell
# Verify Add/Remove Programs entry
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AcmeMyApp"

# Verify app configuration
reg query "HKLM\SOFTWARE\Acme\MyApp"
```

---

## Troubleshooting

### Error: "Missing archive 'payload'"

**Cause:** No `payload/` directory exists  
**Fix:**
```powershell
mkdir payload
echo "test" > payload\test.txt
```

### Error: "Missing asset 'assets/banner.png'"

**Cause:** Referenced in manifest but file doesn't exist  
**Fix:** Either:
- Create the file: `copy assets\logo.png assets\banner.png`
- Remove from manifest: `banner: null` or delete the line

### No UI / WebView2 error

**Cause:** WebView2 Runtime not installed  
**Fix:**
- Install [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/download/)
- Or use `--build-runtime-check` flag (if implemented)

### Installation path issues

Edit `app.default_install_dir` in `installer.yaml`:

```yaml
app:
  default_install_dir: "{{PROGRAMFILES64}}/MyCompany/MyApp"
```

### Logging and error code validation

Use the following checks to verify the implemented logging and error code behavior:

1. Add a `logging` block with `path` and `file_name` to your test manifest.
2. Set `logging.mode` to `auto` and run once to verify lifecycle logs are emitted automatically.
3. Switch to `logging.mode: manual_only` and verify normal lifecycle messages are suppressed.
4. Confirm the installer writes a log file in the configured location.
5. Trigger a known failure, such as a missing archive, to confirm the installer emits an `HG-*` code.
6. Confirm `install.hooks.post_install` failures classify correctly for syntax errors, non-zero exit, timeout, and access denied cases.

Example test output should include lines like:

```text
[ERROR] HG-EXTRACT-001 step=4 action=extract field=archive value=payload.zst reason="..." fix="..."
```

---

## Quick Start Template

Minimal installer with no archives:

### 1. Create `installer.yaml`:

```yaml
app:
  name: "HelloWorld"
  version: "1.0.0"
  publisher: "MyCompany"
  default_install_dir: "{{PROGRAMFILES64}}/MyCompany/HelloWorld"
  require_admin: false

pages:
  - type: welcome
  - type: summary
  - type: install
  - type: finish

install:
  setup:
    create_dirs:
      - "{{INSTDIR}}"

  components:
    core:
      archive: "payload.zst"
      target: "{{INSTDIR}}"

  system:
    register_app:
      hive: HKCU
      key: "Software/MyCompany/HelloWorld"
      version: "1.0.0"
      install_location: "{{INSTDIR}}"

  finalize:
    write_uninstaller: "{{INSTDIR}}/uninstall.exe"
```

### 2. Build:

```powershell
hagane.exe run installer.yaml --release
```

### 3. Test:

```powershell
target/release/HelloWorld-setup.exe
```

### Logging-focused Quick Start

If you want to test the logging pipeline directly, add logging and a post-install hook:

```yaml
logging:
  mode: auto
  path: "{{TEMP}}/MyAppLogs"
  file_name: "installation.log"
  timestamp: true
  slow_step_warn_sec: 5

install:
  hooks:
    post_install:
      - run:
          platform: windows
          command: "Write-Host 'Testing PowerShell action'"
          shell: powershell
          wait: true
          fail_on_nonzero: true
```

---

## Build Notes

Use release mode for shipping builds:

```powershell
hagane.exe run installer.yaml --release
```

For rapid local iteration, run without `--release` from source during development:

```powershell
cargo run -p builder --bin hagane -- run installer.yaml
```

---

## Testing Requirements Check

All system requirements are checked **in parallel** (no PowerShell):

1. **OS Version** → WinAPI `RtlGetVersion()`
2. **RAM** → WinAPI `GlobalMemoryStatusEx()`
3. **Disk Space** → WinAPI `GetDiskFreeSpaceEx()`
4. **Windows Update KB** → Registry query
5. **.NET Framework** → Registry `HKLM\SOFTWARE\Microsoft\NET Framework Setup`
6. **VC++ Redistributable** → Registry scan

Verify these work on your system by:
1. Opening the installer
2. Going to Requirements page
3. Checking results display instantly (parallel evaluation)

---

## Advanced Testing

### Capture build logs:

```powershell
hagane.exe run installer.yaml --release 2>&1 | Tee-Object build.log
```

### Check embedded.rs:

```powershell
# View generated manifest
Get-Content runner/src/generated/embedded.rs | Select-Object -First 50
```

### Monitor installation:

```powershell
# Watch the installer write files
Get-Process explorer | ForEach-Object { watcher }
# Or use Process Monitor: https://docs.microsoft.com/en-us/sysinternals/downloads/procmon
```

---

## Next Steps

1. ✅ Run example: `MyApp-setup.exe`
2. ✅ Customize `installer.yaml` with your app
3. ✅ Add your files to `payload/`, `docs/`, etc.
4. ✅ Rebuild and test
5. ✅ Ship the `.exe`

---

## Linux Testing (WSL2 / Ubuntu)

### Prerequisites

Install WebKitGTK build dependencies in WSL2:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev
```

### Build Linux Installer Binary

From the workspace root on Linux/WSL2:

```bash
cargo build --release -p runner
```

Output: `target/release/<app-name>-linux-x86_64`

### Run the Linux Installer

The GUI uses wry + WebKitGTK. WSLg provides `$DISPLAY` automatically.

```bash
./target/release/myapp-linux-x86_64
```

If `app.require_admin: true`, the installer re-launches itself with `sudo` automatically.

### Verify PATH Integration

After installation completes, open a **new terminal** and check:

```bash
# If user scope was selected:
grep "INSTDIR" ~/.bashrc

# If system scope was selected:
cat /etc/profile.d/hagane-path.sh
grep "INSTDIR" /etc/bash.bashrc

# Verify the binary is in PATH:
which myapp
myapp --version
```

> **WSL2 note:** `/etc/profile.d/` only applies to login shells. System scope also writes to `/etc/bash.bashrc` so the PATH works in all WSL2 terminal sessions without requiring a login shell.

### Verify Symlink (if created by post-install hook)

```bash
ls -la /usr/local/bin/myapp
# Should show: /usr/local/bin/myapp -> /usr/local/myapp/bin/myapp
```

### Test Linux Uninstall

```bash
sudo /usr/local/myapp/uninstall.sh
```

The uninstall script:
1. Removes the install directory (`rm -rf`)
2. Removes the `/usr/local/bin/<name>` symlink if it points into the install directory
3. Cleans `~/.bashrc` and `~/.profile` entries (using `$SUDO_USER` to target the correct user's home)
4. Cleans `/etc/bash.bashrc` entries
5. Removes `/etc/profile.d/hagane-path.sh`

Verify after uninstall:

```bash
test -d /usr/local/myapp && echo "FAIL: dir still exists" || echo "OK: removed"
which myapp 2>/dev/null && echo "FAIL: still in PATH" || echo "OK: not in PATH"
```

### Test Post-Install Bash Hook Logging

Add a bash hook to your test manifest:

```yaml
install:
  hooks:
    post_install:
      - run:
          platform: linux
          command: |
            echo "Hook stdout line"
            echo "Hook stderr line" >&2
          shell: bash
          wait: true
          fail_on_nonzero: true
```

After running the installer, confirm the log shows:
- `[INFO] [bash] Hook stdout line`
- `[WARN] [bash stderr] Hook stderr line`
