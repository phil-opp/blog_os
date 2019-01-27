# Blog OS (A Freestanding Rust Binary)

[![Build Status](https://travis-ci.org/phil-opp/blog_os.svg?branch=post-01)](https://travis-ci.org/phil-opp/blog_os/branches)

This repository contains the source code for the [A Freestanding Rust Binary][post] post of the [Writing an OS in Rust](https://os.phil-opp.com) series.

[post]: https://os.phil-opp.com/freestanding-rust-binary/

**Check out the [master branch](https://github.com/phil-opp/blog_os) for more information.**

## Building

You need a nightly Rust compiler. To build the project on Linux, run:

```
cargo rustc -- -Z pre-link-arg=-nostartfiles
```

The entry point and the build command differ slightly on macOS and Windows. See the [post] for more information.

Please file an issue if you have any problems.

## License
The source code is dual-licensed under MIT or the Apache License (Version 2.0).
