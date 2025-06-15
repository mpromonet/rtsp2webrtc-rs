FROM rust AS builder
LABEL maintainer=michel.promonet@free.fr
LABEL description="RTSP to webrtc proxy written in Rust"

ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID

RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && apt-get update \
    && apt-get install -y sudo \
    && echo "$USERNAME ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME

WORKDIR /workspace

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./src ./src

RUN cargo build --release

USER $USERNAME

FROM rust:slim
WORKDIR /app

COPY --from=builder /workspace/target/release/rtsp2webrtc-rs .
COPY ./config.json .
COPY ./www ./www/

ENTRYPOINT ["./rtsp2web-rs"]
CMD ["-C", "config.json", "-k", "key.pem", "-c", "cert.pem"]
