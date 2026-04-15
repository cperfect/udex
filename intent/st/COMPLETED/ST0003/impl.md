# Implementation - ST0003: Implement Security Scanning of repository

## Implementation

Trivy filesystem scanning is wired into the repo at three levels:

1. **Devcontainer** — Trivy installed via the official Aqua Security apt repository in `.devcontainer/Dockerfile`, available immediately on container rebuild.
2. **Local** — developers run `trivy fs --config .trivy.yaml .` from the repo root; output is identical to CI.
3. **CI** — `02-Security.yml` GitHub Actions workflow runs `aquasecurity/trivy-action` pointed at `.trivy.yaml` on every push and PR to `main`.

The blocking threshold is MEDIUM and above. UNKNOWN and LOW findings are reported but do not fail the build. Accepted findings are suppressed via `.trivyignore` with a mandatory comment explaining the rationale.

## Code Examples

### Run scan locally (same as CI)

```bash
trivy fs --config .trivy.yaml .
```

### Suppress an accepted finding

```
# .trivyignore
CVE-2021-12345  # Not exploitable — affected code path is unreachable in this binary
```

### Check what Trivy would find without failing

```bash
trivy fs --config .trivy.yaml --exit-code 0 .
```

## Technical Details

- **Scan types**: `vuln` (Cargo.lock CVEs), `secret` (hardcoded credentials), `misconfig` (GitHub Actions / TOML / YAML)
- **Exit code**: `1` when any finding at MEDIUM or above is detected; `0` otherwise
- **Severity mapping**: Trivy has no WARN level; MEDIUM is the closest equivalent to "warn/minor or higher"
- **DB caching**: `trivy-action` caches the vulnerability database between CI runs automatically
- **Devcontainer install**: uses the signed apt repository (`aquasecurity.github.io/trivy-repo`) — same source recommended in the Trivy docs for Debian-based systems

## Challenges & Solutions

- **Trivy has no WARN level**: The requirement said "WARN/minor or higher". Trivy's scale is UNKNOWN < LOW < MEDIUM < HIGH < CRITICAL. MEDIUM was chosen as the blocking threshold as the closest practical equivalent to "minor".
- **Keeping local and CI in sync**: Using a shared `.trivy.yaml` that both the local binary and `trivy-action` read eliminates the risk of the two environments diverging.
