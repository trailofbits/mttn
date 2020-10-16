mttn
====

*memory tracer, take N*

[![Build Status](https://github.com/trailofbits/mttn/workflows/CI/badge.svg)](https://github.com/trailofbits/mttn/actions?query=workflow%3ACI)

---

`mttn` is a small, very slow program tracer with a few specific goals:

* Not modifying the program's instruction stream.
* Faithfully recording most memory accesses.
* Generating traces that are suitable for SIEVE's Tiny86.

Strong anti-goals:
* Being fast.

Weak anti-goals:

* Supporting memory accesses that are either variable-sized or larger than 64 bits.

### Building and use

*mttn* uses Linux-specific `ptrace` APIs and syscalls, so you'll need to run it on a relatively
recent Linux kernel. If you're on another platform, you can use the Dockerfile:

```bash
$ docker build . -t mttn
$ docker run --rm -it --cap-add=SYS_PTRACE -v $(pwd):/app/mttn mttn
$ # in docker
$ cd /app/mttn
```

Once you have the appropriate environment, just `cargo build`:

```bash
$ cargo build
$ ./target/debug/mttn -h
```
