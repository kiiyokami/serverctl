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
    && curl -fsSLo /tmp/helm.tgz https://get.helm.sh/helm-v3.16.0-linux-amd64.tar.gz \
    && echo "327cfbc7ddc5a3eb644039ceb0cff66394628654c4f5a76bf715ed15b893983b  /tmp/helm.tgz" | sha256sum -c - \
    && tar xzf /tmp/helm.tgz -C /tmp \
    && mv /tmp/linux-amd64/helm /usr/local/bin/helm \
    && rm -rf /tmp/linux-amd64 /tmp/helm.tgz
COPY --from=builder /serverctl-bot /usr/local/bin/serverctl-bot
ENTRYPOINT ["/usr/local/bin/serverctl-bot"]
