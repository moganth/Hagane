# Error Simulation Manifests

These manifests are intentionally designed to fail with specific automatic Hagane error codes.

## How to run a simulation

1. Build installer payload:

```powershell
hagane build --manifest sdk/example/error-sim/<FILE>.yaml
```

2. Compile installer:

```powershell
hagane pack --manifest sdk/example/error-sim/<FILE>.yaml
```

3. Run generated setup EXE from `sdk/example/error-sim` and check:

- UI progress log
- `$INSTDIR\\logs\\installation.log`

## Files

- HG-YAML-001.yaml
- HG-VAR-001.yaml
- HG-EXTRACT-001.yaml
- HG-EXTRACT-002.yaml
- HG-COPY-001.yaml
- HG-REG-001.yaml
- HG-REG-002.yaml
- HG-ENV-001.yaml
- HG-RUN-001.yaml
- HG-RUN-002.yaml
- HG-PS-001.yaml
- HG-PS-002.yaml
- HG-PS-003.yaml
- HG-PS-004.yaml
- HG-PS-005.yaml

## Notes

- `HG-REG-002`, `HG-ENV-001`, and `HG-PS-005` depend on permission context. Run as standard user (non-admin) to reproduce access-denied cases.
- `HG-EXTRACT-001` requires an extract step archive with no matching source folder so it is not embedded.
- Keep these manifests for testing only, not production installers.
