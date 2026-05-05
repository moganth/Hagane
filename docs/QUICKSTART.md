# Quick Start

Build your first installer in under 5 minutes.

This guide shows a **Windows** installer. For Linux-specific steps (WebKitGTK setup, PATH scope, bash hooks, and uninstall) see [hagane.md](hagane.md) and [documentation.md](documentation.md).

## What You Will Build

A standalone Windows setup EXE (`myapp-setup.exe`) that:

- Shows a welcome screen with your app branding
- Checks system requirements natively (no PowerShell, no subprocess)
- Lets the user pick an install directory
- Extracts your app files to the chosen location
- Creates desktop shortcuts and registry entries
- Writes a working uninstaller

---

## 1. Create Your Manifest

Create a folder for your installer project and add `installer.yaml`:

```yaml
# installer.yaml
# yaml-language-server: $schema=../../sdk/schema/installer.schema.json
app:
  name: "MyApp"
  version: "1.0.0"
  publisher: "Acme Corp"
  logo: "assets/logo.png"
  default_install_dir: "{{PROGRAMFILES64}}/Acme/MyApp"

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
      register_app:
        hive: HKCU
        key: "Software/Acme/MyApp"
        version: "1.0.0"
        install_location: "{{INSTDIR}}"
      shortcuts:
        - name: "MyApp"
          target: "{{INSTDIR}}/MyApp.exe"
          location: desktop

  finalize:
    windows:
      write_uninstaller: "{{INSTDIR}}/uninstall.exe"
```

> **Tip**: The `# yaml-language-server` comment gives you field completion and validation in VS Code with the YAML extension installed.

---

## 2. Add Your Payload

Create a `payload/` folder next to `installer.yaml` and put your application files inside. Hagane compresses it automatically into `payload.zst` during the build step.

```text
my-installer/
├── installer.yaml
├── assets/
│   └── logo.png
└── payload/          ← compressed to payload.zst at build time
    └── MyApp.exe
```

---

## 3. Build

Run from the directory containing your `installer.yaml`:

```powershell
hagane run --release
```

> **Auto-Discovery**: When exactly one `.yaml` / `.yml` file is in your working directory, Hagane selects it automatically. No path argument needed.

If multiple YAML files are detected in the directory, Hagane shows a warning and exits cleanly — listing every candidate so you can pick the right one:

![Multiple YAML files detected — Hagane warns and exits](assets/hagane-cli/2.png)

Hagane streams the full build pipeline — banner, manifest validation, payload compression, and `cargo build` output:

![hagane run --release — successful build output](assets/hagane-cli/3.png)

The output EXE is written to `bin/` next to your manifest:

```text
my-installer/
└── bin/
    └── MyApp-setup.exe   ← ready to distribute
```

---

## 4. Run Your Installer

```powershell
# GUI mode
.\bin\MyApp-setup.exe

# Silent / automated install
.\bin\MyApp-setup.exe /S
```

---

## CLI Help

Run `hagane` at any time to see the banner and available commands:

![hagane CLI — ASCII banner and commands overview](assets/hagane-cli/1.png)

---

## What's Next

- Apply a **theme preset** — see [Theming & Presets](#theming)
- Collect extra info from users with a **custom page** — see [Custom Pages](#custom_pages)
- Configure **install logging** for diagnostics — see [Logging](#logging)
- Understand all **error codes** — see [Error Codes](#error_codes)
