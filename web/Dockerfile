FROM rust:bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs

RUN cargo build --release && rm -rf src

COPY src ./src
COPY static ./static

RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r webuser && useradd -r -g webuser webuser

WORKDIR /app

COPY --from=builder /app/target/release/genesis-web /usr/local/bin/genesis-web
COPY --from=builder /app/static ./static

USER webuser

EXPOSE 8080

CMD ["/usr/local/bin/genesis-web"]
