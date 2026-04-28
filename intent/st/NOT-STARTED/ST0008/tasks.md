---
verblock: "28 Apr 2026:v0.1: vscode - Initial task breakdown"
---

# ST0008: Tasks — Inject keys and secrets

## Work Packages

- [ ] WP-01: Gitignore & remove committed secrets
- [ ] WP-02: Developer setup scripts (`gen-env.sh`, `gen-keys-and-certs.sh`)
- [ ] WP-03: Devcontainer post-create integration
- [ ] WP-04: Config crate evaluation and `_secret` naming convention
- [ ] WP-05: File-injection guard in config loader
- [ ] WP-06: Remove secrets from CLI arguments
- [ ] WP-07: Inject secrets into Compose and CI via env vars
- [ ] WP-08: Update CONTRIBUTING.md, SECRETS.md, SECURITY.md

## Sequencing

```
WP-01  (remove secrets from repo)
  └─► WP-02  (gen scripts replace what was removed)
        └─► WP-03  (devcontainer runs gen scripts)

WP-04  (decide config approach + naming convention)
  └─► WP-05  (implement file-injection guard)
        └─► WP-06  (remove secrets from CLI args)

WP-07  (compose/CI — can run in parallel with WP-04..06)

WP-08  (docs — after all implementation WPs complete)
```

WP-01 through WP-03 are the highest-priority sequence: they get secrets out of
the repo immediately. WP-04 through WP-06 can follow in the same or next
session. WP-07 is independent. WP-08 is always last.

## Dependencies

- WP-02 depends on WP-01 (need to know what was removed to generate replacements)
- WP-05 depends on WP-04 (naming convention must be decided before guard can be coded)
- WP-06 depends on WP-05 (CLI uses the same config loading path)
- WP-08 depends on all other WPs
