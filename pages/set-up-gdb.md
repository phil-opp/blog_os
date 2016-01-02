---
layout: page
title: "Set Up GDB"
---
There are a lot of things that can go wrong when developing an OS. So it's a good idea to add a debugger to our toolset, which allows us to set breakpoints and examine variables. We will use [GDB](https://www.gnu.org/software/gdb/) as QEMU supports it out of the box.

### QEMU parameters
To make QEMU listen for a gdb connection, we add the `-s` flag to the `run` target in our Makefile:

```make
run: $(iso)
	@qemu-system-x86_64 -cdrom $(iso) -s
```
This allows us to connect a debugger at any time, for example to investigate why a panic occurred.

To wait for a debugger connection on startup, we add a `debug` target to the Makefile:

```make
debug: $(iso)
	@qemu-system-x86_64 -cdrom $(iso) -s -S
```
It is identical to the `run` target except for the additional `-S` flag. This flag causes QEMU to freeze on startup and wait until a debugger is connected. Now it _should_ be possible to connect gdb.

### The annoying issue
Unfortunately gdb has an issue with the switch to long mode. If we connect when the CPU is already in long mode, everything works fine. But if we use `make debug` and thus connect right at the start, we get an error when we set a breakpoint in 64-bit mode:

```
Remote 'g' packet reply is too long: [a very long number]
```
This issue is known [since 2012][gdb issue patch] but it is still not fixed. Maybe we find the reason in the [issue thread][gdb issue thread]:

[gdb issue patch]: http://www.cygwin.com/ml/gdb-patches/2012-03/msg00116.html
[gdb issue thread]: https://sourceware.org/bugzilla/show_bug.cgi?id=13984#c11

> from my (limited) experience, unless you ping the gdb-patches list weekly, this patch is more likely to remain forgotten :-)

Pretty frustrating, especially since the patch is [very small][gdb patch commit].

[gdb patch commit]: https://github.com/phil-opp/binutils-gdb/commit/9e88c451844ad38bb82fe77d1f388c87c41b4520

### Building the patched GDB
So the only way to use gdb with `make debug` is to build a modified gdb version that includes the patch. I created a repository with the patched GDB to make this easy. Just follow [the build instructions].

[the build instructions]: https://github.com/phil-opp/binutils-gdb#gdb-for-64-bit-rust-operating-systems

### Connecting GDB
Now you should have a `rust-os-gdb` subfolder. In its `bin` directory you find the `gdb` executable and the `rust-gdb` script, which [improves rendering of Rust types]. To make it easy to use it for our OS, we add a `make gdb` target to our Makefile:

[improves rendering of Rust types]: https://michaelwoerister.github.io/2015/03/27/rust-xxdb.html

```make
gdb:
	@rust-os-gdb/bin/rust-gdb "build/kernel-x86_64.bin" -ex "target remote :1234"
```
It loads the debug information from our kernel binary and connects to the `localhost:1234` port, on which QEMU listens by default.

### Using GDB
After connecting to QEMU, you can use various gdb commands to control execution and examine data. All commands can be abbreviated as long they are still unique. For example, you can write `c` or `cont` instead of `continue`. The most important commands are:

- `help` or `h`: Show the help.
- `break` or `b`: Set a breakpoint. It possible to break on functions such as `rust_main` or on source lines such as `lib.rs:42`. You can use tab for autocompletion and omit parts of the path as long it's still unique. To modify breakpoints, you can use `disable`, `enable`, and `delete` plus the breakpoint number.
- `continue` or `c`: Continue execution until a breakpoint is reached.
- `next` or `n`: Step over the current line and break on the next line of the function. Sometimes this doesn't work in Rust OSes.
- `step` or `s`: Step into the current line, i.e. jump to the called function. Sometimes this doesn't work in Rust OSes.
- `list` or `l`: Shows the source code around the current position.
- `print` or `p`: Prints the value of a variable. You can use Cs `*` and `&` operators. To print in hexadecimal, use `p/x`.
- `tui enable`: Enables the text user interface, which provides a graphical interface (see below). To disable it again, run `tui disable`.

![gdb text user interface](images/gdb-tui-screenshot.png)

Of course there are many more commands. Feel free to send a PR if you think this list is missing something important. For a more complete GDB overview, check out [Beej's Quick Guide][bggdb] or the [website for Harvard's CS161 course][CS161].

[bggdb]: http://beej.us/guide/bggdb/
[CS161]: http://www.eecs.harvard.edu/~margo/cs161/resources/gdb.html
