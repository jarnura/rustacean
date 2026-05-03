# syntax=docker/dockerfile:1

# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /app

# Cache dependency downloads before copying source.
COPY Cargo.toml Cargo.lock ./
COPY crates/rb-tracing/Cargo.toml             crates/rb-tracing/Cargo.toml
COPY crates/rb-schemas/Cargo.toml             crates/rb-schemas/Cargo.toml
COPY crates/rb-kafka/Cargo.toml               crates/rb-kafka/Cargo.toml
COPY crates/rb-storage-neo4j/Cargo.toml       crates/rb-storage-neo4j/Cargo.toml
COPY services/projector-neo4j/Cargo.toml      services/projector-neo4j/Cargo.toml

# Stub sources so `cargo build` caches deps without the full source tree.
RUN find crates services -name "Cargo.toml" | sed 's|Cargo.toml|src/lib.rs|' | \
    xargs -I{} sh -c 'mkdir -p "$(dirname {})" && touch "{}"' && \
    mkdir -p services/projector-neo4j/src && \
    printf 'fn main() {}' > services/projector-neo4j/src/main.rs && \
    cargo build --release -p projector-neo4j 2>/dev/null || true && \
    rm -rf crates/*/src services/*/src

# Install build dependencies (rdkafka requires cmake + C toolchain).
RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        cmake libssl-dev libsasl2-dev zlib1g-dev libcurl4-openssl-dev pkg-config \
        gcc g++ && \
    rm -rf /var/lib/apt/lists/*

# Copy real source and build.
COPY crates/                      crates/
COPY services/projector-neo4j/    services/projector-neo4j/
COPY proto/                       proto/

RUN cargo build --release -p projector-neo4j

# ── Stage 2: minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libsasl2-2 zlib1g libcurl4 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 65532 nonroot && \
    useradd --uid 65532 --gid 65532 --no-create-home --system nonroot

USER nonroot

COPY --from=builder /app/target/release/projector-neo4j /usr/local/bin/projector-neo4j

ENTRYPOINT ["/usr/local/bin/projector-neo4j"]
