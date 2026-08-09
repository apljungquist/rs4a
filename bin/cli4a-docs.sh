#!/usr/bin/env sh
set -eu

unset RUST_LOG
unset AXIS_DEVICE_IP
unset AXIS_DEVICE_PASS
unset AXIS_DEVICE_USER
unset AXIS_DEVICE_HTTP_PORT
unset AXIS_DEVICE_HTTPS_PORT
unset AXIS_DEVICE_HTTPS_SELF_SIGNED
unset FIRMWARE_INVENTORY_LOCATION
unset FIRMWARE_INVENTORY_OFFLINE

set -x

# Special commands in alphabetical order
cli4a help
cli4a help completions
# Normal commands in alphabetical order
cli4a help install
