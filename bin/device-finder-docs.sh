#!/usr/bin/env sh
set -eu

unset RUST_LOG

set -x

device-finder help
device-finder help completions
