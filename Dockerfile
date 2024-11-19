FROM docker.io/rust:1.79-slim-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev pkg-config

WORKDIR /opt
COPY Cargo.toml /opt/
COPY Cargo.lock /opt/
COPY src /opt/src
RUN cargo build --release

FROM docker.io/debian:bookworm-slim

ENV NETDOX_SECRET=default-secret!?
ENV NETDOX_CONFIG=/opt/config

COPY --from=build /usr/lib/x86_64-linux-gnu/libssl.so.3 /usr/lib/x86_64-linux-gnu/libssl.so.3
COPY --from=build /usr/lib/x86_64-linux-gnu/libcrypto.so.3 /usr/lib/x86_64-linux-gnu/libcrypto.so.3
COPY --from=build /etc/ssl/certs/* /etc/ssl/certs/

COPY --from=build /opt/target/release/netdox /usr/bin/netdox

ENTRYPOINT ["/usr/bin/netdox"]
