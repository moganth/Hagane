# Hagane Installer Error Codes and Logging Guide

This document defines stable v1 error codes and logging behavior in `installer.yaml`.

## Goals

- Show actionable install failures to users.
- Keep logging behavior deterministic across auto and manual modes.
- Provide consistent error codes for support and troubleshooting.

## Logging Modes and Hooks

Hagane logging is configured globally and applies to the compiled operations from `install`.
Post-install commands are defined in `install.hooks.post_install`.

### Logging Configuration Block

Add a top-level `logging` block:

```yaml
logging:
  mode: auto                    # "auto" or "manual_only" (default: auto)
  path: "{{INSTDIR}}/logs"        # Directory to store log files
  file_name: "installation.log"  # Log file name
  timestamp: true                # Prefix each log line with ISO timestamp (default: true)
  include_raw_os_error: false    # Include raw OS error details in auto-logged errors (default: false)
  slow_step_warn_sec: 10         # Warn threshold in seconds for long-running steps
```

#### Logging Configuration Parameters

| Parameter | Type | Default | Required? | Notes |
|-----------|------|---------|-----------|-------|
| `mode` | string | `auto` | No | `auto` logs lifecycle messages (start/warn/success) and classified failures. `manual_only` suppresses normal lifecycle logging during execution. |
| `path` | string | — | Recommended | Installation directory must be resolvable. Supports `{{INSTDIR}}`, `{{PROGRAMFILES}}`, `{{APPDATA}}`, `{{LOCALAPPDATA}}`. |
| `file_name` | string | — | Recommended | Name of the log file (e.g., `installation.log`, `setup.log`). |
| `timestamp` | boolean | `true` | No | If `true`, each log line is prefixed with ISO 8601 timestamp (e.g., `2026-04-12T14:32:01.234Z`). |
| `include_raw_os_error` | boolean | `false` | No | If `true`, automatic error classification includes raw Windows OS error details (may expose implementation details). |
| `slow_step_warn_sec` | integer | `10` | No | Threshold in seconds for slow-step warning lines. Must be greater than 0. |

### Install DSL Logging Behavior

Log level is automatic in `mode: auto`:

- Start-of-operation log: `info`
- Long-running operation notice: `warn`
- Failures: `error`

**Note**: When file logging is desired, set both `logging.path` and `logging.file_name`.

## Conditional Execution with Components

Component-based operations are skipped if that component is not selected by the user during installation.

### Supported DSL Locations with `component`

- `install.system.shortcuts[*].component`
- `install.system.path.component`

### Example: Component-Based Installation

```yaml
components:
  - id: core
    name: "Core Application"
    required: true
    selected: true
  - id: docs
    name: "Documentation"
    required: false
    selected: true
  - id: dev_tools
    name: "Developer Tools"
    required: false
    selected: false

install:
  components:
    core:
      archive: "core.zst"
      target: "{{INSTDIR}}"
    docs:
      archive: "docs.zst"
      target: "{{INSTDIR}}/docs"
    dev_tools:
      archive: "devtools.zst"
      target: "{{INSTDIR}}/devtools"

  system:
    shortcuts:
      - name: "Developer Shell"
        target: "{{INSTDIR}}/devtools/shell.exe"
        location: start_menu
        component: dev_tools
    path:
      add: "{{INSTDIR}}/devtools/bin"
      scope: user
      component: dev_tools
```

## Post-Install Command Hooks

`install.hooks.post_install` executes commands with automatic error classification.

### PowerShell Hook Execution

Execute a PowerShell command:

```yaml
install:
  hooks:
    post_install:
      - run:
          command: |
            Write-Host "Hello from installer"
            [Environment]::SetEnvironmentVariable("MY_VAR", "value", "User")
          shell: powershell
          wait: true
          fail_on_nonzero: true
          timeout_sec: 30
```

### Program Hook Execution

Execute a normal process command:

```yaml
install:
  hooks:
    post_install:
      - run:
          command: "{{INSTDIR}}/bin/post_install.exe --mode setup"
          shell: program
          wait: true
          fail_on_nonzero: true
          timeout_sec: 60
```

#### `run` Parameters

