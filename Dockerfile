# Stage 1: Build native Rust Edition 2024 release binaries
FROM rust:1.85-slim-bookworm AS builder
WORKDIR /usr/src/fineract-rust
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --bin fineract-server --bin fineract-schedule-demo

# Stage 2: Minimal distroless runtime container (< 25 MB image, non-root user)
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /usr/src/fineract-rust/target/release/fineract-server /app/fineract-server
COPY --from=builder /usr/src/fineract-rust/target/release/fineract-schedule-demo /app/fineract-schedule-demo
ENV PORT=8080
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/app/fineract-server"]
