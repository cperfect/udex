---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Add 02-Security GitHub Actions workflow"
scope: Small
status: Done
---

# WP-03: Add 02-Security GitHub Actions workflow

## Objective

Add a GitHub Actions workflow that runs the Trivy scan on every push and PR to main, failing the build when MEDIUM or higher findings are detected, so insecure changes cannot be merged.

## Deliverables

- `.github/workflows/02-Security.yml` — workflow using `aquasecurity/trivy-action@v0.35.0`, triggered on push, PR, weekly schedule, and manual dispatch; `exit-code: 1` and `severity: MEDIUM,HIGH,CRITICAL` set on the action step

## Acceptance Criteria

- [x] Workflow runs on push and PR to main
- [x] Workflow runs on manual dispatch (`workflow_dispatch`)
- [x] Workflow runs on a weekly Monday schedule
- [x] Action fails (exit code 1) when MEDIUM+ findings are present
- [x] `Trivy Security Scan` status check available to add to branch protection rule

## Dependencies

- WP-02 (`.trivy.yaml` config)
