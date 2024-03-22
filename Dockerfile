ARG VERSION=bookworm

FROM rust:1-slim-$VERSION AS build

RUN <<EOF /bin/sh -e
apt update
apt install -y make perl
apt-get clean
EOF

WORKDIR /app

COPY . .

RUN --mount=type=cache,target=target <<EOF /bin/sh -e
cargo build -r
cp -r target/release /build
EOF

FROM debian:$VERSION-slim AS runtime

ENV WORKFLOWS_DIR /var/lib/crabflow/workflows

WORKDIR /opt/crabflow

RUN <<EOF /bin/sh -e
useradd -mrd /opt/crabflow crabflow
chown -R crabflow: .
mkdir -p $WORKFLOWS_DIR
chown -R crabflow: $WORKFLOWS_DIR
apt update
apt install -y ca-certificates
apt-get clean
EOF

USER crabflow

FROM runtime AS builder

ENV PATH=/opt/crabflow/.cargo/bin:$PATH

USER root

RUN <<EOF /bin/sh -e
apt update
apt install -y curl
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc
chmod a+r /etc/apt/keyrings/docker.asc
echo \
    "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian \
    $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
    tee /etc/apt/sources.list.d/docker.list > /dev/null
apt update
apt install -y docker-ce-cli
apt-get clean
EOF

USER crabflow

RUN <<EOF /bin/sh -e
curl -sSfo /tmp/rustup.sh https://sh.rustup.rs
/bin/bash /tmp/rustup.sh -y
rm /tmp/rustup.sh
EOF

COPY --from=build /build/crabflow-builder /usr/local/bin/crabflow-builder

ENTRYPOINT ["/usr/local/bin/crabflow-builder"]
CMD ["-w"]

FROM runtime AS git-synchronizer

COPY --from=build /build/crabflow-git-synchronizer /usr/local/bin/crabflow-git-synchronizer

ENTRYPOINT ["/usr/local/bin/crabflow-git-synchronizer"]

FROM runtime AS migrator

ENV MIGRATIONS_DIR /opt/crabflow/migrations

COPY --from=build /build/crabflow-migrator /usr/local/bin/crabflow-migrator
COPY migrator/resources/main/db/migrations $MIGRATIONS_DIR

ENTRYPOINT ["/usr/local/bin/crabflow-migrator"]

FROM runtime AS scheduler

COPY --from=build /build/crabflow-scheduler /usr/local/bin/crabflow-scheduler

ENTRYPOINT ["/usr/local/bin/crabflow-scheduler"]
