# syntax=docker/dockerfile:1

########## builder ##########
FROM rust:1-bookworm AS builder
WORKDIR /app

# reqwest defaults to native-tls (OpenSSL), so the build needs it.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 1) Cache the dependency graph: compile a stub crate with only the manifests.
#    This layer only busts when Cargo.toml / Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

# 2) Build the real binary — dependencies stay cached from step 1.
COPY src ./src
COPY locales ./locales
RUN cargo build --release --locked

########## runtime ##########
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# ca-certificates: outbound HTTPS (CTFtime OAuth). libssl3: native-tls runtime.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cctf-rs /usr/local/bin/cctf-rs
# Fluent's static_loader! and load_bracket_scripts() resolve paths relative to
# the process CWD at runtime, so ./locales must sit next to where we run.
COPY --from=builder /app/locales ./locales

EXPOSE 8080
ENV BIND_ADDR=0.0.0.0:8080
CMD ["cctf-rs"]
