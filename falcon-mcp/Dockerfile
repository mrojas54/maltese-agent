# syntax=docker/dockerfile:1.6
FROM rust:1.92-slim AS builder
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY falcon-mcp ./falcon-mcp
COPY falcon-agent ./falcon-agent
RUN cargo build --release --locked -p falcon-mcp

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /src/target/release/falcon-mcp /usr/local/bin/falcon-mcp
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/falcon-mcp"]
CMD ["--http", "8080", "--root", "/workspace"]
