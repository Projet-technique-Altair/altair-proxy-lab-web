FROM rust:1.92-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /app/target/release/altair-proxy-lab-web /usr/local/bin/altair-proxy-lab-web
ENV PORT=8086
EXPOSE 8086
USER nonroot:nonroot
CMD ["/usr/local/bin/altair-proxy-lab-web"]
