# syntax=docker/dockerfile:1

FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /srv/brillante \
         --shell /usr/sbin/nologin brillante
COPY --from=build /src/target/release/brillante /usr/local/bin/brillante
WORKDIR /srv/brillante
USER brillante
EXPOSE 3000
ENTRYPOINT ["brillante"]
CMD ["--host", "0.0.0.0"]
