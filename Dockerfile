# This uses cargo-chef to cache dependencies in order to speed up docker builds.
#
# See: https://github.com/LukeMathWalker/cargo-chef
FROM lukemathwalker/cargo-chef:latest-rust-1.65.0 AS chef
WORKDIR /app

## Prepare
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

## Build
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build the dependencies, this is the caching Docker layer.
RUN cargo chef cook --release --recipe-path recipe.json
# Build the application.
COPY . .
RUN cargo build --release

## Package
FROM debian:bullseye-slim
COPY --from=builder /app/target/release/wohnzimmer /usr/local/bin/wohnzimmer
COPY static/ static/
COPY templates/ templates/
USER nobody
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/wohnzimmer"]
CMD ["--listen-addr", "0.0.0.0:8080"]