| Parameter | Type | Default | Required? | Notes |
|-----------|------|---------|-----------|-------|
| `command` | string | — | Yes | Command text to execute. For PowerShell, this is script content; for program mode, this is the command line. |
| `shell` | string | — | Yes | `powershell` or `program`. |
| `wait` | boolean | `true` | No | If `true`, the installer waits for the script to complete before continuing. If `false`, script runs in background. |
| `fail_on_nonzero` | boolean | `true` | No | If `true`, a non-zero exit code from the script fails the entire installation. If `false`, non-zero exits are ignored. |
| `timeout_sec` | number | (none) | No | Maximum execution time in seconds. If the script exceeds this time and `wait=true`, it's terminated and classified as `HG-PS-004` (timeout). |

#### Hook Execution Notes

- **Error Action Preference**: Scripts are automatically wrapped with `$ErrorActionPreference='Stop'` to ensure deterministic error codes.
- **Elevation**: If the installer runs with admin elevation, scripts execute with the same privileges.
- **Output**: Command output is captured and included in automatic error classification (HG-PS-001 through HG-PS-005 when using PowerShell).
- **Working Directory**: Hooks inherit the installer's working directory (`{{INSTDIR}}`).

## Error Line Format

When an installation step fails and logging is enabled (default `mode: auto`), the installer automatically generates an error line in the following format:

```text
[ERROR] <CODE> step=<N> action=<ACTION> field=<FIELD> value=<VALUE> reason="..." fix="..."
```

### Error Line Components

| Component | Description | Example |
|-----------|-------------|---------|
| `CODE` | 9-character stable v1 error code (HG-XXXX-NNN) | `HG-EXTRACT-001` |
| `step` | Compiled operation index (1-indexed) | `step=4` |
| `action` | The operation action type | `action=extract` |
| `field` | The problematic field in the operation configuration | `field=archive` |
| `value` | The value of that field (may be truncated) | `value=docs.zst` |
| `reason` | Root cause of the failure (auto-classified) | `reason="archive 'docs.zst' is missing from embedded payload"` |
| `fix` | Recommended action to resolve the error | `fix="Run hagane build again and ensure the archive source folder exists near installer.yaml."` |

### Example Error Log Line

```text
[ERROR] HG-EXTRACT-001 step=4 action=extract field=archive value=docs.zst reason="archive 'docs.zst' is missing from embedded payload" fix="Run hagane build again and ensure the archive source folder exists near installer.yaml."
```

## Stable v1 Error Codes (Complete Reference)

### Manifest and Variable Resolution

#### `HG-YAML-001` — Manifest Schema Validation Failure

**When it occurs:** During manifest parsing or validation, the installer detects invalid YAML structure, missing required fields, or invalid field values.

**Common causes:**
- Missing `app.name`, `app.version`, or `app.publisher`
- Missing `pages` with at least one `install` type page
- Invalid registry hive name (not one of: HKLM, HKCU, HKCR, HKU, HKCC)
- Invalid environment scope (`scope` must be `user` or `system`)
- Invalid environment operation (`operation` must be `set`, `append`, or `prepend`)
- Missing or malformed `install` block
- Hook with missing `run.command` or `run.shell`

**Typical fix:** Review the error reason and correct the manifest YAML. Re-run `hagane build` to re-validate.

**Field mapping in error line:**
```
field=key field (e.g., app.name, logging.path, [registry|env|step] configuration)
value=malformed value
```

#### `HG-VAR-001` — Unresolved Installer Variable

**When it occurs:** During step execution, a path-like field contains a variable reference that cannot be resolved.

**Supported variables:**
- `{{INSTDIR}}` (or legacy `$INSTDIR`) — installation directory
- `{{PROGRAMFILES}}` (or legacy `$PROGRAMFILES`) — typically C:\Program Files
- `{{PROGRAMFILES64}}` (or legacy `$PROGRAMFILES64`) — 64-bit variant
- `{{APPDATA}}` (or legacy `$APPDATA`) — per-user Application Data folder
- `{{LOCALAPPDATA}}` (or legacy `$LOCALAPPDATA`) — per-user Local\Application Data folder
- `{{TEMP}}` (or legacy `$TEMP`) — user temp directory
- `{{WINDIR}}` (or legacy `$WINDIR`) — Windows system directory
- Any custom variable declared in top-level `variables:` (for example `{{APP_REG_KEY}}`)

**Common causes:**
- Misspelled variable name (e.g., `{{INSTDIR_ROOT}}` instead of `{{INSTDIR}}`)
- Typo inside braces (e.g., `${INSDIR}`)
- Custom variables (not supported)

**Typical fix:** Correct the variable name and re-run the installer.

