# Installer Engine Documentation

Installer Engine is a YAML-driven cross-platform installer framework built in Rust.
It combines a native backend (requirements checks, install plan execution, rollback) with a WebView2/WebKitGTK UI layer and the `hagane` CLI for packaging and compilation.
Windows uses WebView2; Linux uses wry + WebKitGTK.

## Why

| Problem with NSIS | This engine |
|---|---|
| PowerShell/CMD for system checks -> slow, laggy | Native WinAPI calls only — microseconds |
| Fixed UI with limited branding | WebView2 HTML/CSS — fully brandable |
| Script language with no type safety | YAML with JSON Schema validation + IDE autocomplete |
| No parallel processing | Requirement checks run in parallel via Rayon |
| Bloated runtime | Pure Rust binary, no .NET/runtime dependency |

---

## Quick Start

### 1. Write your manifest

```yaml
# sdk/example/installer.yaml
app:
  name: "MyApp"
  version: "1.0.0"
  publisher: "Your Company"
  logo: "assets/logo.png"
  default_install_dir: "{{PROGRAMFILES64}}/YourCompany/MyApp"

theme:
  accent_color: "#0078D4"

pages:
  - type: welcome
  - type: license
  - type: requirements
  - type: install_dir
  - type: install
  - type: finish

requirements:
  - type: os
    platform: windows
    min_build: 18362
    label: "Windows 10 1903+"
  - type: ram
    min_mb: 2048
    label: "2 GB RAM"
  - type: disk
    min_mb: 200
    label: "200 MB free space"

install:
  setup:
    create_dirs:
      - "{{INSTDIR}}"

  components:
    core:
      archive: "payload.zst"
      target: "{{INSTDIR}}"

  system:
    windows:
      shortcuts:
        - name: "MyApp"
          target: "{{INSTDIR}}/MyApp.exe"
          location: desktop

  finalize:
    windows:
      write_uninstaller: "{{INSTDIR}}/uninstall.exe"
```

### 2. Place your payload

```
sdk/example/
├── installer.yaml
├── assets/
│   ├── logo.png
│   ├── banner.png
│   └── icon.ico
└── payload/          <- folder named after archive (without .zst)
    ├── MyApp.exe
    └── ...
```

### 3. Build

Installed Hagane (after installing Hagane on the machine):

```powershell
# Auto-discovery: run from the directory containing your installer.yaml
hagane run --release

# Or with an explicit manifest path
hagane run installer.yaml --release
```

> **Tip — Auto-discovery**: When exactly one `.yaml` or `.yml` file exists in your working directory, `hagane run --release` (no manifest argument) selects it automatically. If multiple YAML files are present, Hagane lists them all and asks you to specify one explicitly.

Build from source (from this repository):

```powershell
cargo build --release -p builder --bin hagane
.\target\release\hagane.exe run .\path\to\installer.yaml --release
```

For iterative work during development:

```powershell
cargo run -p builder --bin hagane -- run .\path\to\installer.yaml --release
```

### 4. Run

```powershell
# GUI mode
myapp-setup.exe

# Silent install (no UI)
myapp-setup.exe /S
```

---

## Theme Customization

The installer supports full theme customization via optional manifest fields.

### Basic Theme Token

```yaml
theme:
  accent_color: "#0078D4"           # Buttons, links, accents
  background_color: "#FFFFFF"       # Main background
  text_color: "#1A1A1A"             # Primary text
  font_family: "'Segoe UI', sans-serif"
```

### Advanced Theme Tokens

```yaml
theme:
  # Color variants for depth and interactivity
  accent_dark_color: "#005A9E"      # Button hover/pressed states
  accent_light_color: "#EBF3FB"     # Focus rings, light backgrounds

  # Surfaces and text
  surface_color: "#F5F5F5"          # Cards, alt backgrounds
  text_muted_color: "#6B6B6B"       # Secondary text, labels
  border_color: "#E0E0E0"           # Borders, dividers
  border_radius: 6                   # Corner roundness (px)

  # Semantic colors
  success_color: "#107C10"          # Success text
  success_bg_color: "#F7F9F8"       # Success background
  error_color: "#C42B1C"            # Error text
  error_bg_color: "#FFF7F6"         # Error background

  # Progress bar — gradient from start to end
  progress_color: "#0078D4"         # Gradient start color
  progress_light_color: "#EBF3FB"   # Gradient end color

  # Window geometry
  window_width: 780                  # Pixels
  window_height: 540                 # Pixels
```

