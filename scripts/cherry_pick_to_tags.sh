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

# Copy internal script to a temporary untracked file because an untracked
# file is kept by git when switching branches (that way we can update tags
# where this script doesn't exist).
cp "cherry_pick_to_tags_internal.sh" "cherry_pick_to_tags_internal_tmp.sh"
sh "cherry_pick_to_tags_internal_tmp.sh" $*
rm "cherry_pick_to_tags_internal_tmp.sh"
