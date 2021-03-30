FROM rust:1.51

RUN apt update && apt install -y nasm gdb

CMD /bin/bash