**Authoring note:** Custom variables must be declared in `variables:` and use keys with `A-Z`, `0-9`, and `_` (optionally prefixed with `$`). Use `{{KEY}}` as the preferred template syntax.

---

### Extract and Copy Operations

#### `HG-EXTRACT-001` — Archive Missing from Payload

**When it occurs:** An `extract` action references an archive name that was not embedded during build.

**Cause:** The archive source folder specified in the build command does not exist, or its contents were not found.

**Typical fix:**
1. Verify the archive source directory exists in your build context.
2. Re-run `hagane build` to re-scan and embed archives.
3. Verify the `archive` field in the manifest matches the embedded name.

**Field mapping:**
```
field=archive
value=<archive name from manifest>
```

#### `HG-EXTRACT-002` — Extraction I/O Failure or Destination Not Writable

**When it occurs:** The extraction process fails due to I/O errors, permission issues, or an invalid destination path.

**Common causes:**
- Destination directory is on a read-only drive
- Destination path contains invalid characters or is malformed
- File locking (e.g., antivirus or file explorer blocking writes)
- Insufficient disk space
- Running without elevation when destination is protected (e.g., C:\Program Files under some Windows configurations)

**Typical fix:** Check destination path permissions, close file locks, ensure adequate free disk space, or run the installer with elevated privileges.

**Field mapping:**
```
field=destination
value=<destination path from manifest>
```

#### `HG-COPY-001` — Copy Source File Missing or Invalid

**When it occurs:** A `copy_file` action references a source file that does not exist or cannot be read.

**Common causes:**
- Source file was not extracted or copied in a previous step
- Source path typo or incorrect variable resolution
- Source file permission-denied (rare)
- Source file was deleted between build and installation

**Typical fix:** Verify the source file path in the manifest and confirm previous `extract` or `copy_file` steps produce the expected file.

**Field mapping:**
```
field=source
value=<source path from manifest>
```

---

### Registry Operations

You can either use low-level `registry` actions or high-level actions:

- `register_uninstall` for Add/Remove Programs metadata
- `register_app` for app settings (`InstallDir`, `Version`)

#### `HG-REG-001` — Invalid Registry Configuration

**When it occurs:** A `registry` step contains structurally invalid configuration (invalid hive, key format, or type).

**Common causes:**
- Invalid `hive` name (must be: HKLM, HKCU, HKCR, HKU, or HKCC)
- `value_type` mismatch (e.g., `BINARY` type with string value)
- Invalid key path syntax
- Missing required fields for the operation
- Missing required `register_uninstall` fields (name/version/publisher/install location/uninstall string)
- Missing required `register_app` fields (`inst_loc`/`install_location`, `version`)

**Typical fix:** Validate the registry hive, key, value type, and value format. Test with regedit if unsure.

**Field mapping:**
```
field=key
value=<HIVE\Key path>
```

#### `HG-REG-002` — Registry Access Denied (Permission Required)

**When it occurs:** A `registry` action attempts to write to a protected hive or key that requires elevated privileges.

**Common registry locations requiring admin:**
- `HKLM\Software\*` — HKEY_LOCAL_MACHINE (system-wide)
- `HKCR\*` — HKEY_CLASSES_ROOT (system-wide)

**Typical fix:**
1. **For system-wide settings:** Ensure installer runs with **administrator elevation**. Set `app.require_admin: true` in manifest.
2. **For user-only settings:** Use `HKCU` instead of `HKLM`.
3. **At build time:** Verify `app.require_admin` is set correctly in `installer.yaml`.

**Field mapping:**
```
field=key
value=<HIVE\Key path>
```

---

### Environment Variables

#### `HG-ENV-001` — Environment Variable Operation Failed

**When it occurs:** An `env_var` step fails due to invalid scope, operation, or permission issues.

**Valid Scope Values:**
- `user` — per-user environment (HKEY_CURRENT_USER)
- `system` — system-wide environment (HKEY_LOCAL_MACHINE) — **requires admin elevation**

**Valid Operation Values:**
- `set` — replace or create variable
- `append` — add to the end of existing value (with `;` separator)
- `prepend` — add to the beginning of existing value (with `;` separator)

**Common causes:**
- Invalid scope or operation spelling
- Attempting system-wide operation (`scope: system`) without elevation
- Registry write failure due to ACLs

**Typical fix:**
1. Validate `scope` and `operation` values in manifest.
2. **For system scope:** Run installer as Administrator.
3. **For user scope:** No elevation required; ensure registry path is writable.

