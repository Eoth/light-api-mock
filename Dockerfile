FROM node:22-alpine AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci --ignore-scripts
COPY frontend/ ./
RUN npm run build

FROM rust:1.96-alpine AS backend
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
RUN cargo build --release

FROM alpine:3.21
RUN adduser -D -u 1000 app
WORKDIR /app
COPY --from=backend /build/target/release/light-mock /app/light-mock
COPY --from=frontend /build/frontend/dist /app/static
RUN mkdir -p /data && chown app:app /data
USER app
ENV DATA_PATH=/data STATIC_DIR=/app/static PORT=7342
EXPOSE 7342
ENTRYPOINT ["/app/light-mock"]
