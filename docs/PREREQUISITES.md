# Prerequisites & Installation

Everything you need installed before building Windows installers with Hagane.

## System Requirements

| Requirement | Minimum |
|---|---|
| OS | Windows 10 (x64) build 18362+ or Windows 11 |
| Rust | Stable channel via `rustup` |
| WebView2 Runtime | Pre-installed on Windows 11; installer available for Windows 10 |
| MSVC Build Tools | Visual Studio 2019+ or Build Tools for Visual Studio 2022 |

---

## Install Rust

```powershell
winget install Rustlang.Rustup
```

Restart your terminal, then verify:

```powershell
rustc --version
cargo --version
```

---

## Install WebView2 Runtime

WebView2 ships with Windows 11 by default. For Windows 10:

```powershell
winget install Microsoft.EdgeWebView2Runtime
```

Or download the installer directly from the [WebView2 download page](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

---

## Install MSVC Build Tools

The Rust toolchain for Windows requires MSVC linker and headers. Install via:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet"
```

Or install the full **Visual Studio 2022** IDE which includes these tools.

---

## Install Hagane

Run the Hagane setup executable. It places the CLI at:

```text
C:\Program Files\Hagane\bin\hagane.exe
```

The installer automatically adds this directory to your `PATH`. Verify after installation:

```powershell
hagane --version
```

---

## PATH Setup (Manual)

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

## Build Hagane From Source

If you are developing the engine itself, build the CLI directly:

```powershell
# Build release binary
cargo build --release -p builder --bin hagane

# Verify
.\target\release\hagane.exe --version
```

Use `.\target\release\hagane.exe` in place of `hagane` for all commands when running from source.
