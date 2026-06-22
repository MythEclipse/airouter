FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app

# Install trunk for frontend build
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/* && cargo install trunk --locked && rustup target add wasm32-unknown-unknown

# Backend
COPY Cargo.toml Cargo.lock* ./
COPY src/ ./src/
COPY migrations/ ./migrations/
RUN cargo build --release

# Frontend
COPY frontend/ ./frontend/
RUN cd frontend \
    && trunk build --dist /app/frontend-dist --release 2>&1 | tail -1 \
    && cp -r style /app/frontend-dist/style 2>/dev/null || true

# Runtime image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/airouter /usr/local/bin/airouter
COPY --from=builder /app/frontend-dist /usr/local/share/airouter/frontend-dist
COPY config.example.yaml /etc/airouter/config.yaml

ENV AIROUTER_CONFIG=/etc/airouter/config.yaml
WORKDIR /usr/local/share/airouter
EXPOSE 3000
CMD ["airouter"]
