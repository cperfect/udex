# Implementation - ST0025: Cluster integration tests

## Implementation

### WP01 — Chart: default dev to 2 replicas (as built)

- `values.yaml`: added `replicaCount: 2` with a comment explaining the dev default (always exercise the multi-instance stateless invariant).
- `templates/deployment.yaml`: `replicas: {{ .Values.replicaCount }}`; header comment updated from "one replica" to ".Values.replicaCount copies (default 2)".

Verified (helm v4.0.0): `validate-lint-helm.sh` passes; `helm template --show-only templates/deployment.yaml` renders `replicas: 2` by default and `replicas: 3` with `--set replicaCount=3`.

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
