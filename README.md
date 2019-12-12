# Blog OS (Paging Implementation)

[![Azure Pipelines CI build](https://img.shields.io/azure-devops/build/phil-opp/blog_os/1/post-09.svg?label=Build&style=flat-square)](https://dev.azure.com/phil-opp/blog_os/_build?definitionId=1)

This repository contains the source code for the [Paging Implementation][post] post of the [Writing an OS in Rust](https://os.phil-opp.com) series.

[post]: https://os.phil-opp.com/paging-implementation/

**Check out the [master branch](https://github.com/phil-opp/blog_os) for more information.**

## Building

You need a nightly Rust compiler. First you need to install the `cargo-xbuild` and `bootimage` tools:

```
cargo install cargo-xbuild bootimage
```

Then you can build the project by running:

```
cargo xbuild
```

To create a bootable disk image, run:

```
cargo bootimage
```

This creates a bootable disk image in the `target/x86_64-blog_os/debug` directory.

Please file an issue if you have any problems.

## Running

You can run the disk image in [QEMU] through:

[QEMU]: https://www.qemu.org/

```
cargo xrun
```

Of course [QEMU] needs to be installed for this.

You can also write the image to an USB stick for booting it on a real machine. On Linux, the command for this is:

```
dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

Where `sdX` is the device name of your USB stick. **Be careful** to choose the correct device name, because everything on that device is overwritten.

## Testing

To run the unit and integration tests, execute `cargo xtest`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Note that this only applies to this git branch, other branches might be licensed differently.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
