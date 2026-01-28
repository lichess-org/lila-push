# syntax=docker/dockerfile:1

FROM rust:1-trixie AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release
COPY . .
RUN cargo build --release

FROM debian:trixie-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*
RUN groupadd -g 1001 lichess && \
    useradd -u 1001 -g lichess -m -d /home/lichess -s /bin/bash lichess
COPY --from=builder --chown=lichess:lichess /app/target/release/lila-push /usr/local/bin/lila-push
USER lichess
ENTRYPOINT ["/usr/local/bin/lila-push"]
