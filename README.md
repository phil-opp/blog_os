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

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Note that this only applies to this git branch, other branches might be licensed differently.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
