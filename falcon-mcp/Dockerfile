# syntax=docker/dockerfile:1.6
FROM rust:1.75-slim AS builder
WORKDIR /src
COPY falcon-mcp/Cargo.toml falcon-mcp/Cargo.lock* ./
COPY falcon-mcp/src ./src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /src/target/release/falcon-mcp /usr/local/bin/falcon-mcp
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/falcon-mcp"]
CMD ["--http", "8080", "--root", "/workspace"]
