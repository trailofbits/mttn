FROM trailofbits/openjdk-i386 as openjdk

FROM trailofbits/mttn

RUN mkdir /jdk

COPY --from=trailofbits/openjdk-i386 /jdk /jdk

ENV PATH=/jdk/build/linux-x86-server-release/jdk/bin:$PATH
ENV LD_LIBRARY_PATH=/jdk/build/linux-x86-server-release/jdk/lib:$LD_LIBRARY_PATH
