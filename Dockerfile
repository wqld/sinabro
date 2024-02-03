FROM alpine:3.18

ARG ARCH

COPY --chown=root:root /target/${ARCH}-unknown-linux-musl/debug/agent /app/
COPY --chown=root:root /target/${ARCH}-unknown-linux-musl/debug/sinabro-cni /sinabro-cni

ENV RUST_LOG=info

EXPOSE 8080
CMD ["/app/agent"]
