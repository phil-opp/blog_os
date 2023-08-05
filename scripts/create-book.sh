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
pandoc book.md -o book.epub --metadata title="Writing an OS in Rust"  --metadata author="Philipp Oppermann"
