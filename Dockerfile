FROM rust:1.84-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ ./src/
COPY frontend/ frontend/
COPY frontend-dist/ frontend-dist/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/airouter /usr/local/bin/airouter
COPY --from=builder /app/frontend-dist /usr/local/share/airouter/frontend-dist
COPY config.example.yaml /etc/airouter/config.yaml

ENV AIROUTER_CONFIG=/etc/airouter/config.yaml
WORKDIR /usr/local/share/airouter
EXPOSE 3000

CMD ["airouter"]
