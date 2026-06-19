---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Docs + security scan"
scope: Small
status: Done
---

# WP-05: Docs + security scan

## Objective

Bring docs in line with the new ingress and clear the Helm security scan for the re-encrypt hop.

## Deliverables

- `projects/k8s/README.md` — update the mermaid diagram and prose (passthrough → terminate+re-encrypt), the chart-structure listing, and the health-probe note if affected.
- Trivy/KSV Helm misconfig scan run; a justified, annotated suppression added if `insecureSkipVerify: true` (or the new Secret) is flagged, consistent with the existing dormant-KSV-suppression approach.

## Acceptance Criteria

- [x] README diagram + prose describe terminate + re-encrypt; no stale passthrough references (repo-wide sweep clean)
- [x] Chart-structure listing matches the new templates
- [x] `docs/SECRETS.md` lists the Traefik edge cert material + new edge `regenerate_certs.sh`
- [x] `.trivy.yaml` renders the chart with the new required secrets (all 7 templates scanned — no silent skip)
- [x] Helm Trivy/KSV scan green at the CI gate: exit 0 at MEDIUM+, only pre-existing LOW findings, `insecureSkipVerify` not flagged

## Dependencies

- WP02 (chart templates must exist to scan/document).
