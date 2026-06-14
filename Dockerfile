###################
# chef
###################
FROM lukemathwalker/cargo-chef:latest-rust-1.95 AS chef
WORKDIR /app

###################
# planner
###################
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

###################
# builder
###################
FROM chef AS builder

RUN apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*

RUN case "$(uname -m)" in \
      x86_64)  echo x86_64-unknown-linux-musl ;; \
      aarch64) echo aarch64-unknown-linux-musl ;; \
      *) echo "Unsupported architecture: $(uname -m)" >&2 && exit 1 ;; \
    esac > /rust_target.txt && \
    rustup target add "$(cat /rust_target.txt)"

ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
    CC_x86_64_unknown_linux_musl=musl-gcc \
    CC_aarch64_unknown_linux_musl=musl-gcc

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --target "$(cat /rust_target.txt)" --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --target "$(cat /rust_target.txt)" && \
    cp "/app/target/$(cat /rust_target.txt)/release/gotify2matrix" /app/gotify2matrix

###################
# runtime
###################
FROM gcr.io/distroless/static-debian12 AS runtime
WORKDIR /app
COPY --from=builder /app/gotify2matrix /usr/local/bin/gotify2matrix
ENV RUST_LOG="warn,gotify2matrix=debug"
CMD ["/usr/local/bin/gotify2matrix"]
