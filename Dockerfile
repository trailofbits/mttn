FROM rust:1.57

RUN apt update && apt install -y nasm gdb gcc-multilib

CMD /bin/bash
