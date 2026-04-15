---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Add .trivy.yaml config and .trivyignore"
scope: Small
status: Done
---

# WP-02: Add .trivy.yaml config and .trivyignore

## Objective

Create the shared Trivy configuration file and an acknowledgement file for accepted findings, so local and CI scans use identical settings.

## Deliverables

- `.trivy.yaml` — shared scan config: vuln, secret, misconfig, and license scanners; blocks on MEDIUM/HIGH/CRITICAL (exit-code 1)
- `.trivyignore` — baseline suppression file with instructions for adding accepted findings with rationale

## Acceptance Criteria

- [x] `.trivy.yaml` present at repo root with all four scanners configured
- [x] `exit-code: 1` set so scans fail on MEDIUM+ findings
- [x] `.trivyignore` present with documented suppression format
- [x] `trivy fs --config .trivy.yaml .` runs successfully locally

## Dependencies

- WP-01 (Trivy installed in devcontainer)
