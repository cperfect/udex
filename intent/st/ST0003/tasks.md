# Tasks - ST0003: Implement Security Scanning of repository

## Tasks

- [x] WP-01: Add Trivy to devcontainer
- [x] WP-02: Add .trivy.yaml config and .trivyignore
- [x] WP-03: Add 02-Security GitHub Actions workflow
- [x] WP-04: Document local usage in repo README / devcontainer docs

## Task Notes

- WP-01: Trivy installed via the official Aqua Security apt repo in `.devcontainer/Dockerfile`
- WP-02: `.trivy.yaml` blocks on MEDIUM/HIGH/CRITICAL; `.trivyignore` provided as an empty baseline with suppression instructions
- WP-03: `02-Security.yml` uses `aquasecurity/trivy-action`; add `Trivy Security Scan` as a required status check in the main branch protection rule to block merging on failure
- WP-04: Security badge and usage section added to root `README.md`; `impl.md` filled in

## Dependencies

None — all WPs are independent and were completed in sequence.
