FROM rust:1.47

RUN cargo install sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache

RUN apt update && apt install -y nasm gdb

CMD /bin/bash
