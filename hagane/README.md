# Hagane Package

This folder is the package root for the Hagane installer itself.

Minimal layout:

- bin/ — install output directory inside the target machine install root
- assets/ — optional brand assets later
- docs/ — optional docs later
- payload/ — staged files to package into the installer
- installer.yaml — minimal installer manifest for Hagane

Build flow:

1. Place the Hagane payload in `hagane/payload` before packaging.
2. Run `hagane run installer.yaml --release`.
3. The generated installer should install `hagane.exe` into `C:\Program Files\Hagane\bin`.

PATH scope choices are modeled as components in the manifest for now:

- Add to user PATH
- Add to system PATH
