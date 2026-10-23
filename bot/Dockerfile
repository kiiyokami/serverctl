# syntax=docker/dockerfile:1
FROM rust:1.88-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
# Cache the crate registry and target/ across builds so only changed code recompiles.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && cp target/release/serverctl-bot /serverctl-bot

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL https://get.helm.sh/helm-v3.16.0-linux-amd64.tar.gz | tar xz -C /tmp \
    && mv /tmp/linux-amd64/helm /usr/local/bin/helm \
    && rm -rf /tmp/linux-amd64
COPY --from=builder /serverctl-bot /usr/local/bin/serverctl-bot
ENTRYPOINT ["/usr/local/bin/serverctl-bot"]
