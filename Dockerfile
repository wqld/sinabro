# cross build --target x86_64-unknown-linux-musl --release
FROM alpine:3.18

COPY --chown=root:root /target/x86_64-unknown-linux-musl/release/sinabro /app/
COPY --chown=root:root /tests/bin/sinabro-cni /sinabro-cni

RUN apk update && apk add iproute2

EXPOSE 8080
CMD ["/app/sinabro"]
