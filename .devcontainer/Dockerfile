FROM ubuntu:26.04@sha256:5e275723f82c67e387ba9e3c24baa0abdcb268917f276a0561c97bef9450d0b4

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

USER root

RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        curl \
        file \
        gh \
        git \
        jq \
        less \
        libssl-dev \
        lsof \
        musl-tools \
        pkg-config \
        procps \
        python3 \
        shellcheck \
        sudo \
        unzip \
        zip \
    && if getent passwd 1000 >/dev/null; then \
        existing_user="$(getent passwd 1000 | cut -d: -f1)"; \
        existing_group="$(id -gn "${existing_user}")"; \
        if [[ "${existing_group}" != "vscode" ]]; then \
            groupmod --new-name vscode "${existing_group}"; \
        fi; \
        if [[ "${existing_user}" != "vscode" ]]; then \
            usermod --login vscode --home /home/vscode --move-home --shell /bin/bash "${existing_user}"; \
        fi; \
    else \
        groupadd --gid 1000 vscode; \
        useradd --create-home --shell /bin/bash --uid 1000 --gid 1000 vscode; \
    fi \
    && echo "vscode ALL=(root) NOPASSWD:ALL" > /etc/sudoers.d/50-vscode \
    && chmod 0440 /etc/sudoers.d/50-vscode \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:${PATH}

RUN export CARGO_HOME=/usr/local/cargo \
    && curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf \
        | sh -s -- -y --profile minimal --default-toolchain 1.95.0 \
    && rustup toolchain install nightly --profile minimal --component llvm-tools-preview \
    && rustup component add clippy rustfmt llvm-tools-preview --toolchain 1.95.0 \
    && rustup target add x86_64-unknown-linux-musl --toolchain 1.95.0 \
    && cargo install cargo-nextest cargo-audit cargo-deny cargo-semver-checks cargo-outdated cargo-llvm-cov cargo-fuzz --locked \
    && rustc --version >/dev/null \
    && cargo --version >/dev/null \
    && cargo nextest --version >/dev/null \
    && cargo audit --version >/dev/null \
    && cargo deny --version >/dev/null \
    && cargo semver-checks --version >/dev/null \
    && cargo outdated --version >/dev/null \
    && cargo llvm-cov --version >/dev/null \
    && cargo fuzz --version >/dev/null \
    && shellcheck --version >/dev/null \
    && gh --version >/dev/null \
    && clang --version >/dev/null

ENV CARGO_HOME=/home/vscode/.cargo
ENV HOME=/home/vscode

RUN install -d -m 0755 -o vscode -g vscode \
        /home/vscode/.cargo \
        /home/vscode/.cargo/git \
        /home/vscode/.cargo/registry \
        /home/vscode/.cache \
        /workspaces

WORKDIR /workspaces/ffhn

USER vscode
