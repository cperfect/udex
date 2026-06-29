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
`GRAFANA_ADMIN_PASSWORD` in `.env`; anonymous **Editor** access is also enabled,
so Explore works without logging in). See [Viewing telemetry](#viewing-telemetry).

> **Host binding.** The host-facing ports (Grafana, Prometheus, Loki, Tempo, and
> the collector debug/health endpoints) publish on all interfaces by default
> (`OBS_HOST_BIND=0.0.0.0`). That default is required under OrbStack /
> docker-outside-of-docker, which only forwards all-interface ports to the host's
> localhost (and already restricts them to the host loopback, so they are not
> LAN-reachable there). If you run Docker **directly on a Linux host**, set
> `OBS_HOST_BIND=127.0.0.1` (in `.env` or your shell) so these endpoints — Grafana
> has anonymous Editor access — stay off the LAN. The OTLP ingest ports
> (`4317`/`4318`) always publish on all interfaces, because k3d pods reach them
> via `host.k3d.internal`, a non-loopback host IP.

### Default-on in the devcontainer; opt-in otherwise

In the **devcontainer** this stack starts automatically: `post-create.sh` runs
`up.sh` after first-time setup, and the components carry `restart: unless-stopped`
so they persist across restarts. Grafana is available at <http://localhost:3000>
without any extra step.

Outside the devcontainer it is **opt-in**: a plain `docker compose up` of the base
stack starts only PostgreSQL and Hydra; the observability services carry a compose
`profile` of `observability` and are started only by `up.sh`. Either way, the Udex
**application** still only emits OTLP when an `observability.otlp_endpoint` is
configured (server config) or `UDEX_OTLP_ENDPOINT` is set (CLI) - starting the
backend components imposes no telemetry overhead on app/test runs that have not
opted in. Tear the stack down anytime with `down.sh`.

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

## Viewing telemetry

All three signals are explored through **Grafana** at <http://localhost:3000>.
Anonymous **Editor** access is enabled, so no login is needed (the admin login is
`admin` / `GRAFANA_ADMIN_PASSWORD` from `.env`). The Tempo, Prometheus, and Loki
datasources are provisioned automatically.

> Telemetry only exists for requests made **while telemetry was active**. The k3d
> deployment is observability-enabled, so traffic to it produces signals tagged
> `deployment.environment=k3d`; for a local CLI run, set `UDEX_OTLP_ENDPOINT`
> (and `UDEX_OTLP_CA`). Generate some traffic first, e.g.
> `cargo test -p udex-sdk --test integration_tests test_obs_k8s`, then search a
> recent time range (local retention is short).

In Grafana, open **Explore** (left nav) and pick the datasource:

### Traces (Tempo)

Explore -> **Tempo** -> **Search** (Service Name `udex-server`) or the **TraceQL**
tab:

```text
{ resource.service.name = "udex-server" }     # all server traces
{ resource.deployment.environment = "k3d" }   # only the k3d deployment
{ span.index = "<index-name>" }                # by datastore span attribute
{ span.key = "<entry-uuid>" }                  # a specific entry operation
```

Open a trace to see the gRPC request span with the nested `db.*` datastore spans
(and, from an OTel-enabled client, the `sdk.*` client span as the root).

Direct API:

```bash
curl -s -G http://localhost:3200/api/search \
  --data-urlencode 'q={ resource.service.name = "udex-server" }' | jq '.traces[].traceID'
curl -s http://localhost:3200/api/traces/<traceID> | jq -r '.batches[].scopeSpans[].spans[].name'
```

### Metrics (Prometheus)

Explore -> **Prometheus**, or the Prometheus UI at <http://localhost:9090>.
Example queries:

```text
udex_rpc_requests_total                                                            # gRPC requests, by method + status
rate(udex_rpc_requests_total[1m])                                                  # request rate
histogram_quantile(0.95, sum by (le) (rate(udex_rpc_duration_seconds_bucket[5m]))) # p95 request latency
postgresql_backends                                                                # PostgreSQL receiver metrics
```

Direct API:

```bash
curl -s -G http://localhost:9090/api/v1/query \
  --data-urlencode 'query=udex_rpc_requests_total' | jq '.data.result'
```

### Logs (Loki)

Explore -> **Loki**. Example LogQL:

```text
{service_name="udex-server"}                   # OTLP logs from the server
{service_name="udex-server"} |= "error"        # filter by content
{container="udex_devcontainer-postgres-1"}     # postgres container logs (via Vector)
```

Direct API:

```bash
curl -s -G http://localhost:3100/loki/api/v1/query_range \
  --data-urlencode 'query={service_name="udex-server"}' | jq '.data.result'
```

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
`host.docker.internal`, `host.k3d.internal`, `127.0.0.1`, and `::1`. The app trusts `certs/ca.crt`
via its `observability.otlp_ca` setting. These certs are gitignored and for local
development only.

## Notes

- Storage is local/ephemeral (no persistent volumes) with short retention - this
  is a development aid, not a production deployment.
- The Grafana "Udex Overview" dashboard is a starter; the app already emits
  request/datastore spans and per-method metrics, so richer dashboards can be
  built on `udex_rpc_*` metrics and the `db.*` / gRPC spans.
