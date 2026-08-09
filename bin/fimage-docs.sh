#!/usr/bin/env sh
set -eu

unset RUST_LOG

set -x

# Special commands in alphabetical order
fimage help
# Normal commands in alphabetical order
fimage help extract
fimage help inspect
