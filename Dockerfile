FROM node:22-alpine AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --ignore-scripts
COPY frontend/ ./
RUN npm run build

FROM rust:1.96-alpine AS backend
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ src/
# build.rs embeds the built UI in the binary.
COPY --from=frontend /build/frontend/dist frontend/dist
RUN cargo build --release --locked

FROM alpine:3.21
RUN adduser -D -u 1000 app && mkdir -p /data && chown app:app /data
WORKDIR /app
COPY --from=backend /build/target/release/mimicway /app/mimicway
USER app
ENV DATA_PATH=/data PORT=7342 BIND_ADDRESS=0.0.0.0
EXPOSE 7342
ENTRYPOINT ["/app/mimicway"]
