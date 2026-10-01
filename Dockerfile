# Base images are pinned by digest (the tag is kept for readability); Dependabot proposes the updates.
FROM node:22-alpine@sha256:0a7108bf6c7bf5de370ffb1a3ed6be93d405b43ff159f681a8d18c0e2bc2e402 AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --ignore-scripts
COPY frontend/ ./
RUN npm run build

FROM rust:1.96-alpine@sha256:a41f7740f8b45d45795624eec13a8b42263cc700f19f7e4e86e04d3dda08a479 AS backend
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ src/
# build.rs embeds the built UI in the binary.
COPY --from=frontend /build/frontend/dist frontend/dist
RUN cargo build --release --locked

FROM alpine:3.24@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6
RUN adduser -D -u 1000 app && mkdir -p /data && chown app:app /data
WORKDIR /app
COPY --from=backend /build/target/release/mimicway /app/mimicway
USER app
ENV DATA_PATH=/data PORT=7342 BIND_ADDRESS=0.0.0.0
EXPOSE 7342
ENTRYPOINT ["/app/mimicway"]
