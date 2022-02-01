FROM rust:1.58.1

ENV DEBIAN_FRONTEND=noninteractive
RUN apt update && apt install -y nasm gdb gcc-multilib g++-multilib

ENV PATH=/app/mttn/target/debug/:$PATH

WORKDIR /app/mttn

CMD /bin/bash
