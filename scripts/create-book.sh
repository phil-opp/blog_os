#!/bin/bash

# create working dir
rm -r book/
mkdir book/

# copy data to working dir
cat ../blog/content/edition-2/posts/*/index.md > book/book.md
find ../blog/content/edition-2/posts ! -name "*.md" -exec cp -t book/ {} +

# remove zola metadata
sed -i '/^+++/,/^+++/d' book/book.md
# remove br in table in 06, pandoc handles the layout
sed -i '/<br>/d' book/book.md
# details/summary breaks epub layout
sed -i '/^<details>/d' book/book.md
sed -i '/^<\/details>/d' book/book.md
sed -i '/^<summary>/d' book/book.md

# special fix for linking to different folder
sed -i 's|../paging-introduction/||g' book/book.md

# go to work dir and create epub
cd book/
pandoc book.md -o "Writing an OS in Rust.epub" --metadata cover-image="../cover.png" --metadata title="Writing an OS in Rust"  --metadata author="Philipp Oppermann" --metadata description="This blog series creates a small operating system in the Rust programming language. Each post is a small tutorial and includes all needed code, so you can follow along if you like. The source code is also available in the corresponding Github repository."

#clean up
cd ..
mv "book/Writing an OS in Rust.epub" .
rm -rf book/
