# Prerequisites & Installation

Everything you need installed before building installers with Hagane.

Hagane targets both **Windows** (WebView2/Win32) and **Linux** (wry + WebKitGTK/GTK3). Requirements differ by platform.

---

## Windows

### System Requirements

| Requirement | Minimum |
|---|---|
| OS | Windows 10 (x64) build 18362+ or Windows 11 |
| Rust | Stable channel via `rustup` |
| WebView2 Runtime | Pre-installed on Windows 11; installer available for Windows 10 |
| MSVC Build Tools | Visual Studio 2019+ or Build Tools for Visual Studio 2022 |

### Install Rust

```powershell
winget install Rustlang.Rustup
```

Restart your terminal, then verify:

```powershell
rustc --version
cargo --version
```

### Install WebView2 Runtime

WebView2 ships with Windows 11 by default. For Windows 10:

```powershell
winget install Microsoft.EdgeWebView2Runtime
```

Or download the installer directly from the [WebView2 download page](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

### Install MSVC Build Tools

The Rust toolchain for Windows requires MSVC linker and headers. Install via:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet"
```

Or install the full **Visual Studio 2022** IDE which includes these tools.

### Install Hagane (Windows)

Run the Hagane setup executable. It places the CLI at:

```text
C:\Program Files\Hagane\bin\hagane.exe
```

The installer automatically adds this directory to your `PATH`. Verify after installation:

```powershell
hagane --version
```

### PATH Setup (Manual, Windows)

If Hagane is not found on your `PATH` after installation, add it manually:

```powershell
[Environment]::SetEnvironmentVariable(
    "PATH",
    $env:PATH + ";C:\Program Files\Hagane\bin",
    "User"
)
```

Re-open your terminal after running this.

---

## Linux (Ubuntu / Debian / WSL2)

### System Requirements

| Requirement | Notes |
|---|---|
| OS | Ubuntu 22.04+ / Debian 12+ (or WSL2 with WSLg) |
| Rust | Stable channel via `rustup` |
| WebKitGTK 4.1 | For the installer GUI (wry + GTK3) |
| GTK3 dev headers | Required to compile the runner |
| GCC / Clang | Usually pre-installed; required by `cc` build crate |

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

### Install WebKitGTK and GTK3 Build Dependencies

```bash
sudo apt-get update
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libssl-dev \
    pkg-config \
    build-essential
```

> **WSL2 note:** The GUI requires a display server. WSL2 with WSLg provides `$DISPLAY` automatically on Windows 11. Verify with `echo $DISPLAY` — it should return a non-empty value.

### Install Hagane (Linux)

Run the Hagane Linux installer binary (requires root — the installer re-launches with `sudo` automatically):

```bash
./hagane-linux-x86_64
```

The installer places the CLI at `/usr/local/hagane/bin/hagane` and creates a symlink at `/usr/local/bin/hagane`. Verify in a new terminal:

```bash
hagane --version
```

### PATH Setup (Manual, Linux)

If `hagane` is not found after installation, add the install bin directory to your PATH manually:

```bash
echo 'export PATH="/usr/local/hagane/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
hagane --version
```

---

## Build Hagane From Source

If you are developing the engine itself, build the CLI directly from the workspace root:

### Windows

```powershell
cargo build --release -p builder --bin hagane
.\target\release\hagane.exe --version
```

### Linux

```bash
cargo build --release -p builder --bin hagane
./target/release/hagane --version
```

Use the built binary in place of `hagane` for all commands when running from source.
