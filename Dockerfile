FROM rust:1.49

RUN apt update && apt install -y nasm gdb

CMD /bin/bash
