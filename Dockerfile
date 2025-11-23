FROM rust:latest AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs

RUN cargo build --release && rm -rf src

COPY src ./src
COPY migrations ./migrations

RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r botuser && useradd -r -g botuser botuser

WORKDIR /app

COPY --from=builder /app/target/release/genesis /usr/local/bin/genesis
COPY --from=builder --chown=botuser:botuser /app/migrations ./migrations

RUN chown -R botuser:botuser /app

USER botuser

CMD ["/usr/local/bin/genesis"]
