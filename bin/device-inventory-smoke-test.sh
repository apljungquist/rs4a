#!/usr/bin/env sh
set -eu

DB_SNAPSHOT=crates/device-inventory/test-data/devices.json

unset RUST_LOG
export DEVICE_INVENTORY_LOCATION=$(mktemp -d)
export DEVICE_INVENTORY_OFFLINE=true

set -x

device-inventory load < "${DB_SNAPSHOT}"
device-inventory dump > "${DB_SNAPSHOT}"
device-inventory add local 192.168.0.90 root pass
device-inventory for-each --alias 'l*' sh -- -c 'echo $AXIS_DEVICE_IP'
device-inventory for-each sh -- -c 'echo $AXIS_DEVICE_IP'
device-inventory activate --alias local
eval $(device-inventory activate --alias local)
device-inventory list
device-inventory remove --alias 'local'
device-inventory list
device-inventory remove --alias 'vlt-*'
device-inventory list
