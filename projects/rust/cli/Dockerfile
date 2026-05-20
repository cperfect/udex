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
#   4. runtime  — copies the stripped binary into a minimal debian image

# ── Chef installer ────────────────────────────────────────────────────────────
FROM rust:1.95.0-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /build

# ── Planner: produce recipe.json from Cargo manifests ────────────────────────
# The planner only reads Cargo.toml / Cargo.lock — it does not compile
# anything and does not need protoc or the proto files.
FROM chef AS planner
COPY rust/ rust/
RUN cd rust && cargo chef prepare --recipe-path recipe.json

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
RUN cd rust && cargo chef cook --profile "${PROFILE}" --recipe-path recipe.json

# Copy real source and compile only the workspace crates. All dependency
# objects from the cook step are reused; only udex-* crates are compiled here.
COPY rust/ rust/
RUN cd rust && cargo build --profile "${PROFILE}" -p udex-cli \
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
