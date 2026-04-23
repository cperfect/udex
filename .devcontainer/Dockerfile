# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="bookworm"
ARG RUST_VERSION="1.95.0"
FROM mcr.microsoft.com/devcontainers/rust:${VARIANT}

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
  && apt-get -y install --no-install-recommends \
  protobuf-compiler libprotobuf-dev \
  wget apt-transport-https gnupg lsb-release

## Install Trivy (security scanner)
# This installs the latest Trivy package from the official repo (may differ from the version used in CI).
# Commands match the official Trivy Debian install docs: https://trivy.dev/docs/latest/getting-started/installation/
RUN wget -qO - https://aquasecurity.github.io/trivy-repo/deb/public.key | gpg --dearmor | tee /usr/share/keyrings/trivy.gpg > /dev/null \
  && echo "deb [signed-by=/usr/share/keyrings/trivy.gpg] https://aquasecurity.github.io/trivy-repo/deb generic main" \
     | tee -a /etc/apt/sources.list.d/trivy.list \
  && apt-get update \
  && apt-get -y install --no-install-recommends trivy

# install intent
# Clone the repository
RUN mkdir -p /opt/intent
WORKDIR /opt/intent
RUN git clone https://github.com/matthewsinclair/intent.git .
RUN ln -s /opt/intent/bin/intent /usr/local/bin/intent

