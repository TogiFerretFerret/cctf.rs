# syntax=docker/dockerfile:1

########## builder ##########
FROM rust:1-bookworm AS builder
WORKDIR /app

# reqwest's rustls uses aws-lc-rs; building aws-lc-sys needs cmake + a C
# compiler (gcc/make already ship in the rust image). No OpenSSL anywhere.
RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake \
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

########## docs (from-scratch TS OpenAPI viewer → single self-contained file) ##########
FROM node:22-alpine AS docs
WORKDIR /docs
COPY apidocs/package*.json ./
RUN npm install
COPY apidocs/ ./
RUN npm run build

########## runtime ##########
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# reqwest verifies outbound HTTPS (CTFtime OAuth) against the system trust store
# via rustls-platform-verifier, so ca-certificates is required. No OpenSSL.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cctf-rs /usr/local/bin/cctf-rs
# Fluent's static_loader! and load_bracket_scripts() resolve paths relative to
# the process CWD at runtime, so ./locales must sit next to where we run.
COPY --from=builder /app/locales ./locales
# Built docs viewer (single self-contained index.html), served at /docs.
COPY --from=docs /docs/dist ./apidocs/dist

EXPOSE 8080
ENV BIND_ADDR=0.0.0.0:8080
CMD ["cctf-rs"]
