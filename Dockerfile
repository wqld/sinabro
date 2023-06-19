FROM cgr.dev/chainguard/static
COPY --chown=nonroot:nonroot ./agent/agent /app/
EXPOSE 8080
ENTRYPOINT ["/app/agent"]
