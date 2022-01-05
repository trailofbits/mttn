FROM trailofbits/openjdk-i386 as openjdk

FROM rust:1.57

ENV DEBIAN_FRONTEND=noninteractive
RUN apt update && apt install -y nasm gdb gcc-multilib

RUN mkdir /jdk

COPY --from=trailofbits/openjdk-i386 /jdk /jdk

ENV PATH=/jdk/build/linux-x86-server-release/jdk/bin:$PATH
ENV LD_LIBRARY_PATH=/jdk/build/linux-x86-server-release/jdk/lib:LD_LIBRARY_PATH

WORKDIR /workdir

CMD /bin/bash
