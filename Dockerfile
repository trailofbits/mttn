FROM rust:1.53

RUN apt update && apt install -y nasm gdb gcc-multilib

CMD /bin/bash
