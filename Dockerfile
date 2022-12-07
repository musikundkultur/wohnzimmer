FROM rust:1.65.0-slim-bullseye as builder

WORKDIR /usr/src/wohnzimmer
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo install --locked --path .
 
FROM debian:bullseye-slim

COPY static/ static/
COPY templates/ templates/
COPY --from=builder /usr/local/cargo/bin/wohnzimmer /usr/local/bin/wohnzimmer

USER nobody
EXPOSE 8080

CMD ["wohnzimmer", "--listen-addr", "0.0.0.0:8080"]
