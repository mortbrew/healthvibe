# syntax=docker/dockerfile:1
# healthvibe image: three stages.
#   1. web   - builds the React SPA (placeholder until PR13 adds web/)
#   2. build - compiles a fully static musl binary
#   3. final - distroless "static" image, non-root, nothing but the binary

# ---- 1. web (placeholder) -------------------------------------------------
FROM node:22-alpine AS web
WORKDIR /web
# PR13 will: COPY web/ . && npm ci && npm run build
RUN mkdir -p dist

# ---- 2. build --------------------------------------------------------------
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY --from=web /web/dist ./web/dist
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
USER 1000:1000
ENV HEALTHVIBE_BIND=0.0.0.0:8000 \
    HEALTHVIBE_DB_DIR=/data/db \
    HEALTHVIBE_STAGING_DIR=/data/staging
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD ["/healthvibe", "healthcheck"]
ENTRYPOINT ["/healthvibe"]
CMD ["serve"]
