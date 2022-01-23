# Building Blog OS using Docker
Inspired by [redox].
You just need `git`, `make`, and `docker`.
It is better to use a non-privileged user to run the `docker` command, which is usually achieved by adding the user to the `docker` group.

## Run the container to build Blog OS
You can build the docker image using `make docker_build` and run it using `make docker_run`.

## Run the container interactively
You can use the `make` target `docker_interactive` to get a shell in the container.

## Clear the toolchain caches (Cargo & Rustup)
To clean the docker volumes used by the toolchain, you just need to run `make docker_clean`.

[redox]: https://github.com/redox-os/redox

## License
The source code is dual-licensed under MIT or the Apache License (Version 2.0). This excludes the `blog` directory.
