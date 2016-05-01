#!/bin/sh

set -e

# build rust project
make

# clone hugo branch, which contains the blog template
git clone --branch=hugo https://github.com/phil-opp/blog_os.git hugo
cd hugo

# download hugo
wget https://github.com/spf13/hugo/releases/download/v0.15/hugo_0.15_linux_amd64.tar.gz
tar xf hugo_0.15_linux_amd64.tar.gz

# build the blog
hugo_0.15_linux_amd64/hugo_0.15_linux_amd64

cd ..
