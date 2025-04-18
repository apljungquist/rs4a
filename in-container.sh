#!/usr/bin/env sh
set -eux

rsync -arv ./ ../w/
cd ../w
cargo run