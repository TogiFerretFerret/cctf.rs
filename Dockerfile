# syntax=docker/dockerfile:1

########## chef base ##########
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

########## plan — capture the dependency recipe ##########
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

########## build ##########
FROM chef AS builder
# reqwest's rustls pulls aws-lc-rs; building aws-lc-sys needs cmake + a C compiler
# (gcc/make already ship in the rust image). No OpenSSL anywhere.
RUN apt-get update && apt-get install -y --no-install-recommends cmake \
    && rm -rf /var/lib/apt/lists/*
# Cook ONLY the dependencies — this layer is cached until recipe.json changes.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# Real sources now; only our crate recompiles (deps reused from the cooked layer).
COPY . .
RUN cargo build --release

########## docs (built via the Makefile → single self-contained file) ##########
FROM node:22-alpine AS docs
RUN apk add --no-cache make
WORKDIR /build
COPY Makefile ./
COPY apidocs ./apidocs
RUN make build-docs

########## runtime ##########
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# ca-certificates for outbound HTTPS (CTFtime OAuth) via rustls-platform-verifier.
# rclone for the file-storage backend (the app shells out to it for uploads).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates rclone \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cctf-rs /usr/local/bin/cctf-rs
# Fluent's static_loader! + load_bracket_scripts() + the openapi spec resolve
# relative to CWD at runtime, so these must sit next to where we run.
COPY --from=builder /app/locales ./locales
COPY --from=builder /app/openapi.yaml ./openapi.yaml
COPY --from=docs /build/apidocs/dist ./apidocs/dist

EXPOSE 6767
ENV BIND_ADDR=0.0.0.0:6767
CMD ["cctf-rs"]
