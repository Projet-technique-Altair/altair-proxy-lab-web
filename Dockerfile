FROM rust:1.92-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && groupadd --system --gid 10001 altair \
  && useradd --system --uid 10001 --gid altair --home-dir /nonexistent --shell /usr/sbin/nologin altair \
  && rm -rf /var/lib/apt/lists/*
COPY --from=builder --chown=altair:altair /app/target/release/altair-proxy-lab-web /usr/local/bin/altair-proxy-lab-web
ENV PORT=8086
EXPOSE 8086
USER 10001
CMD ["/usr/local/bin/altair-proxy-lab-web"]
