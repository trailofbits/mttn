FROM rust:1.48

RUN apt update && apt install -y nasm gdb

CMD /bin/bash
