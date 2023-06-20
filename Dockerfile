FROM cgr.dev/chainguard/static
COPY --chown=nonroot:nonroot ./operator/operator /app/
EXPOSE 8080
ENTRYPOINT ["/app/operator"]
