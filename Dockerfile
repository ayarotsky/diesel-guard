FROM rust:1.96.0 AS builder

ARG TARGETARCH

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools clang libclang-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN case "$TARGETARCH" in \
      amd64) TRIPLE="x86_64-unknown-linux-musl" ;; \
      arm64) TRIPLE="aarch64-unknown-linux-musl" ;; \
      *) echo "Unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && rustup target add "$TRIPLE" \
    && CC=musl-gcc \
       RUSTFLAGS="-C linker=musl-gcc" \
       cargo build --release --target "$TRIPLE" \
    && cp "target/$TRIPLE/release/diesel-guard" /diesel-guard

FROM alpine:3

RUN addgroup -g 1000 diesel && adduser -u 1000 -G diesel -s /bin/sh -D diesel
COPY --from=builder /diesel-guard /usr/local/bin/diesel-guard
RUN chown diesel:diesel /usr/local/bin/diesel-guard
USER diesel

ENTRYPOINT ["/usr/local/bin/diesel-guard"]
