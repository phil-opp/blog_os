#!/bin/sh
# Copyright 2014 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# Exit if anything fails
set -e

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 COMMIT_HASH TAGS" >&2
    exit 1
fi

branch_name=$(git symbolic-ref -q HEAD)
branch_name=${branch_name##refs/heads/}
branch_name=${branch_name:-HEAD}

commit="$1"

echo "current branch $branch_name"
echo "commit hash $commit"

# update tags
git fetch --tags

shift
for tag in "$@"; do
    echo "UPDATING TAG $tag"
    {
        git branch tmp_update_tag_"$tag" "$tag"

        # cherry pick commit and update tag
	git checkout tmp_update_tag_"$tag"
        git cherry-pick -x "$commit"
        git tag -f "$tag" HEAD

        # switch back to previous branch
        git checkout "$branch_name"
        git branch -D tmp_update_tag_"$tag"

        # push the updated tag
        git push origin "$tag" --force
    } >/dev/null
done
