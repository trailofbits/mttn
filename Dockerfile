FROM rust:1.63.0

RUN apt update && apt install -y nasm gdb gcc-multilib

CMD /bin/bash
