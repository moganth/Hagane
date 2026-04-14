# Hagane Logging Guide

This guide explains exactly how installer logging works, what auto mode does, what manual_only mode does, and how to choose between them.

## What logging controls

Hagane logs can go to two destinations:

- Installer UI log stream
- Optional log file on disk

Top-level logging config:

```yaml
logging:
  mode: auto
  path: "{{INSTDIR}}/logs"
  file_name: "installation.log"
  timestamp: true
  include_raw_os_error: false
  slow_step_warn_sec: 10
```

Field meanings:

- mode: `auto` or `manual_only`.
- path: folder for log file output.
- file_name: log file name.
- timestamp: when true, each file log line includes local timestamp.
- include_raw_os_error: reserved compatibility field for diagnostics policy.
- slow_step_warn_sec: threshold in seconds for slow-step warning lines. Must be greater than 0.

## Exact mode behavior

### Auto mode

In `mode: auto`, Hagane emits lifecycle logs for every executed step:

- Start of step: info
- Slow step (elapsed >= slow_step_warn_sec): warn
- Step failure: error with classified HG-* code
- Rollback failures: error

When file logging is configured, completion lines remain in the log file but are not echoed into the UI log box. This avoids showing the same lifecycle text twice in the visible installer UI.

Skip behavior in auto mode:

- If a step has `component` and that component is not selected, the step is skipped.
- Hagane emits an info skip line showing the reason.

Inline `log` blocks in auto mode:

- If a step has inline `log`, that explicit message is used as the start message for that step.
- If no inline `log` is present, Hagane emits a default start message.

### Manual-only mode

In `mode: manual_only`, normal execution logs are explicit:

- Hagane does not auto-emit lifecycle start/success/skip messages.
- Only inline `log` blocks produce normal step log messages.
- Slow-step warnings are emitted only for steps that have inline `log`.

Important: failures are still always logged.

- Classified step failures (HG-* lines) are emitted in both modes.
- Rollback errors are emitted in both modes.

This keeps troubleshooting reliable even when normal logs are fully manual.

## Inline step log block

You can attach a `log` object to executable steps.
Use exactly one of:

- `log.ui`
- `log.file`
- `log.both`

Example:

```yaml
steps:
  - action: extract
    log:
      both: "Extracting core files"
    archive: "payload.zst"
    destination: "{{INSTDIR}}"
```

Rules:

- `log.ui`: writes to UI only.
- `log.file`: writes to file only (requires logging.path + logging.file_name).
- `log.both`: writes to both UI and file (requires logging.path + logging.file_name).
- Inline log messages are emitted at info level.

## Recommended usage patterns

### Pattern A: Fully automatic lifecycle logging

Use this when you want full traceability with minimal YAML noise.

```yaml
logging:
  mode: auto
  path: "{{TEMP}}/MyAppLogs"
  file_name: "installation.log"
  slow_step_warn_sec: 10

steps:
  - action: create_dir
    path: "{{INSTDIR}}"

  - action: extract
    archive: "payload.zst"
    destination: "{{INSTDIR}}"

  - action: write_uninstaller
    path: "{{INSTDIR}}/uninstall.exe"
```

What you get:

- Automatic start/success lines for each step
- Automatic slow-step warnings
- Automatic classified failures

### Pattern B: Manual narrative logs

Use this when you want tightly curated user-facing wording.

```yaml
logging:
  mode: manual_only
  path: "{{TEMP}}/MyAppLogs"
  file_name: "installation.log"

steps:
  - action: create_dir
    log:
      both: "Preparing installation directory"
    path: "{{INSTDIR}}"

  - action: extract
    log:
      both: "Deploying application payload"
    archive: "payload.zst"
    destination: "{{INSTDIR}}"
```

What you get:

- Only the messages you wrote in inline `log`
- No automatic lifecycle chatter for successful execution
- Still gets classified error lines on failure

## Example output

Auto mode success path (illustrative):

```text
[INFO] Starting step 1/3: Creating directory C:\Program Files\Acme\MyApp
[INFO] Completed step 1/3: Creating directory C:\Program Files\Acme\MyApp
[INFO] Starting step 2/3: Extracting payload.zst
[WARN] Extracting payload.zst is taking longer than expected (12s)
[INFO] Completed step 2/3: Extracting payload.zst
```

Failure output (both modes):

```text
[ERROR] HG-EXTRACT-001 step=2 action=extract field=archive value=payload.zst reason="archive 'payload.zst' is missing from embedded payload" fix="Run hagane build again and ensure the archive source folder exists near installer.yaml."
[ERROR] Rollback error: ...
```

## Validation rules to remember

- If any step uses `log.file` or `log.both`, top-level `logging.path` and `logging.file_name` are required.
- `logging.slow_step_warn_sec` must be greater than 0 when present.
- For `run_powershell`, exactly one of `script` or `file` must be set.

## Quick decision guide

Use `auto` when:

- You want complete step lifecycle visibility by default.
- You want fewer per-step log entries in YAML.
- You want easier support diagnostics.

Use `manual_only` when:

- You need strict editorial control over visible normal logs.
- You want to minimize log volume and only show selected messages.
- You are building a highly curated install narrative.
