#!/bin/sh

set -e

# build rust project
make

# check formatting (rustfmt)
PATH=~/.cargo/bin:$PATH
cargo fmt -- --write-mode=diff

# clone hugo branch, which contains the blog template
git clone --branch=hugo https://github.com/phil-opp/blog_os.git hugo
cd hugo

# download hugo
wget https://github.com/spf13/hugo/releases/download/v0.16/hugo_0.16_linux-64bit.tgz
tar xf hugo_0.16_linux-64bit.tgz

# build the blog
./hugo

cd ..
