FROM rust:1.90-alpine AS builder
RUN apk add --no-cache musl-dev
RUN mkdir /build
COPY . /build
WORKDIR /build
RUN cargo build --release

FROM scratch
COPY --from=builder /build/target/release/dimpact /bin/dimpact
WORKDIR /work

ENTRYPOINT ["/bin/dimpact"]
