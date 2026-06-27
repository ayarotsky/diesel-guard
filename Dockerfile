FROM rust:1.96.0 AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETARCH

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools clang libclang-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json

# Cook dependencies in their own layer. As long as recipe.json is unchanged
# (i.e. no dependency changes), this layer is reused and only application code
# is recompiled below.
RUN case "$TARGETARCH" in \
      amd64) TRIPLE="x86_64-unknown-linux-musl" ;; \
      arm64) TRIPLE="aarch64-unknown-linux-musl" ;; \
      *) echo "Unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && echo "$TRIPLE" > /triple \
    && rustup target add "$TRIPLE" \
    && CC=musl-gcc \
       RUSTFLAGS="-C linker=musl-gcc" \
       cargo chef cook --release --target "$TRIPLE" --recipe-path recipe.json

COPY . .

RUN TRIPLE="$(cat /triple)" \
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
