---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Docs + security scan"
scope: Small
status: Not Started
---

# WP-05: Docs + security scan

## Objective

Bring docs in line with the new ingress and clear the Helm security scan for the re-encrypt hop.

## Deliverables

- `projects/k8s/README.md` — update the mermaid diagram and prose (passthrough → terminate+re-encrypt), the chart-structure listing, and the health-probe note if affected.
- Trivy/KSV Helm misconfig scan run; a justified, annotated suppression added if `insecureSkipVerify: true` (or the new Secret) is flagged, consistent with the existing dormant-KSV-suppression approach.

## Acceptance Criteria

- [ ] README diagram + prose describe terminate + re-encrypt; no stale passthrough references
- [ ] Chart-structure listing matches the new templates
- [ ] Helm Trivy/KSV scan is green (clean or with annotated, justified suppressions)

## Dependencies

- WP02 (chart templates must exist to scan/document).
