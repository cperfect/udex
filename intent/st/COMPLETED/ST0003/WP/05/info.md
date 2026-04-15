---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Add weekly scheduled full-codebase Trivy scan"
scope: Small
status: Done
---

# WP-05: Add weekly scheduled full-codebase Trivy scan

## Objective

Schedule an automatic weekly Trivy scan of the main branch so newly published CVEs are caught between PRs, not just at PR time.

## Deliverables

- `.github/workflows/02-Security.yml` — `schedule` trigger added (`0 6 * * 1`, every Monday at 06:00 UTC) and `workflow_dispatch` added for on-demand manual runs

## Acceptance Criteria

- [x] Workflow has a `schedule` trigger firing every Monday at 06:00 UTC
- [x] Workflow has a `workflow_dispatch` trigger for manual runs from the GitHub Actions UI

## Dependencies

- WP-03 (02-Security workflow)
