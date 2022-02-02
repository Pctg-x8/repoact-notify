FROM rust:slim-buster as builder

COPY . /src/
WORKDIR /src
RUN cargo build --release

FROM debian:buster-slim

COPY --from=builder /src/target/release/repoact-notify /
CMD ["/repoact-notify"]
