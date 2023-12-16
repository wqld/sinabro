# cross build --target aarch64-unknown-linux-musl --release
FROM alpine:3.18

COPY --chown=root:root /target/aarch64-unknown-linux-musl/release/sinabro /app/
COPY --chown=root:root /target/aarch64-unknown-linux-musl/release/sinabro-cni /sinabro-cni

RUN apk update && apk add iproute2 iptables

EXPOSE 8080
CMD ["/app/sinabro"]