### All Theme Fields Are Optional

Every field in the `theme` block is completely optional. Omit any field and the installer will use a sensible built-in default.

| Field | Default | Purpose |
|---|---|---|
| `accent_color` | `#0078D4` | Primary button color, links, active states |
| `accent_dark_color` | `#005A9E` | Button hover/pressed states, emphasis |
| `accent_light_color` | `#EBF3FB` | Focus rings, light backgrounds, hover underlay |
| `background_color` | `#FFFFFF` | Main window background |
| `surface_color` | `#F5F5F5` | Cards, alternate backgrounds, section dividers |
| `text_color` | `#1A1A1A` | Primary text, headings, body copy |
| `text_muted_color` | `#6B6B6B` | Secondary text, labels, hints, disabled text |
| `border_color` | `#E0E0E0` | Borders, dividers, input outlines |
| `border_radius` | `6` | Corner roundness in pixels (applies to buttons, cards, inputs) |
| `success_color` | `#107C10` | Success message text, checkmarks |
| `success_bg_color` | `#F7F9F8` | Success message background |
| `error_color` | `#C42B1C` | Error message text, warnings |
| `error_bg_color` | `#FFF7F6` | Error message background |
| `progress_color` | `#0078D4` | Progress bar gradient start color |
| `progress_light_color` | `#EBF3FB` | Progress bar gradient end color |
| `font_family` | `'Segoe UI', system-ui, sans-serif` | Typography, applies to all text |
| `window_width` | `780` | Setup window width in pixels |
| `window_height` | `540` | Setup window height in pixels |

**Minimal theme** (just brand color):

```yaml
theme:
  accent_color: "#FF6B35"
```

**Moderate theme** (brand + light/dark mode):

```yaml
theme:
  accent_color: "#2563EB"
  background_color: "#FFFFFF"
  text_color: "#1A1A1A"
```

**Complete theme** (full control):

```yaml
theme:
  accent_color: "#4F8FF7"
  accent_dark_color: "#2E6FDB"
  accent_light_color: "#D9E7FF"
  background_color: "#0F172A"
  surface_color: "#111C33"
  text_color: "#E5EEF9"
  text_muted_color: "#94A3B8"
  border_color: "#24344D"
  success_color: "#22C55E"
  success_bg_color: "#102A1A"
  error_color: "#F87171"
  error_bg_color: "#2A1414"
  progress_color: "#4F8FF7"
  progress_light_color: "#A5C8FF"
  font_family: "'Inter', sans-serif"
  border_radius: 8
  window_width: 800
  window_height: 600
```

### Example Presets

**Minimal Modern** (clean, light, blue accent):

```yaml
theme:
  accent_color: "#2563EB"
  accent_dark_color: "#1D4ED8"
  accent_light_color: "#DBEAFE"
  background_color: "#FFFFFF"
  surface_color: "#F8FAFC"
  text_color: "#0F172A"
  text_muted_color: "#475569"
  border_color: "#E2E8F0"
  success_color: "#15803D"
  success_bg_color: "#F0FDF4"
  error_color: "#B91C1C"
  error_bg_color: "#FEF2F2"
  progress_color: "#2563EB"
  progress_light_color: "#DBEAFE"
```

**Dark Corporate** (dark background, light text, blue accent):

```yaml
theme:
  accent_color: "#4F8FF7"
  accent_dark_color: "#2E6FDB"
  accent_light_color: "#D9E7FF"
  background_color: "#0F172A"
  surface_color: "#111C33"
  text_color: "#E5EEF9"
  text_muted_color: "#94A3B8"
  border_color: "#24344D"
  success_color: "#22C55E"
  success_bg_color: "#102A1A"
  error_color: "#F87171"
  error_bg_color: "#2A1414"
  progress_color: "#4F8FF7"
  progress_light_color: "#A5C8FF"
```

