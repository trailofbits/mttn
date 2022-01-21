FROM rust:1.58.1

RUN apt update && apt install -y nasm gdb gcc-multilib

CMD /bin/bash
