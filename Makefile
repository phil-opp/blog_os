# Copyright 2016 Philipp Oppermann. See the README.md
# file at the top-level directory of this distribution.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

arch ?= x86_64
target ?= $(arch)-blog_os
kernel := build/kernel-$(arch).bin
iso := build/os-$(arch).iso

rust_os := target/$(target)/debug/libblog_os.a
linker_script := src/arch/$(arch)/linker.ld
grub_cfg := src/arch/$(arch)/grub.cfg
assembly_source_files := $(wildcard src/arch/$(arch)/*.asm)
assembly_object_files := $(patsubst src/arch/$(arch)/%.asm, \
	build/arch/$(arch)/%.o, $(assembly_source_files))

# used by docker_* targets
docker_image ?= blog_os
tag ?= 0.1
docker_cargo_volume ?=  blogos-$(shell id -u)-$(shell id -g)-cargo
docker_rustup_volume ?=  blogos-$(shell id -u)-$(shell id -g)-rustup
docker_args ?= -e LOCAL_UID=$(shell id -u) -e LOCAL_GID=$(shell id -g) -v $(docker_cargo_volume):/usr/local/cargo -v $(docker_rustup_volume):/usr/local/rustup -v $(shell pwd):$(shell pwd) -w $(shell pwd)
docker_clean_args ?= $(docker_cargo_volume) $(docker_rustup_volume)

.PHONY: all clean run debug iso cargo gdb

all: $(kernel)

clean:
	@cargo clean
	@rm -rf build

run: $(iso)
	@qemu-system-x86_64 -cdrom $(iso) -s

debug: $(iso)
	@qemu-system-x86_64 -cdrom $(iso) -s -S

# docker_* targets 

docker_build:
	@docker build docker/ -t $(docker_image):$(tag)

docker_iso: 
	@docker run --rm $(docker_args) $(docker_image):$(tag) make iso

docker_run: docker_iso
	@qemu-system-x86_64 -cdrom $(iso) -s

docker_interactive:
	@docker run -it --rm $(docker_args) $(docker_image):$(tag) 

docker_clean:
	@docker volume rm $(docker_clean_args)

gdb:
	@rust-os-gdb/bin/rust-gdb "build/kernel-x86_64.bin" -ex "target remote :1234"

iso: $(iso)

$(iso): $(kernel) $(grub_cfg)
	@mkdir -p build/isofiles/boot/grub
	@cp $(kernel) build/isofiles/boot/kernel.bin
	@cp $(grub_cfg) build/isofiles/boot/grub
	@grub-mkrescue -o $(iso) build/isofiles 2> /dev/null
	@rm -r build/isofiles

$(kernel): cargo $(rust_os) $(assembly_object_files) $(linker_script)
	@ld -n --gc-sections -T $(linker_script) -o $(kernel) $(assembly_object_files) $(rust_os)

cargo:
	@xargo build --target $(target)

# compile assembly files
build/arch/$(arch)/%.o: src/arch/$(arch)/%.asm
	@mkdir -p $(shell dirname $@)
	@nasm -felf64 $< -o $@
