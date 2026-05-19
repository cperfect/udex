# Build context: projects/
# Example: docker build -f projects/rust/cli/Dockerfile projects/ -t udex:latest
#          docker build -f projects/rust/cli/Dockerfile projects/ -t udex:dev --build-arg PROFILE=dev

# ── Builder ───────────────────────────────────────────────────────────────────
FROM rust:1.95.0-bookworm AS builder

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

WORKDIR /build

# Copy proto definitions (required by api/build.rs, which resolves them as
# ../../protobuf relative to the workspace root).
COPY protobuf/ protobuf/

# Copy the full Rust workspace.
COPY rust/ rust/

WORKDIR /build/rust

RUN cargo build --profile "${PROFILE}" -p udex-cli \
    # Map profile name to output directory: Cargo writes "dev" builds to
    # target/debug/, not target/dev/.
    && dir=$([ "${PROFILE}" = "dev" ] && echo "debug" || echo "${PROFILE}") \
    && cp "target/${dir}/udex" /build/udex

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/udex /usr/local/bin/udex

EXPOSE 443

ENTRYPOINT ["udex", "serve", "--config", "/etc/udex/config.toml"]
