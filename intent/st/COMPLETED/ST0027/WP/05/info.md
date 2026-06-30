---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Agnostic otlp_headers config in udex-telemetry"
scope: Small
status: Done
---

# WP-05: Agnostic otlp_headers config in udex-telemetry

## Objective

Add optional, generic OTLP header support to `udex-telemetry` (and the server/CLI config surface) so users can point the app at their own header-authed OTel backends (Honeycomb, Grafana Cloud, ClickStack all-in-one). This keeps the application solution-agnostic; our own fixture does not use it.

## Deliverables

- An `otlp_headers` map option in the telemetry config (e.g. `observability.otlp_headers` and a CLI/env equivalent), passed to the OTLP exporters' metadata.
- Validation + redaction so header values (often secrets) never leak into logs.
- Docs: the "plug your own OTel backend" path, including the ClickStack all-in-one example (`Authorization: <raw-key>`, no `Bearer` prefix — per the spike).

## Acceptance Criteria

- [x] Configured headers are sent on OTLP export. (Mechanism unit-tested: `build_metadata_carries_headers` proves the header reaches the exporter `MetadataMap`, attached to all 3 OTLP exporters. Full live check vs the all-in-one is the documented manual path; the spike already proved its raw-`authorization` acceptance.)
- [x] Header values are not emitted in logs. (Manual `Debug` redacts values; `validate()` errors never echo the value — both unit-tested.)
- [x] Default/empty config behaves exactly as today (no header) — our fixture is unaffected. (Empty map -> empty `MetadataMap`; verified.)

## Dependencies

- Independent of the fixture WPs; can land any time. Logically grouped here as the agnostic-backend escape hatch.
