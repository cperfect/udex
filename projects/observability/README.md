# Local Observability Stack

This directory contains the local development observability stack for Udex:
distributed tracing, metrics, and log aggregation built on OpenTelemetry and the
Grafana ecosystem. It is the local backend that receives the OpenTelemetry
signals the Udex server, SDK, and CLI emit (traces, metrics, and logs over OTLP).

The Udex application is coupled only to **open standards** (OTLP). This stack is
one possible backend; any OTel-compatible backend can be substituted by
reconfiguring the Collector, never the application.

## Components

| Component | Image | Role | Local port |
|---|---|---|---|
| OpenTelemetry Collector (contrib) | `otel/opentelemetry-collector-contrib` | Single OTLP ingest for traces/metrics/logs; PostgreSQL metrics via `postgresqlreceiver` | 4317 (OTLP gRPC/TLS), 4318 (OTLP HTTP/TLS), 8889 (Prometheus exporter), 13133 (health) |
| Grafana Tempo | `grafana/tempo` | Trace storage + query | 3200 |
| Prometheus | `prom/prometheus` | Metrics storage + query; scrapes the Collector | 9090 |
| Grafana Loki | `grafana/loki` | Log storage + query (OTLP + Vector) | 3100 |
| Vector | `timberio/vector` | Ships Docker container stdout to Loki | - |
| Grafana | `grafana/grafana` | Dashboards + exploration | 3000 |

## Signal topology

```text
                         +------------------------------------------+
 udex server --OTLP/TLS-->  OTel Collector                          |
 udex sdk/cli (traces,     |  - postgresqlreceiver (DB metrics)     |
   metrics, logs)          |  - exporters -> Tempo / Prom / Loki    |
                           +---+-----------+-----------+------------+
 udex server --JSON stdout--+  | traces    | metrics   | logs
 postgres/hydra stdout -----+->| Vector ---+-----------+
                               v           v           v
                             Loki        Tempo     Prometheus
                               +--------- Grafana ----------+
```

- **Traces**: app/sdk/cli -> OTLP (TLS) -> Collector -> Tempo.
- **Metrics**: app -> OTLP (TLS) -> Collector -> Prometheus (scrapes the
  Collector's exporter on `:8889`). PostgreSQL server metrics come from the
  Collector `postgresqlreceiver`.
- **Logs (hybrid)**: app -> OTLP (TLS) -> Collector -> Loki *when an endpoint is
  configured*; the app *always* also writes JSON to stdout, which Vector (plus
  postgres/hydra container stdout) ships to Loki as the durable floor.

## Prerequisites

- The base dev stack patterns are in place: `.env` (`scripts/gen-env.sh`) and key
  material (`scripts/gen-keys-and-certs.sh`, which now also generates the OTLP
  collector certs under `certs/`).
- Docker + Docker Compose (see `scripts/dev-doctor.sh`).

## Usage

```bash
# Bring the stack up (starts the base postgres + hydra first if needed)
bash projects/observability/scripts/up.sh

# Recreate from scratch
bash projects/observability/scripts/rebuild.sh

# Tear the stack down (leaves the base stack running)
bash projects/observability/scripts/down.sh
```

Then open Grafana at <http://localhost:3000> (user `admin`, password from
`GRAFANA_ADMIN_PASSWORD` in `.env`; anonymous viewer access is also enabled).

### Opt-in by design

This stack is **off by default**. A plain `docker compose up` of the base stack
starts only PostgreSQL and Hydra. The observability services carry a compose
`profile` of `observability` and are only started by `up.sh`. The Udex app, in
turn, only emits OTLP when an `observability.otlp_endpoint` is configured - so
nothing here imposes overhead on a developer who has not opted in.

### How layering works (and why it is its own project)

`up.sh` runs `docker-compose.observability.yml` as its **own** compose project
(`udex-observability`) and attaches it to the **already-running base/devcontainer
network**, which it detects from the running `postgres` container. This means:

- In the devcontainer, the app reaches the collector by service name
  (`otel-collector:4317`); the collector reaches `postgres` the same way.
- On the host, the same services are reachable via the published ports above.

It is deliberately a separate single-file project rather than being merged into
the devcontainer/base compose file set via `-f a -f b`: with multiple `-f` files,
relative bind-mount paths resolve against the *first* file's directory, which
would break the `./collector/config.yaml`-style mounts here. A single-file
project keeps those paths correct, and the external-network attachment still
gives full in-network reachability.

## Configuration layout

```text
projects/observability/
  docker-compose.observability.yml   # the stack (profile: observability)
  collector/config.yaml              # OTLP receivers + postgresqlreceiver + exporters
  tempo/tempo.yaml                   # Tempo single-binary config
  prometheus/prometheus.yml          # scrape config (Collector targets)
  loki/loki-config.yaml              # Loki single-binary config (OTLP enabled)
  vector/vector.yaml                 # container stdout -> Loki
  grafana/provisioning/              # datasources + dashboard provider
  grafana/dashboards/                # provisioned dashboards
  certs/                             # generated OTLP certs (gitignored)
  scripts/                           # up.sh / down.sh / rebuild.sh
```

## TLS

The app -> Collector OTLP hop uses TLS, consistent with the project's
TLS-everywhere principle. `certs/regenerate_certs.sh` (invoked by
`scripts/gen-keys-and-certs.sh`) generates a self-contained OTLP CA and a
collector server cert with SANs for `otel-collector`, `localhost`,
`host.docker.internal`, `127.0.0.1`, and `::1`. The app trusts `certs/ca.crt`
via its `observability.otlp_ca` setting. These certs are gitignored and for local
development only.

## Notes

- Storage is local/ephemeral (no persistent volumes) with short retention - this
  is a development aid, not a production deployment.
- The Grafana "Udex Overview" dashboard is a starter; the app already emits
  request/datastore spans and per-method metrics, so richer dashboards can be
  built on `udex_rpc_*` metrics and the `db.*` / gRPC spans.