**Field mapping:**
```
field=operation
value=scope=<scope>, operation=<operation>
```

---

### Program Execution

#### `HG-RUN-001` — Executable Not Found

**When it occurs:** A `run_program` action references an executable that cannot be located in PATH or at the specified path.

**Common causes:**
- Executable not extracted or copied in previous steps
- Path typo or incorrect variable resolution
- File extension missing (e.g., `.exe`)
- Executable is for a different architecture or OS

**Typical fix:**
1. Verify the executable was extracted or copied by a previous step.
2. Check the path in the manifest; ensure variables resolve correctly.
3. Test the executable exists by extracting manually and checking with file explorer.

**Field mapping:**
```
field=executable
value=<executable path/filename from manifest>
```

#### `HG-RUN-002` — Process Non-Zero Exit Code

**When it occurs:** A `run_program` step executes but returns a non-zero exit code, indicating failure.

**Common causes:**
- Program encountered an error (e.g., setup hook failed)
- Program requires arguments or configuration not provided
- Program dependency is missing (e.g., .NET Framework, library)
- Permission issue during program execution

**Typical fix:**
1. Review the program's documentation and verify arguments are correct.
2. Ensure all dependencies are present in the system or bundled.
3. If the program's failure should not stop installation, set `fail_on_nonzero: false` in `run_program` definition.
4. Run the program manually from the install directory to see detailed error output.

**Field mapping:**
```
field=executable
value=<executable path from manifest>
```

---

### PowerShell Execution

#### `HG-PS-001` — PowerShell Parse/Syntax Error

**When it occurs:** PowerShell script contains syntax errors and cannot be parsed.

**Common causes:**
- Unclosed quotes or braces
- Misspelled keywords (e.g., `if ()` with invalid condition)
- Invalid pipeline syntax
- Escape character issues in YAML (remember PowerShell needs correct escaping)

**Typical fix:**
1. Test the script manually in PowerShell: `powershell -NoProfile -File script.ps1`
2. Fix syntax errors.
3. Use inline `script:` in manifest for simple code and external `.ps1` files for complex scripts.

**Field mapping:**
```
field=script
value=<script name or first line>
```

#### `HG-PS-002` — PowerShell/Command Not Found

**When it occurs:** The PowerShell executable cannot be invoked, or a referenced cmdlet/command is not available.

**Common causes:**
- PowerShell not in PATH (rare on Windows)
- Cmdlet does not exist or is misspelled
- Cmdlet from a module that is not imported
- Module path issues in restricted environments

**Typical fix:**
1. Verify PowerShell exists on the target system (should be standard on Windows 7+).
2. Verify cmdlet names and module availability.
3. Wrap cmdlet usage in full namespace if needed (e.g., `Microsoft.PowerShell.Utility\Get-Member`).
4. Pre-import modules at the start of the script if needed.

**Field mapping:**
```
field=script
value=<script name or first line>
```

#### `HG-PS-003` — PowerShell Non-Zero Exit Code

**When it occurs:** PowerShell script executes but exits with a non-zero code (script logic error or explicit exit call).

**Common causes:**
- Script explicitly calls `exit 1` or similar
- Terminating error occurs and `$ErrorActionPreference` is not set to `Stop` (handled automatically)
- Script returns a falsy value that PowerShell interprets as failure

**Typical fix:**
1. Review script logic near the end; check for explicit `exit` calls.
2. Ensure error handling is correct (`try`/`catch` blocks).
3. Test script manually and note the exact PowerShell error.

**Field mapping:**
```
field=script
value=<script name or first line>
```

#### `HG-PS-004` — PowerShell Timeout

**When it occurs:** A PowerShell script runs longer than the specified `timeout_sec` and is terminated.

**Cause:** Script took too long to execute, likely due to waiting for resources, network operations, or infinite loops.

**Typical fix:**
1. Increase `timeout_sec` in the manifest (if the delay is expected).
2. Optimize the script to reduce execution time.
3. Break long-running operations into smaller async tasks.

**Field mapping:**
```
field=timeout_sec
value=<timeout value in seconds>
```

#### `HG-PS-005` — PowerShell Access Denied / Execution Policy Blocked

**When it occurs:** PowerShell script is blocked by execution policy, security manager, or access control.

**Common causes:**
- Script Execution Policy is `Restricted` (default on non-admin shells)
- Script is not digitally signed but policy requires it
- User lacks permissions to run scripts
- Antivirus or security software blocked execution

