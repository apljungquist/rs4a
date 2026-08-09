#!/usr/bin/env sh
set -eu

unset RUST_LOG
unset AXIS_DEVICE_HTTPS_SELF_SIGNED

set -x

# Special commands in alphabetical order
device-inventory help
device-inventory help completions
# Normal commands in alphabetical order
device-inventory help activate
device-inventory help add
device-inventory help deactivate
device-inventory help dump
device-inventory help for-each
device-inventory help sync
device-inventory help list
device-inventory help load
device-inventory help login
device-inventory help remove