---

## For Open-Source Users

If you are using this engine to ship your own app installer:

- You **must** create your own `installer.yaml` (app metadata, pages, requirements, and `install` plan).
- You do **not** ship `installer.yaml` to end users — it is embedded into the generated setup EXE at build time.
- `installer.schema.json` is optional at runtime, but strongly recommended during authoring for IDE validation/autocomplete.

### Minimal author workflow

1. Copy `sdk/example/installer.yaml` and edit it for your app.
2. Create payload folders next to your manifest (for each `extract` archive name).
3. Build your setup EXE:

```powershell
# Auto-discovery (from the directory containing installer.yaml)
hagane run --release

# Or explicit:
hagane run ./path/to/installer.yaml --release
```

4. Distribute only the output setup EXE (for example `myapp-setup.exe`).

### Enable YAML Schema in VS Code

At the top of your manifest, add:

```yaml
# yaml-language-server: $schema=../../sdk/schema/installer.schema.json
```

This gives field completion, type checks, and early validation errors while authoring.

---

## Project Structure

```
installer-engine/
├── engine/                    # Core library crate
│   └── src/
│       ├── parser/            # YAML schema + validation (serde)
│       ├── requirements/      # Native WinAPI system checks (parallel)
│       ├── install/           # Step runner, file ops, registry, shortcuts
│       ├── state.rs           # Installer state machine
│       └── ipc.rs             # Rust ↔ WebView2 JSON message protocol
├── runner/                    # Binary — Win32 window + WebView2 host
├── builder/                   # hagane CLI — compresses & packages installer
├── ui/
│   ├── pages/                 # HTML pages (welcome, license, requirements…)
│   └── assets/                # style.css, bridge.js
└── sdk/
    ├── example/               # Example installer.yaml + assets
    └── schema/                # installer.schema.json for IDE support
```

---

## Requirements Checks

> **Platform note:** The checks below use Windows-native APIs. Linux requirements checks use equivalent POSIX calls. Both platforms evaluate checks in parallel via Rayon.

| Check | Windows API used |
|---|---|
| Windows version | `RtlGetVersion()` |
| RAM | `GlobalMemoryStatusEx()` |
| Disk space | `GetDiskFreeSpaceEx()` |
| .NET Framework | Registry read — no subprocess |
| VC++ Redistributable | Registry scan — no subprocess |

All checks run **in parallel** via Rayon the moment the requirements page loads.

---

## Available Pages

| type | Description |
|---|---|
| `welcome` | Splash with logo, app name, description |
| `license` | Scrollable license text with accept checkbox |
| `requirements` | Live parallel check results |
| `install_dir` | Path picker with disk space indicator |
| `components` | Optional feature selection with sizes |
| `user_info` | Name, organization, serial key fields |
| `summary` | Review before install |
| `install` | Progress bar, real-time log, rollback on error |
| `finish` | Launch app / desktop shortcut toggles |
| `error` | Error detail with rollback confirmation |

---

## Install DSL Blocks

The installer now uses a required top-level `install` block (legacy top-level `steps` is rejected).

| block | Purpose |
|---|---|
| `install.setup.create_dirs` | Creates required directories before extraction |
| `install.components.<id>` | Maps each component to an archive and destination target |
| `install.system.register_app` | Writes app registration metadata |
| `install.system.register_uninstall` | Writes Add/Remove Programs metadata |
| `install.system.shortcuts` | Creates desktop/start-menu/startup shortcuts |
| `install.system.path` | **(Windows flat form)** Adds a directory to PATH. Single entry only. Scope: `user` (HKCU) or `system` (HKLM). |
| `install.system.linux.path` | **(Linux)** Adds a directory to PATH. Accepts a **single entry or a list** — use a list to define user and system scope entries simultaneously, each gated on its own component. |
| `install.hooks.post_install` | Runs post-install hooks. `shell` can be `powershell` (Windows), `bash` (Linux), or `program` (all platforms). |
| `install.finalize.write_uninstaller` | Writes the generated uninstaller executable |

---

