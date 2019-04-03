# Blog OS (A Freestanding Rust Binary)

[![Azure Pipelines CI build](https://img.shields.io/azure-devops/build/phil-opp/blog_os/1/post-01.svg?label=Build&style=flat-square)](https://dev.azure.com/phil-opp/blog_os/_build?definitionId=1)

This repository contains the source code for the [A Freestanding Rust Binary][post] post of the [Writing an OS in Rust](https://os.phil-opp.com) series.

[post]: https://os.phil-opp.com/freestanding-rust-binary/

**Check out the [master branch](https://github.com/phil-opp/blog_os) for more information.**

## Building

To build the project on Linux, run:

```
cargo rustc -- -Clink-arg=-nostartfiles
```

The entry point and the build command differ slightly on macOS and Windows. See the [post] for more information.

Please file an issue if you have any problems.

## License
The source code is dual-licensed under MIT or the Apache License (Version 2.0).
