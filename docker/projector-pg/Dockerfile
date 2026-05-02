# syntax=docker/dockerfile:1

# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /app

# Cache dependency downloads before copying source.
COPY Cargo.toml Cargo.lock ./
COPY crates/rb-tracing/Cargo.toml      crates/rb-tracing/Cargo.toml
COPY crates/rb-schemas/Cargo.toml      crates/rb-schemas/Cargo.toml
COPY crates/rb-kafka/Cargo.toml        crates/rb-kafka/Cargo.toml
COPY crates/rb-tenant/Cargo.toml       crates/rb-tenant/Cargo.toml
COPY crates/rb-storage-pg/Cargo.toml   crates/rb-storage-pg/Cargo.toml
COPY services/projector-pg/Cargo.toml  services/projector-pg/Cargo.toml
COPY services/tombstoner/Cargo.toml    services/tombstoner/Cargo.toml

# Stub sources so `cargo build` caches deps without the full source tree.
RUN find crates services -name "Cargo.toml" | sed 's|Cargo.toml|src/lib.rs|' | \
    xargs -I{} sh -c 'mkdir -p "$(dirname {})" && touch "{}"' && \
    mkdir -p services/projector-pg/src && \
    printf 'fn main() {}' > services/projector-pg/src/main.rs && \
    cargo build --release -p projector-pg 2>/dev/null || true && \
    # Remove stub artifacts so real source replaces them.
    rm -rf crates/*/src services/*/src

# Copy real source and build.
COPY crates/                 crates/
COPY services/projector-pg/  services/projector-pg/
COPY proto/                  proto/

# Install rdkafka build deps.
RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        cmake libssl-dev libsasl2-dev zlib1g-dev libcurl4-openssl-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

RUN cargo build --release -p projector-pg

# ── Stage 2: minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libsasl2-2 zlib1g libcurl4 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 65532 nonroot && \
    useradd --uid 65532 --gid 65532 --no-create-home --system nonroot

USER nonroot

COPY --from=builder /app/target/release/projector-pg /usr/local/bin/projector-pg

ENTRYPOINT ["/usr/local/bin/projector-pg"]
