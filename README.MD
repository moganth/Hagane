# Installer Engine

A fast, YAML-driven installer framework for Windows built with Rust, WebView2, and Zstd.

## Why

| Problem with NSIS | This engine |
|---|---|
| PowerShell/CMD for system checks → slow, laggy | Native WinAPI calls only — microseconds |
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
  default_install_dir: "$PROGRAMFILES64\\YourCompany\\MyApp"

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

steps:
  - action: extract
    archive: "payload.zst"
    destination: "$INSTDIR"
  - action: shortcut
    target: "$INSTDIR\\MyApp.exe"
    location: desktop
    name: "MyApp"
  - action: write_uninstaller
    path: "$INSTDIR\\uninstall.exe"
```

### 2. Place your payload

```
sdk/example/
├── installer.yaml
├── assets/
│   ├── logo.png
│   ├── banner.png
│   └── icon.ico
└── payload/          ← folder named after archive (without .zst)
    ├── MyApp.exe
    └── ...
```

### 3. Build

```powershell
# Install the builder
cargo build --release -p builder

# Package and compile the installer
cd sdk/example
..\..\target\release\iebuild.exe --manifest installer.yaml --build --compression-level 9

# Output: installer-engine/target/release/myapp-setup.exe
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

The installer supports full theme customization via optional manifest fields:

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
  border_radius: 6                    # Corner roundness (px)
  
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

theme_affects:
  - All buttons and interactive elements
  - Progress bar gradient (start and end colors)
  - Requirement check icons and card backgrounds
  - Success/error messages and backgrounds
  - Window and component layouts
  - Text and border colors throughout

```

### All Theme Fields Are Optional

Every field in the `theme` block is completely optional. Omit any field and the installer will use a sensible built-in default.

| Field | Default | Purpose |
|-------|---------|---------|
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
All other colors fall back to defaults. Install windows work perfectly.

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

- You **must** create your own `installer.yaml` (app name, pages, steps, requirements, etc.).
- You do **not** ship `installer.yaml` to end users — it is embedded into the generated setup EXE at build time.
- `installer.schema.json` is optional at runtime, but strongly recommended during authoring for IDE validation/autocomplete.

### Minimal author workflow

1. Copy `sdk/example/installer.yaml` and edit it for your app.
2. Create payload folders next to your manifest (for each `extract` archive name).
3. Build your setup EXE:

```powershell
cd <workspace-root>
cargo build --release -p builder
./target/release/iebuild.exe --manifest ./path/to/installer.yaml --build
```

4. Distribute only the output setup EXE (for example `myapp-setup.exe`).

### Enable YAML schema in VS Code

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
├── builder/                   # iebuild CLI — compresses & packages installer
├── ui/
│   ├── pages/                 # HTML pages (welcome, license, requirements…)
│   └── assets/                # style.css, bridge.js
└── sdk/
    ├── example/               # Example installer.yaml + assets
    └── schema/                # installer.schema.json for IDE support
```

---

## Requirements Checks (all native, no PowerShell)

| Check | API used |
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

## Available Step Actions

| action | Description |
|---|---|
| `extract` | Decompress Zstd+tar archive to destination |
| `copy_file` | Copy file with optional overwrite + backup |
| `delete_file` | Delete a file |
| `create_dir` | Create directory (and parents) |
| `registry` | Write/delete registry keys and values |
| `shortcut` | Create .lnk shortcut (desktop/start menu/startup) |
| `env_var` | Set/append/prepend to environment variables |
| `service` | Install/start/stop/delete Windows services |
| `run_program` | Execute a program (optionally wait) |
| `write_uninstaller` | Write the auto-generated uninstaller |

---

## Variables in Paths

| Variable | Resolves to |
|---|---|
| `$INSTDIR` | Chosen installation directory |
| `$PROGRAMFILES` | `C:\Program Files (x86)` |
| `$PROGRAMFILES64` | `C:\Program Files` |
| `$APPDATA` | `C:\Users\<user>\AppData\Roaming` |
| `$LOCALAPPDATA` | `C:\Users\<user>\AppData\Local` |
| `$TEMP` | Temp directory |
| `$WINDIR` | `C:\Windows` |

---

## Theme Customization

All colors, fonts, and sizing are CSS variables injected at runtime from `theme:` in your YAML.
No recompilation needed to rebrand the installer.

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
cargo build --release -p builder  # just iebuild
```

> **Note:** The runner requires Windows to compile (uses `windows-rs` and `webview2-com`).
> The engine library and builder compile cross-platform for testing.