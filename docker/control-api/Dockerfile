# syntax=docker/dockerfile:1

# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /app

# Cache dependency downloads before copying source.
COPY Cargo.toml Cargo.lock ./
COPY crates/rb-tracing/Cargo.toml      crates/rb-tracing/Cargo.toml
COPY crates/rb-schemas/Cargo.toml      crates/rb-schemas/Cargo.toml
COPY crates/rb-secrets/Cargo.toml      crates/rb-secrets/Cargo.toml
COPY services/migrate/Cargo.toml       services/migrate/Cargo.toml
COPY services/control-api/Cargo.toml   services/control-api/Cargo.toml

# Stub sources so `cargo build` caches deps without the full source tree.
RUN find crates services -name "Cargo.toml" | sed 's|Cargo.toml|src/lib.rs|' | \
    xargs -I{} sh -c 'mkdir -p "$(dirname {})" && touch "{}"' && \
    mkdir -p services/control-api/src && \
    printf 'fn main() {}' > services/control-api/src/main.rs && \
    cargo build --release -p control-api 2>/dev/null || true && \
    # Remove stub artifacts so real source replaces them.
    rm -rf crates/*/src services/*/src

# Copy real source and build.
COPY crates/  crates/
COPY services/control-api/  services/control-api/
COPY services/migrate/      services/migrate/
COPY proto/                 proto/

# Install rdkafka build deps (needed because migrate crate uses rdkafka).
RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        cmake libssl-dev libsasl2-dev zlib1g-dev libcurl4-openssl-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

RUN cargo build --release -p control-api

# ── Stage 2: minimal runtime image ────────────────────────────────────────────
# debian:bookworm-slim matches the builder's glibc 2.36 and supplies libz + sasl2
# required by rdkafka at runtime (distroless/cc-debian12 lacks libz.so.1).
FROM debian:bookworm-slim AS runtime

RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libsasl2-2 zlib1g libcurl4 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 65532 nonroot && \
    useradd --uid 65532 --gid 65532 --no-create-home --system nonroot

USER nonroot

COPY --from=builder /app/target/release/control-api /usr/local/bin/control-api

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/control-api"]
CMD ["serve"]
