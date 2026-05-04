ARG BASE_IMAGE=ffhn-devcontainer:local
FROM ${BASE_IMAGE}

USER root

RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install --yes --no-install-recommends docker.io nodejs npm \
    && npm install --global @devcontainers/cli@0.86.0 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

USER root
