# Stage 1: Build the Rust application
FROM rust:1.83 as builder

WORKDIR /app

COPY . .

# Install necessary system dependencies
RUN apt-get update && apt-get install -y libssl-dev pkg-config

# Build the application and run tests
RUN cargo test --release
RUN cargo build --release

# Stage 2: Create the runtime container
FROM debian:bullseye-slim
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/solana_phoenix_tx_api /app/

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Expose the application's port
EXPOSE 8080

# Command to run the application
CMD ["./solana_phoenix_tx_api"]