**Typical fix:**
1. **For user scripts:** Run installer with elevated privileges (admin), which relaxes policy temporarily.
2. **Persistent:** Edit ExecutionPolicy (requires admin): `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`
3. **For secure environments:** Request script signing or provide security exemption.
4. **In manifest:** Set `app.require_admin: true` to ensure elevated context during script execution.

**Field mapping:**
```
field=script
value=<script name or first line>
```

---

## Recommended Authoring Pattern

### Manifest Structure with Logging

Here's a complete example showing best practices for using logging and error detection:

```yaml
app:
  name: "My Application"
  version: "1.0.0"
  publisher: "Acme Inc"
  require_admin: false        # Set to true if registry (HKLM) or system env vars are needed

logging:
  mode: auto                  # "auto" (default) or "manual_only"
  path: "{{INSTDIR}}/logs"
  file_name: "installation.log"
  timestamp: true
  include_raw_os_error: false

pages:
  - type: welcome
    title: "Welcome to My Application"
  - type: requirements
  - type: install_dir
  - type: install
  - type: finish

install:
  setup:
    create_dirs:
      - "{{INSTDIR}}"
      - "{{INSTDIR}}/logs"

  components:
    core:
      archive: "app_binaries.zst"
      target: "{{INSTDIR}}/bin"

  system:
    register_app:
      hive: HKCU
      key: "Software/Acme/MyApp"
      version: "1.0.0"
      install_location: "{{INSTDIR}}"

    path:
      add: "{{INSTDIR}}/bin"
      scope: user

  hooks:
    post_install:
      - run:
          command: |
            Write-Host "Configuring application at {{INSTDIR}}"
          shell: powershell
          wait: true
          fail_on_nonzero: true
          timeout_sec: 60

  finalize:
    write_uninstaller: "{{INSTDIR}}/Uninstall.exe"
```

### Error Handling and Troubleshooting Pattern

When errors occur, use the logged error line to diagnose:

1. **Extract the error code** from the log: `[ERROR] HG-XXXXX-NNN ...`
2. **Find the code section** in this document
3. **Read the `field` and `value`**: These describe what went wrong
4. **Review the `fix`**: Recommended resolution steps

**Example log output:**
```
[ERROR] HG-ENV-001 step=8 action=env_var field=operation value=scope=system, operation=set reason="registry open failed" fix="Use scope user/system and operation set/append/prepend. For system scope, run installer elevated."
```

**Interpretation:**
- Step 8 tried to set a system environment variable
- The installer was not run with admin privileges
- **Fix:** Set `app.require_admin: true` in manifest or run installer as Administrator

## Troubleshooting Checklist

If users report an error code:

1. **Extract the error code** from the log line: `HG-XXXX-NNN`
2. **Find the code** in the "Stable v1 Error Codes" section above
3. **Read the description** to understand what failed
4. **Review the `field`, `value`, `reason`, and `fix`** from the error line
5. **Apply the recommended fix:**
   - Correct `installer.yaml` configuration
   - Fix manifest logic (e.g., use correct registry hives, variables, operators)
   - Update payload layout or ensure proper extraction order
   - For elevation issues: Set `app.require_admin: true`
6. **Re-build and re-test:**
  - Run `hagane run installer.yaml --release` to build the installer
   - Test the installer in silence mode: `installer-setup.exe /S`
   - Check `%TEMP%\HaganeInstall\logs\installation.log` for detailed error output
7. **Iterate** until the error no longer appears

## Logging Configuration Best Practices

1. **Include a top-level logging block** when you need file output.
2. **Use mode: auto** (default) for automatic error classification — provides rich context
3. **Use mode: manual_only** only if you want to suppress normal lifecycle logs
4. **Set timestamp: true** for debugging sequences of steps
5. **Store logs in user-writable location** like `{{TEMP}}` for non-admin testing, `{{INSTDIR}}` for normal installs
6. **Review logs after test failures** to extract error codes and diagnose issues

## Compatibility Notes

- **Stable v1 Codes**: These 15 codes are stable and will not change in v1. New failure types will receive new code families.
- **Field/Value changes**: The `field`, `value`, `reason`, and `fix` sections may be enhanced but will maintain semantic compatibility.
- **Variable Support**: The variable list ({{INSTDIR}}, {{PROGRAMFILES}}, etc.) is fixed v1. New variables will only be added in v2+.
- **Scope/Operation Enums**: Environment scope and operation values are fixed v1 (user, system; set, append, prepend).
