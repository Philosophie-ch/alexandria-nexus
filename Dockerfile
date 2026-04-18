# Build stage
FROM rust:1.94-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

# Pre-populate known_hosts for GitLab (hexforge private dep)
RUN mkdir -p ~/.ssh && \
    ssh-keyscan gitlab.com >> ~/.ssh/known_hosts

# Copy dependency manifests first (cache layer)
COPY Cargo.toml Cargo.lock ./

# Copy source
COPY src ./src
COPY migrations ./migrations

# Build release binary (SSH agent forwarding for hexforge private dep)
RUN --mount=type=ssh cargo build --release --bin alexandria-nexus

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    python3 \
    python3-pip \
    && pip3 install --no-cache-dir --break-system-packages pylatexenc \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/alexandria-nexus /app/alexandria-nexus
COPY --from=builder /app/migrations /app/migrations

WORKDIR /app
EXPOSE 8080

CMD ["/app/alexandria-nexus"]
