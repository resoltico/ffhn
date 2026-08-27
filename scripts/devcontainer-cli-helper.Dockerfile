ARG NODE_RUNTIME_IMAGE=docker.io/library/node:24-bookworm-slim@sha256:a9f5f7c91a432850b2a8a7797adf5eadb6c733ceed61167806cee7ea7fbc29df
ARG BUILDX_PLUGIN_IMAGE=docker.io/docker/buildx-bin:latest@sha256:1f2f6b2be4a2511ada67336e76892f1a588c89746009dd4b21069e4d867465be
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
    && npm install --global @devcontainers/cli@0.88.0 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

USER root
