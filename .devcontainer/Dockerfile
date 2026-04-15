# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="buster"
ARG RUST_VERISON="stable"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-1-${VARIANT}

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
  && apt-get -y install --no-install-recommends \
  protobuf-compiler libprotobuf-dev \
  wget apt-transport-https gnupg lsb-release

# Install Trivy (security scanner) — same version used in CI
RUN wget -qO - https://aquasecurity.github.io/trivy-repo/deb/public.key | gpg --dearmor -o /usr/share/keyrings/trivy.gpg \
  && echo "deb [signed-by=/usr/share/keyrings/trivy.gpg] https://aquasecurity.github.io/trivy-repo/deb generic main" \
     > /etc/apt/sources.list.d/trivy.list \
  && apt-get update \
  && apt-get -y install --no-install-recommends trivy

# install intent
# Clone the repository
RUN mkdir -p /opt/intent
WORKDIR /opt/intent
RUN git clone https://github.com/matthewsinclair/intent.git .
RUN ln -s /opt/intent/bin/intent /usr/local/bin/intent

