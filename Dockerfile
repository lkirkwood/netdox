FROM docker.io/rust:1.79-slim-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev pkg-config

WORKDIR /opt
COPY Cargo.toml /opt/
COPY Cargo.lock /opt/
COPY src /opt/src
RUN cargo build --release

FROM docker.io/python:3.12-slim-bookworm

ENV NETDOX_SECRET=default-secret!?
ENV NETDOX_CONFIG=/opt/config

COPY --from=build /opt/target/release/netdox /usr/bin/netdox

ENTRYPOINT ["/usr/bin/netdox"]