## Declared Variables (Define Once, Reuse Anywhere)

Use a top-level `variables` block to avoid repeating the same paths and keys.

```yaml
variables:
  COMPANY: "Acme"
  PRODUCT: "MyApp"
  COMPANY_PRODUCT: "{{COMPANY}}/{{PRODUCT}}"
  APP_REG_KEY: "SOFTWARE/{{COMPANY_PRODUCT}}"

  platform:
    windows:
      INSTALL_ROOT: "{{PROGRAMFILES64}}/{{COMPANY}}/{{PRODUCT}}"
    linux:
      INSTALL_ROOT: "/opt/{{COMPANY}}/{{PRODUCT}}"

app:
  default_install_dir: "{{INSTALL_ROOT}}"

install:
  setup:
    create_dirs:
      - "{{INSTDIR}}"

  components:
    core:
      archive: "payload.zst"
      target: "{{INSTDIR}}"

  system:
    windows:
      register_app:
        hive: HKLM
        key: "{{APP_REG_KEY}}"
        install_location: "{{INSTDIR}}"
        version: "2.1.0"

  finalize:
    windows:
      write_uninstaller: "{{INSTDIR}}/uninstall.exe"
    linux:
      write_uninstaller: "{{INSTDIR}}/uninstall"
```

Rules:

- Variable keys should use `A-Z`, `0-9`, and `_` (optionally prefixed with `$`).
  - Preferred syntax is `{{KEY}}` (for example `{{INSTDIR}}`).
  - Built-in variables cannot be overridden: `{{INSTDIR}}`, `{{PROGRAMFILES}}`, `{{PROGRAMFILES64}}`, `{{APPDATA}}`, `{{LOCALAPPDATA}}`, `{{TEMP}}`, `{{WINDIR}}`.
- Declared variables can reference other declared variables.

---

## Variables in Paths

| Variable | Resolves to |
|---|---|
| `{{INSTDIR}}` | Chosen installation directory |
| `{{PROGRAMFILES}}` | `C:\Program Files (x86)` |
| `{{PROGRAMFILES64}}` | `C:\Program Files` |
| `{{APPDATA}}` | `C:\Users\<user>\AppData\Roaming` |
| `{{LOCALAPPDATA}}` | `C:\Users\<user>\AppData\Local` |
| `{{TEMP}}` | Temp directory |
| `{{WINDIR}}` | `C:\Windows` |

Use `{{KEY}}` syntax in new manifests for consistent schema validation.

---

## Logging and Error Codes

The installer supports two logging modes:

- `auto`: lifecycle logging is generated automatically for each executed step (start, slow-step warn, success in file logs, and classified failures).
- `manual_only`: normal lifecycle logging is suppressed during step execution.

In both modes, classified failure lines and rollback errors are still emitted when a step fails.

### Logging Configuration

Add a top-level `logging` block to control mode and file output:

```yaml
logging:
  mode: auto
  path: "{{INSTDIR}}/logs"
  file_name: "installation.log"
  timestamp: true
  include_raw_os_error: false
  slow_step_warn_sec: 10
```

- Set both `path` and `file_name` when you want file logging enabled.
- `slow_step_warn_sec` controls when long-running steps produce a warning.
- When file logging is enabled, completion messages stay in the file log but are not echoed into the UI log box.

### Install Logging Notes

In the current `install` DSL, logging is controlled primarily by top-level `logging.mode`:

- `auto` emits lifecycle logs for compiled install operations.
- `manual_only` suppresses normal lifecycle logs and keeps failure logging deterministic.

For a full behavior matrix and end-to-end examples, see [LOGGING.md](LOGGING.md).

### Post-Install Hook Actions

Use `install.hooks.post_install` to run commands after all install steps complete.

**Windows — PowerShell:**

```yaml
install:
  hooks:
    post_install:
      - run:
          command: |
            Write-Host "Hello from installer"
          shell: powershell
          wait: true
          fail_on_nonzero: true
          timeout_sec: 30
```

**Linux — Bash:**

