# Self-contained TAN Stremio proxy image: the Rust addon + the TAN LADSPA plugin
# + ffmpeg + curl, nothing else needed on the host. Build context = repo root:
#   docker build -f tan-stremio/deploy/Dockerfile -t tan-stremio .
# (docker compose does this for you.)

# ---- build stage -----------------------------------------------------------
FROM rust:1-bookworm AS builder
WORKDIR /src
COPY . .

# Build the addon and tan-ffi (produces the self-contained cdylib libtan.so,
# with all Rust std deps already resolved into it).
RUN cargo build -p tan-ffi -p tan-stremio --release

# The stereo TAN LADSPA plugin (native ELF .so). Link against libtan.so and
# bake an absolute RPATH so it still finds libtan.so after the proxy copies the
# plugin into a per-session temp dir. Linking the cdylib (not a static lib)
# avoids guessing the static Rust runtime's system libs.
RUN mkdir -p /out && gcc -shared -fPIC -O2 -I tan-stremio/ladspa \
        tan-stremio/ladspa/tan_ladspa.c \
        -L target/release -ltan -Wl,-rpath,/app/ladspa \
        -o /out/tan_ladspa.so

# ---- runtime stage ---------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ffmpeg curl ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/tan-stremio /app/tan-stremio
COPY --from=builder /src/target/release/libtan.so /app/ladspa/libtan.so
COPY --from=builder /out/tan_ladspa.so /app/ladspa/tan_ladspa.so
COPY tan-stremio/deploy/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

EXPOSE 5870
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:5870/healthz || exit 1
# Config via env (see .env.example): TAN_UPSTREAM (required), TAN_SECRET,
# TAN_PROFILE. The proxy binds 0.0.0.0 and serves HLS; a fronting reverse proxy
# terminates TLS and sets X-Forwarded-Proto so URLs come out https.
ENTRYPOINT ["/app/entrypoint.sh"]
