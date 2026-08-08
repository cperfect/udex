# Build context: projects/
# Example: docker build -f projects/rust/cli/Dockerfile projects/ -t udex:latest
#          docker build -f projects/rust/cli/Dockerfile projects/ -t udex:dev --build-arg PROFILE=dev
#
# Layer caching strategy (cargo-chef):
#   1. planner  — parses Cargo manifests, emits recipe.json (no compilation)
#   2. builder  — cooks (compiles) all dependencies from recipe.json; this
#                 layer is fully cached when only source files change and
#                 Cargo.toml / Cargo.lock are unchanged
#   3. builder  — copies real source, compiles workspace crates only (~1-2 min)
#   4. runtime  — copies the stripped binary into a distroless image

# ── Chef installer ────────────────────────────────────────────────────────────
FROM rust:1.95.0-bookworm AS chef
RUN cargo install cargo-chef --locked --version 0.1.77
WORKDIR /build

# ── Planner: produce recipe.json from Cargo manifests ────────────────────────
# The planner only reads Cargo.toml / Cargo.lock — it does not compile
# anything and does not need protoc or the proto files.
FROM chef AS planner
COPY rust/ rust/
WORKDIR /build/rust
RUN cargo chef prepare --recipe-path recipe.json

# ── Builder ───────────────────────────────────────────────────────────────────
FROM chef AS builder

ARG PROFILE=release

# Install protoc 34.1 (matches devcontainer version).
# Detects host arch to select the correct release asset.
RUN apt-get update && apt-get install -y --no-install-recommends unzip curl ca-certificates \
    && ARCH=$(uname -m) \
    && case "$ARCH" in \
         x86_64)  PROTOC_ZIP="protoc-34.1-linux-x86_64.zip" ;; \
         aarch64) PROTOC_ZIP="protoc-34.1-linux-aarch_64.zip" ;; \
         *) echo "Unsupported arch: $ARCH" && exit 1 ;; \
       esac \
    && curl -fsSL "https://github.com/protocolbuffers/protobuf/releases/download/v34.1/${PROTOC_ZIP}" \
         -o /tmp/protoc.zip \
    && unzip -q /tmp/protoc.zip -d /usr/local \
    && rm /tmp/protoc.zip \
    && protoc --version \
    && apt-get purge -y unzip curl && apt-get autoremove -y && rm -rf /var/lib/apt/lists/*

# Proto files are required by api/build.rs (tonic-build) when the build script
# runs during dependency cooking. Must be present before `cargo chef cook`.
COPY protobuf/ protobuf/

# Cook (compile) all dependencies. Cached as long as recipe.json —
# and therefore Cargo.toml / Cargo.lock — does not change.
COPY --from=planner /build/rust/recipe.json rust/recipe.json
WORKDIR /build/rust
RUN cargo chef cook --profile "${PROFILE}" --recipe-path recipe.json

# Copy real source and compile only the workspace crates. All dependency
# objects from the cook step are reused; only udex-* crates are compiled here.
WORKDIR /build
COPY rust/ rust/
WORKDIR /build/rust
RUN cargo build --profile "${PROFILE}" -p udex-cli \
    # Map profile name to output directory: Cargo writes "dev" builds to
    # target/debug/, not target/dev/.
    && dir=$([ "${PROFILE}" = "dev" ] && echo "debug" || echo "${PROFILE}") \
    && cp "target/${dir}/udex" /build/udex

# ── Runtime ───────────────────────────────────────────────────────────────────
# Distroless: the OS package surface is the whole attack surface of a runtime
# image, and debian:bookworm-slim carried 163 CVEs (5 CRITICAL, 17 HIGH), none of
# which had an upstream fix — so the base image was the only lever that could
# move them. This variant reports 17, with zero CRITICAL or HIGH, and is 90MB
# smaller.
#
# `cc` specifically, not the smaller `base`: the binary is dynamically linked
# against glibc and needs libgcc_s.so.1 (verified with ldd — libc, libm,
# libgcc_s, ld-linux; rustls means no OpenSSL dependency). On `base` it fails at
# startup with "libgcc_s.so.1: cannot open shared object file". `static` would
# reach zero CVEs but requires a musl static build, which aws-lc-rs (C code, via
# rustls) makes considerably more than a base-image swap.
#
# No ca-certificates install: distroless already ships
# /etc/ssl/certs/ca-certificates.crt, which is what the outbound TLS paths (JWKS
# fetch, OAuth2 token endpoint) need.
#
# Pinned by DIGEST, not tag — distroless tags are rebuilt in place, so a bare tag
# silently changes the base between builds. This is the multi-arch manifest list
# (amd64 for CI, arm64 for Apple-silicon devcontainers). To update: pick the new
# digest from `docker buildx imagetools inspect gcr.io/distroless/cc-debian12`.
#
# Trade-off: no shell, so `kubectl exec -- sh` will not work against a running
# pod. The liveness/readiness probes are tcpSocket rather than exec so they are
# unaffected; for interactive debugging use the `:debug` variant of the same
# image, which adds busybox.
FROM gcr.io/distroless/cc-debian12@sha256:e8e7ee4b8b106d4c5fde9e422a321b2b8a2d5cca546c97adcce927f3e1d36e36

COPY --from=builder /build/udex /usr/local/bin/udex

EXPOSE 443

ENTRYPOINT ["udex", "serve", "--config", "/etc/udex/config.yaml"]
