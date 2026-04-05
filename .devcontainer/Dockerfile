# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.245.2/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="buster"
ARG RUST_VERISON="stable"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-1-${VARIANT}

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
  && apt-get -y install --no-install-recommends \
  protobuf-compiler libprotobuf-dev

# install intent
# Clone the repository
RUN mkdir -p /opt/intent
WORKDIR /opt/intent
RUN git clone https://github.com/matthewsinclair/intent.git .
RUN ln -s /opt/intent/bin/intent /usr/local/bin/intent

