---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Add Trivy to devcontainer"
scope: Small
status: Done
---

# WP-01: Add Trivy to devcontainer

## Objective

Install Trivy in the devcontainer so developers can run security scans locally without any manual setup, using the same tool version available in CI.

## Deliverables

- `.devcontainer/Dockerfile` — Trivy installed via the official Aqua Security apt repository (`aquasecurity.github.io/trivy-repo`)

## Acceptance Criteria

- [x] Trivy is installed in the devcontainer image via the official apt repo
- [x] `trivy --version` succeeds inside the container
- [x] Install commands match the official Trivy Debian installation docs

## Dependencies

- None