```yaml
install:
  hooks:
    post_install:
      - run:
          platform: linux
          command: |
            chmod +x "{{INSTDIR}}/bin/myapp"
            ln -sf "{{INSTDIR}}/bin/myapp" /usr/local/bin/myapp
          shell: bash
          wait: true
          fail_on_nonzero: false
          timeout_sec: 10
```

> **`platform`** restricts the hook to a single OS (`windows` or `linux`). Omit it to run on all platforms.

Bash hook stdout is logged at `INFO` level and stderr at `WARN` level — both appear in the installer log stream so failures are always visible.

Supported parameters:

| Parameter | Type | Notes |
|---|---|---|
| `command` | string | Script content (`powershell`/`bash`) or command line (`program`). |
| `shell` | string | `powershell` (Windows only), `bash` (Linux only), or `program` (all platforms). |
| `platform` | string | Optional. Restrict hook to `windows` or `linux`. Omit to run everywhere. |
| `wait` | boolean | Wait for completion before continuing. Default: `true`. |
| `fail_on_nonzero` | boolean | Fail the installation on non-zero exit. Default: `true`. |
| `timeout_sec` | number | Kill and classify as `HG-PS-004` if exceeded. |

### Stable Error Codes

The installer classifies step failures into stable v1 error codes:

- `HG-YAML-001` - manifest validation failure
- `HG-VAR-001` - unresolved installer variable
- `HG-EXTRACT-001` - archive missing from payload
- `HG-EXTRACT-002` - extraction I/O failure
- `HG-COPY-001` - copy source missing or invalid
- `HG-REG-001` - invalid registry configuration
- `HG-REG-002` - registry access denied / elevation required
- `HG-ENV-001` - environment variable operation failure
- `HG-RUN-001` - executable not found
- `HG-RUN-002` - process non-zero exit or execution failure
- `HG-PS-001` - PowerShell syntax/parse error
- `HG-PS-002` - PowerShell/command not found
- `HG-PS-003` - PowerShell non-zero exit
- `HG-PS-004` - PowerShell timeout
- `HG-PS-005` - PowerShell access denied or execution policy blocked

See [ERROR_CODES.md](ERROR_CODES.md) for the full field-by-field format and fix guidance.

---

## Conditional Install Execution

Component selection is now controlled through `install.components` and per-entry `component` fields under system blocks.

Supported `component` fields include `install.system.shortcuts[*].component` and `install.system.path[*].component` (Linux list form) or `install.system.path.component` (single-entry form).

```yaml
components:
  - id: docs
    name: "Documentation"
    required: false
    selected: true

install:
  components:
    docs:
      archive: "docs.zst"
      target: "{{INSTDIR}}/docs"

  system:
    windows:
      shortcuts:
        - name: "Documentation"
          target: "{{INSTDIR}}/docs/manual.txt"
          location: start_menu
          component: docs
```

---

## Linux PATH Integration

On Linux, `install.system.linux.path` controls how the installed binary directory is added to `$PATH`. Two scopes are supported:

| Scope | What it writes | Takes effect |
|---|---|---|
| `user` | Appends to `~/.bashrc` and `~/.profile` for the installing user | New user terminal sessions |
| `system` | Writes `/etc/profile.d/hagane-path.sh` **and** appends to `/etc/bash.bashrc` | All users, all new terminal sessions (login and non-login) |

> **WSL2 note:** WSL2 terminal sessions are non-login interactive shells by default. `/etc/profile.d/` is only sourced for login shells. Using `scope: system` writes to **both** `/etc/profile.d/` and `/etc/bash.bashrc` so the PATH is active in all shell types without requiring a login.

### Single scope (simple case)

```yaml
install:
  system:
    linux:
      path:
        add: "{{INSTDIR}}/bin"
        scope: user               # writes ~/.bashrc and ~/.profile
```

### Both scopes simultaneously (list form)

When you want users to choose between user-only and system-wide PATH, define two entries and gate each on its own component:

```yaml
components:
  - id: user_path
    name: "Add to user PATH"
    description: "Appends to ~/.bashrc and ~/.profile (this user only)."
    selected: true
  - id: system_path
    name: "Add to system PATH"
    description: "Writes /etc/bash.bashrc and /etc/profile.d/ for all users. Requires admin."
    selected: false

install:
  system:
    linux:
      path:
        - add: "{{INSTDIR}}/bin"
          scope: user
          component: user_path
        - add: "{{INSTDIR}}/bin"
          scope: system
          component: system_path
```

