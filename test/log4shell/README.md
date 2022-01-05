# log4shell exploit tracing

This directory has a minimal proof-of-concept for the "Log4Shell" log4j remote code execution vulnerability
([CVE-2021-44832](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2021-44832)) in the
[`vulnerable-app`](vulnerable-app) subdirectory, as well as the latest version of OpenJDK
targeted to the i386 platform in [`OpenJDK-i386`](OpenJDK-i386).

## Tracing the exploit with mttn

In this directory, run
```shell
make log4shell.trace
```

The following is automatically performed by the Makefile in this directory,
but is included here for reference.

## Reproducing and testing the exploit

In the `vulnerable-app` subdirectory, run
```shell
make test
```

## Building the latest OpenJDK for i386

In the `OpenJDK-i386` directory, run
```shell
make docker
```
Running
```shell
make run
```
in that directory will start a Docker container in which the `java` in the `PATH` is the i386 version.
