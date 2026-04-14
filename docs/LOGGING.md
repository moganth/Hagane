# Hagane Logging Guide

This guide explains exactly how installer logging works with the `install` DSL, what `auto` mode does, what `manual_only` mode does, and how to choose between them.

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

In `mode: auto`, Hagane emits lifecycle logs for every compiled install operation:

- Start of step: info
- Slow step (elapsed >= slow_step_warn_sec): warn
- Step failure: error with classified HG-* code
- Rollback failures: error

When file logging is configured, completion lines remain in the log file but are not echoed into the UI log box. This avoids showing the same lifecycle text twice in the visible installer UI.

Skip behavior in auto mode:

- If a compiled operation has `component` and that component is not selected, the operation is skipped.
- Hagane emits an info skip line showing the reason.

### Manual-only mode

In `mode: manual_only`, normal execution logs are suppressed:

- Hagane does not auto-emit lifecycle start/success/skip messages.
- Slow-step warnings are not emitted for normal operations.

Important: failures are still always logged.

- Classified step failures (HG-* lines) are emitted in both modes.
- Rollback errors are emitted in both modes.

This keeps troubleshooting reliable even when normal logs are fully manual.

## Install DSL logging scope

The top-level `install` DSL does not expose per-operation inline `log` blocks. Logging is controlled by `logging.mode` and emitted from the compiled execution plan.

Rules:

- File logging requires both `logging.path` and `logging.file_name`.
- `auto` mode gives lifecycle visibility with minimal YAML.
- `manual_only` is best when you only want failure-classification output during install execution.

## Recommended usage patterns

### Pattern A: Fully automatic lifecycle logging

Use this when you want full traceability with minimal YAML noise.

```yaml
logging:
  mode: auto
  path: "{{TEMP}}/MyAppLogs"
  file_name: "installation.log"
  slow_step_warn_sec: 10

install:
  setup:
    create_dirs:
      - "{{INSTDIR}}"

  components:
    core:
      archive: "payload.zst"
      target: "{{INSTDIR}}"

  finalize:
    write_uninstaller: "{{INSTDIR}}/uninstall.exe"
```

What you get:

- Automatic start/success lines for each step
- Automatic slow-step warnings
- Automatic classified failures

### Pattern B: Failure-focused logs

Use this when you only want deterministic classified failures during install execution.

```yaml
logging:
  mode: manual_only
  path: "{{TEMP}}/MyAppLogs"
  file_name: "installation.log"

install:
  setup:
    create_dirs:
      - "{{INSTDIR}}"

  components:
    core:
      archive: "payload.zst"
      target: "{{INSTDIR}}"

  finalize:
    write_uninstaller: "{{INSTDIR}}/uninstall.exe"
```

What you get:

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

- File logging always requires top-level `logging.path` and `logging.file_name`.
- `logging.slow_step_warn_sec` must be greater than 0 when present.
- For hooks, `run.command` and `run.shell` are required.

## Quick decision guide

Use `auto` when:

- You want complete step lifecycle visibility by default.
- You want fewer per-step log entries in YAML.
- You want easier support diagnostics.

Use `manual_only` when:

- You want minimal normal logging output.
- You rely on HG error codes and rollback logs for diagnostics.
- You are validating error handling behavior.
