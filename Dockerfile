FROM rust:1.71.0-buster AS builder

WORKDIR /app

COPY . ./

RUN cargo build --release

FROM debian:buster-slim

COPY --from=builder /app/target/release/opuza /usr/local/bin/opuza

COPY "entrypoint.sh" .
RUN chmod +x entrypoint.sh

CMD ["./entrypoint.sh"]
