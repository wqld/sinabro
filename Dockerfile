# cross build --target aarch64-unknown-linux-musl --release
FROM alpine:3.18

COPY --chown=root:root /target/aarch64-unknown-linux-musl/debug/agent /app/
COPY --chown=root:root /target/aarch64-unknown-linux-musl/debug/sinabro-cni /sinabro-cni

ENV RUST_LOG=info

EXPOSE 8080
CMD ["/app/agent"]
