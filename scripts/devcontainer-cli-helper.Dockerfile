ARG NODE_RUNTIME_IMAGE=docker.io/library/node:20-bookworm-slim@sha256:2cf067cfed83d5ea958367df9f966191a942351a2df77d6f0193e162b5febfc0
ARG BUILDX_PLUGIN_IMAGE=docker.io/docker/buildx-bin:latest@sha256:1023c4a1ac77cee49520b620e1fa29d78be4ee3660bffd1a23b8a35f4e9ca417
ARG BASE_IMAGE=ffhn-devcontainer:local
FROM ${NODE_RUNTIME_IMAGE} AS node_runtime
FROM ${BUILDX_PLUGIN_IMAGE} AS buildx_plugin

FROM ${BASE_IMAGE}

USER root

COPY --from=node_runtime /usr/local/bin/node /usr/local/bin/node
COPY --from=node_runtime /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=buildx_plugin /buildx /usr/libexec/docker/cli-plugins/docker-buildx

RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install --yes --no-install-recommends docker.io \
    && ln -sf ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -sf ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
    && chmod 0755 /usr/libexec/docker/cli-plugins/docker-buildx \
    && docker buildx version >/dev/null \
    && npm install --global @devcontainers/cli@0.86.0 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

USER root
