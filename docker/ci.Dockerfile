FROM rust:1.90-alpine
RUN rustup component add clippy
RUN apk add --no-cache musl-dev

