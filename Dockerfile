FROM rust:1.74.0-slim
COPY ./target/release/sinabro /app/
EXPOSE 8080
ENTRYPOINT ["/app/sinabro"]