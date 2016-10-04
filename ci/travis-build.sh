#!/bin/sh

set -e

# codegen
cd codegen
cargo run -- -o "../layouts/partials/recent-updates.html"
cd ..

git clone https://github.com/phil-opp/blog_os.git
cp -r blog_os/blog ../blog
./hugo
