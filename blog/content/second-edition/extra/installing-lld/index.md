+++
title = "Installing LLD"
order = 3
path = "installing-lld"
template = "second-edition/extra.html"
+++

[LLD] is the linker by the LLVM project. It has the big advantage that it is a cross-linker by default. This means that you can link libraries and executables for all kinds of platforms with the same LLD installation.

[LLD]: https://lld.llvm.org/

There are plans to distribute LLD together with the Rust compiler, but is not quite there yet. So you have to install it manually. On this page, we try to describe the installation procedure for as many platforms as possible, so if you have additional information for any listed or unlisted platform, please send a pull request on the [Github repo](https://github.com/phil-opp/blog_os)!

## Linux
On most Linux distributions LLD can be installed through the package manager. For example, for Debian and Ubuntu there is are official apt sources at <https://apt.llvm.org/>.

## Other Platforms
For Windows and Mac you can download a pre-built LLVM release from <http://releases.llvm.org/download.html>, which contains LLD. If there are no pre-compiled versions for your platform (e.g. some other Linux distribution), you can download the source code and [build it yourself](https://lld.llvm.org/#build).
