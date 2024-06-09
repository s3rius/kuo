FROM rust:1.78-bookworm as builder

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . /app
RUN cargo build --release

FROM debian:bookworm as runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/kuo-crds /bin
COPY --from=builder /app/target/release/kuo-operator /bin

CMD [ "kuo-operator" ]
