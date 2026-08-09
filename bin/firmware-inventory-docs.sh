#!/usr/bin/env sh
set -eu

unset RUST_LOG

set -x

# Special commands in alphabetical order
firmware-inventory help
firmware-inventory help completions
# Normal commands in alphabetical order
firmware-inventory help get
firmware-inventory help list
firmware-inventory help login
firmware-inventory help update
