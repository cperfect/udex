---
verblock: "15 Apr 2026:v0.1: vscode - Initial design"
---

# ST0003: Security Scanning Design

## Overview

Trivy scans the repository for four concern types: OS/language vulnerabilities, secrets, misconfigurations, and licence issues. A single shared configuration file (`.trivy.yaml`) drives both local runs and the GitHub Actions workflow so the results are identical in both environments.

## Design Decisions

### D1: Single shared Trivy config file (`.trivy.yaml`)

All scan parameters live in `.trivy.yaml` at the repo root. The local `trivy` binary and the GitHub Action both point at this file so there is no drift between environments.

### D2: Scan scope — filesystem scan of the whole repo

`trivy fs .` covers:
- Cargo dependency manifests (`Cargo.lock`) → CVE detection via the advisory database
- GitHub Actions workflows, TOML/YAML config files → misconfiguration detection
- All tracked files → secret detection

Container image scanning is out of scope for v1 (no images are built in this repo yet).

### D3: Blocking severity — MEDIUM and above

Trivy severity levels: UNKNOWN < LOW < MEDIUM < HIGH < CRITICAL.
Trivy exit codes: 0 = no issues found, 1 = issues found at or above the configured threshold.

The scan is configured with `--exit-code 1 --severity MEDIUM,HIGH,CRITICAL` so that the CI job fails (non-zero exit) when medium or higher vulnerabilities are found. UNKNOWN and LOW are reported but do not fail the build.

> **Note on "WARN/minor":** Trivy does not have a WARN severity level. The closest equivalent to "warn/minor or higher" is MEDIUM. This is the blocking threshold used.

### D4: GitHub Actions — dedicated workflow, required status check

A new workflow `02-Security.yml` runs on every push and PR to `main`. The job must be added as a required status check in the branch protection rule so that a failing scan blocks the merge.

The workflow uses the official `aquasecurity/trivy-action` which caches the vulnerability database between runs.

### D5: Local developer workflow

Trivy is pre-installed in the devcontainer (`.devcontainer/Dockerfile`) via the official Aqua Security apt repository — no setup required for devcontainer users.

Developers not using the devcontainer can install Trivy manually (`brew install trivy`, `apt install trivy`, or a binary from the Trivy releases page).

Either way, run:

```bash
trivy fs --config .trivy.yaml .
```

The output is identical to CI. A failed local scan shows the same findings that would block the PR.

## Configuration files

### `.trivy.yaml`

The canonical source is the `.trivy.yaml` file at the repo root. For available options see the [Trivy config file reference — scan options](https://trivy.dev/docs/latest/references/configuration/config-file/#scan-options).

### `.trivyignore`

Acknowledged/accepted findings are suppressed here with a comment explaining the rationale. Each entry should include the CVE/finding ID and a brief justification.

## GitHub Actions Workflow (`02-Security.yml`)

```yaml
name: 02-Security

on:
  push:
    branches: ["main"]
  pull_request:
    branches: ["main"]

jobs:
  trivy:
    name: Trivy Security Scan
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Trivy
        uses: aquasecurity/trivy-action@master
        with:
          scan-type: fs
          scan-ref: .
          trivy-config: .trivy.yaml
```

## Blocking PR merges

Add `Trivy Security Scan` as a required status check in the `main` branch protection rule (Settings → Branches → Branch protection rules → Require status checks to pass). Once set, any PR where the Trivy job exits non-zero cannot be merged.

## Alternatives Considered

**Separate configs for local and CI**: Rejected — divergence between environments is the primary risk; a single `.trivy.yaml` eliminates it.

**Blocking on LOW and above**: Rejected — Cargo's transitive dependency graph routinely includes low-severity advisories in indirect dependencies; blocking on LOW would create constant noise and impede development before a suppression workflow is established.

**`grype` or `cargo-audit` instead of Trivy**: Rejected — Trivy covers secrets and misconfigurations in addition to CVEs, reducing the number of tools to maintain.

## Out of Scope for v1

- Container image scanning (no images built in this repo)
- SBOM generation
- Dependency licence enforcement
- Automatic issue creation for findings
