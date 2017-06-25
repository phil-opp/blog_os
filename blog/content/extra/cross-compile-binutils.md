+++
title = "Cross Compile Binutils"
template = "plain.html"
url = "cross-compile-binutils"
order = 2
+++

The [GNU Binutils] are a collection of various binary tools such as `ld`, `as`, `objdump`, or `readelf`. These tools are platform-specific, so you need to compile them again if your host system and target system are different. In our case, we need `ld` and `objdump` for the x86_64 architecture.

[GNU Binutils]: https://www.gnu.org/software/binutils/

## Building Setup
First, you need to download a current binutils version from [here][download] \(the latest one is near the bottom). After extracting, you should have a folder named `binutils-2.X` where `X` is for example `25.1`. Now can create and switch to a new folder for building (recommended):

[download]: ftp://sourceware.org/pub/binutils/snapshots

```bash
mkdir build-binutils
cd build-binutils
```

## Configuration
We execute binutils's `configure` and pass a lot of arguments to it (replace the `X` with the version number):

```bash
../binutils-2.X/configure --target=x86_64-elf --prefix="$HOME/opt/cross" \
    --disable-nls --disable-werror \
    --disable-gdb --disable-libdecnumber --disable-readline --disable-sim
```
- The `target` argument specifies the the x86_64 target architecture.
- The `prefix` argument selects the installation directory, you can change it if you like. But be careful that you do not overwrite your system's binutils.
- The `disable-nls` flag disables native language support (so you'll get the same english error messages). It also reduces build dependencies.
- The `disable-werror` turns all warnings into errors.
- The last line disables features we don't need to reduce compile time.

## Building it
Now we can build and install it to the location supplied as `prefix` (it will take a while):

```bash
make
make install
```
Now you should have multiple `x86_64-elf-XXX` files in `$HOME/opt/cross/bin`.

## Adding it to the PATH
To use the tools from the command line easily, you should add the `bin` folder to your PATH:

```bash
export PATH="$HOME/opt/cross/bin:$PATH"
```
If you add this line to your e.g. `.bashrc`, the `x86_64-elf-XXX` commands are always available.
