# syntax=docker/dockerfile:1.7
#
# One Dockerfile, one build target per service. Everything Rust shares a single builder
# stage, so `api`, `worker` and the three CLI adapters compile the workspace once instead of
# five times; the adapter runtimes differ only in which TLS stack they carry.
#
#   docker build --target api      -t pqcas-api .
#   docker build --target worker   -t pqcas-worker .
#   docker build --target web      -t pqcas-web .
#   docker build --target adapter-openssl -t pqcas-adapter-openssl .
#
ARG RUST_VERSION=1.83
ARG DEBIAN_RELEASE=trixie
ARG NODE_VERSION=22
ARG GO_VERSION=1.25

# ---------------------------------------------------------------- Rust workspace builder
FROM rust:${RUST_VERSION}-slim-${DEBIAN_RELEASE} AS rust-builder
WORKDIR /build
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock* ./
COPY crates crates
COPY services services
COPY adapters/cli adapters/cli
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked \
      --bin pqcas-api --bin pqcas-worker --bin pqcas-adapter \
 && mkdir -p /out \
 && cp target/release/pqcas-api target/release/pqcas-worker target/release/pqcas-adapter /out/

# --------------------------------------------------------------------------- common base
FROM debian:${DEBIAN_RELEASE}-slim AS runtime-base
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --create-home pqcas

# ------------------------------------------------------------------------------- API
FROM runtime-base AS api
COPY --from=rust-builder /out/pqcas-api /usr/local/bin/pqcas-api
# Seeds ship inside the image so `pqcas-api seed` needs no volume mount.
COPY deploy/seeds /app/seeds
ENV SEED_DIR=/app/seeds API_BIND=0.0.0.0:8080
EXPOSE 8080
USER pqcas
HEALTHCHECK --interval=10s --timeout=3s --start-period=20s --retries=5 \
  CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/pqcas-api"]
CMD ["serve"]

# ---------------------------------------------------------------------------- worker
FROM runtime-base AS worker
COPY --from=rust-builder /out/pqcas-worker /usr/local/bin/pqcas-worker
USER pqcas
ENTRYPOINT ["/usr/local/bin/pqcas-worker"]

# ------------------------------------------------------------------- adapter: OpenSSL
# Debian trixie ships OpenSSL 3.5, which implements ML-KEM and ML-DSA natively — no
# oqs-provider needed for the groups this lab exercises.
FROM runtime-base AS adapter-openssl
RUN apt-get update \
 && apt-get install -y --no-install-recommends openssl \
 && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /out/pqcas-adapter /usr/local/bin/pqcas-adapter
ENV ADAPTER_BACKEND=openssl \
    ADAPTER_NAME=openssl \
    ADAPTER_CLIENT_BIN=openssl \
    ADAPTER_BIND=0.0.0.0:9100 \
    ADAPTER_KEM_GROUPS="X25519MLKEM768,SecP256r1MLKEM768,SecP384r1MLKEM1024,MLKEM512,MLKEM768,MLKEM1024,x25519,P-256,P-384,P-521" \
    ADAPTER_SIG_ALGS="mldsa44,mldsa65,mldsa87,ed25519,ecdsa_secp256r1_sha256,ecdsa_secp384r1_sha384,rsa_pss_rsae_sha256,rsa_pss_rsae_sha512"
EXPOSE 9100
USER pqcas
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD curl -fsS http://127.0.0.1:9100/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/pqcas-adapter"]

# ----------------------------------------------------------------- adapter: BoringSSL
FROM debian:${DEBIAN_RELEASE}-slim AS boringssl-builder
ARG BORINGSSL_REF=0.20250514.0
RUN apt-get update \
 && apt-get install -y --no-install-recommends git cmake ninja-build golang-go g++ ca-certificates perl \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
RUN git clone --depth 1 --branch ${BORINGSSL_REF} https://boringssl.googlesource.com/boringssl . \
 && cmake -GNinja -B build -DCMAKE_BUILD_TYPE=Release \
 && ninja -C build bssl