Only the selected component's entry is executed at install time. An existing single-entry `path:` block (without a list) continues to work unchanged for backwards compatibility.

---

## High-Level Registry Abstractions

Use high-level actions to avoid repetitive registry write blocks.

### `register_uninstall`

For Add/Remove Programs metadata, use `install.system.register_uninstall`:

```yaml
install:
  system:
    register_uninstall:
      hive: HKLM
      key: "{{UNINSTALL_KEY}}"
      name: "MyApp 2.1.0"
      version: "2.1.0"
      publisher: "Acme Corporation"
      install_location: "{{INSTDIR}}"
      uninstall: "{{INSTDIR}}/uninstall.exe"
      estimated_size_kb: 180224
      no_modify: true
      no_repair: true
```

This expands internally into writes for:

- `DisplayName`
- `DisplayVersion`
- `Publisher`
- `InstallLocation`
- `UninstallString`
- `EstimatedSize` (if provided)
- `NoModify`
- `NoRepair`

Preferred fields in the DSL are `install_location` and `uninstall`.

### `register_app`

For app settings, use `install.system.register_app`:

```yaml
install:
  system:
    register_app:
      hive: HKLM
      key: "{{APP_REG_KEY}}"
      install_location: "{{INSTDIR}}"
      version: "2.1.0"
```

This writes:

- `InstallDir` = `inst_loc`
- `Version` = `version`

---

## Administrator Elevation

Set `app.require_admin` to control whether the installer requests elevation:

```yaml
app:
  require_admin: true
```

Use `true` for operations that need system access:

| Platform | When required |
|---|---|
| Windows | `HKLM` registry writes, system PATH, protected install locations like `C:\Program Files` |
| Linux | Writing to `/usr/local/`, `/etc/profile.d/`, `/etc/bash.bashrc`, or creating symlinks in `/usr/local/bin` |

Use `false` for user-level installs that should not prompt for elevation.

**Linux elevation behavior:** When `require_admin: true` and the installer is not already running as root, it re-launches itself with `sudo` automatically and exits the original process. The elevated re-launch inherits all command-line arguments. The original calling user's home directory is tracked via `$SUDO_USER` so PATH entries are written to the correct user's shell config files.

---

## Theme Customization

All colors, fonts, and sizing are CSS variables injected at runtime from `theme:` in your YAML.
No recompilation needed to rebrand the installer.
For named theme presets and the folder layout used by this repository, see [THEMING_PRESETS.md](THEMING_PRESETS.md).

---

## IDE Autocomplete

Add this comment to the top of your `installer.yaml` for VS Code YAML extension:

```yaml
# yaml-language-server: $schema=../../sdk/schema/installer.schema.json
```

---

## Building from Source

```powershell
# Requirements: Rust stable, Windows SDK, WebView2 SDK
cargo build --release          # builds all crates
cargo build --release -p runner   # just the installer runner
cargo build --release -p builder  # just hagane
```
## Quick Commands

```powershell
cargo build -p builder --bin hagane --release 
Copy-Item .\target\release\hagane.exe .\hagane\payload\bin\hagane.exe -Force 
cargo run -p builder --bin hagane -- run hagane/installer.yaml --release
```

## Useful Commands

```powershell
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\AcmeMyApp" /s
reg query "HKLM\SOFTWARE\Acme\MyApp" /s
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Hagane" /s
reg query "HKLM\SOFTWARE\InstallerEngine\Hagane" /s
```

## Notes

- The runner binary compiles on both **Windows** (WebView2/Win32) and **Linux** (wry + WebKitGTK/GTK3).
- The Windows runner uses `windows-rs` and `webview2-com`. WebView2 Runtime must be installed on the target machine.
- The Linux runner uses `wry 0.43` + `tao 0.30` with WebKitGTK. Build dependencies: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`.
- The engine library and builder compile cross-platform.
