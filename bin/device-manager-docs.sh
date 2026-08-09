#!/usr/bin/env sh
set -eu

unset RUST_LOG
unset AXIS_DEVICE_IP
unset AXIS_DEVICE_PASS
unset AXIS_DEVICE_USER
unset AXIS_DEVICE_HTTP_PORT
unset AXIS_DEVICE_HTTPS_PORT
unset AXIS_DEVICE_HTTPS_SELF_SIGNED

set -x

# Special commands in alphabetical order
device-manager help
device-manager help completions
# Normal commands in alphabetical order
device-manager help init
device-manager help reinit
device-manager help restore
device-manager help upgrade