FROM runtime-base AS adapter-boringssl
COPY --from=boringssl-builder /src/build/bssl /usr/local/bin/bssl
COPY --from=rust-builder /out/pqcas-adapter /usr/local/bin/pqcas-adapter
ENV ADAPTER_BACKEND=boringssl \
    ADAPTER_NAME=boringssl \
    ADAPTER_CLIENT_BIN=bssl \
    ADAPTER_BIND=0.0.0.0:9100 \
    ADAPTER_KEM_GROUPS="X25519MLKEM768,MLKEM768,MLKEM1024,x25519,P-256,P-384,P-521" \
    ADAPTER_SIG_ALGS="ecdsa_secp256r1_sha256,ed25519,rsa_pss_rsae_sha256"
EXPOSE 9100
USER pqcas
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD curl -fsS http://127.0.0.1:9100/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/pqcas-adapter"]

# -------------------------------------------------------------------- adapter: wolfSSL
FROM debian:${DEBIAN_RELEASE}-slim AS wolfssl-builder
ARG WOLFSSL_REF=v5.7.6-stable
RUN apt-get update \
 && apt-get install -y --no-install-recommends git autoconf automake libtool make g++ ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
RUN git clone --depth 1 --branch ${WOLFSSL_REF} https://github.com/wolfSSL/wolfssl.git . \
 && ./autogen.sh \
 && ./configure --enable-tls13 --enable-experimental --enable-kyber --enable-dilithium --enable-examples \
 && make -j"$(nproc)"

FROM runtime-base AS adapter-wolfssl
COPY --from=wolfssl-builder /src/examples/client/.libs/client /usr/local/bin/wolfssl-client
COPY --from=wolfssl-builder /src/src/.libs/ /usr/local/lib/
RUN ldconfig
COPY --from=rust-builder /out/pqcas-adapter /usr/local/bin/pqcas-adapter
ENV ADAPTER_BACKEND=wolfssl \
    ADAPTER_NAME=wolfssl \
    ADAPTER_CLIENT_BIN=wolfssl-client \
    ADAPTER_BIND=0.0.0.0:9100 \
    ADAPTER_KEM_GROUPS="X25519MLKEM768,MLKEM512,MLKEM768,MLKEM1024,x25519,P-256,P-384" \
    ADAPTER_SIG_ALGS="mldsa44,mldsa65,mldsa87,ecdsa_secp256r1_sha256"
EXPOSE 9100
USER pqcas
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD curl -fsS http://127.0.0.1:9100/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/pqcas-adapter"]

# ------------------------------------------------------------------------- adapter: Go
FROM golang:${GO_VERSION}-${DEBIAN_RELEASE} AS go-builder
WORKDIR /src
COPY adapters/go/go.mod ./
COPY adapters/go/*.go ./
RUN CGO_ENABLED=0 go build -trimpath -ldflags "-s -w" -o /out/adapter-go .

FROM runtime-base AS adapter-go
COPY --from=go-builder /out/adapter-go /usr/local/bin/adapter-go
ENV ADAPTER_NAME=go ADAPTER_BIND=0.0.0.0:9100
EXPOSE 9100
USER pqcas
HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD curl -fsS http://127.0.0.1:9100/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/adapter-go"]

# ------------------------------------------------------------------------------- web
FROM node:${NODE_VERSION}-${DEBIAN_RELEASE}-slim AS web-builder
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml* ./
COPY apps/web/package.json apps/web/
RUN pnpm install --frozen-lockfile
COPY apps/web apps/web
WORKDIR /app/apps/web
ENV NEXT_TELEMETRY_DISABLED=1
RUN pnpm build

FROM node:${NODE_VERSION}-${DEBIAN_RELEASE}-slim AS web
WORKDIR /app
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl \
 && rm -rf /var/lib/apt/lists/*
ENV NODE_ENV=production NEXT_TELEMETRY_DISABLED=1 PORT=3000 HOSTNAME=0.0.0.0
# `output: "standalone"` emits the server plus only the traced dependencies.
COPY --from=web-builder /app/apps/web/.next/standalone ./
COPY --from=web-builder /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=web-builder /app/apps/web/public ./apps/web/public
EXPOSE 3000
USER node
HEALTHCHECK --interval=10s --timeout=3s --start-period=20s --retries=5 \
  CMD curl -fsS http://127.0.0.1:3000/en || exit 1
CMD ["node", "apps/web/server.js"]
