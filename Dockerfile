# syntax=docker/dockerfile:1
# healthvibe image: three stages.
#   1. web   - builds the React SPA (web/) into static files
#   2. build - compiles a fully static musl binary
#   3. final - distroless "static" image, non-root, nothing but the binary

# ---- 1. web ------------------------------------------------------------------
FROM node:22-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# ---- 2. build --------------------------------------------------------------
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --bin healthvibe \
 && cp target/release/healthvibe /healthvibe

# ---- 3. final --------------------------------------------------------------
FROM gcr.io/distroless/static-debian12:nonroot
LABEL org.opencontainers.image.title="healthvibe" \
      org.opencontainers.image.description="Self-hosted, privacy-first Apple Health dashboard" \
      org.opencontainers.image.source="https://github.com/mortbrew/healthvibe" \
      org.opencontainers.image.licenses="MIT"
COPY --from=build /healthvibe /healthvibe
COPY --from=web /web/dist /web
USER 1000:1000
ENV HEALTHVIBE_BIND=0.0.0.0:8000 \
    HEALTHVIBE_DB_DIR=/data/db \
    HEALTHVIBE_STAGING_DIR=/data/staging \
    HEALTHVIBE_WEB_DIR=/web
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD ["/healthvibe", "healthcheck"]
ENTRYPOINT ["/healthvibe"]
CMD ["serve"]
