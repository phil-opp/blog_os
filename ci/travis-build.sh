#!/bin/sh

set -e

git clone https://github.com/phil-opp/blog_os.git
cp -r blog_os/blog ../blog
./hugo
